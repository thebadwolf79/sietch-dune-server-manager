//! Kubernetes-backed battlegroup queries, patches, shell specs, and log exports.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    errors::{failure, parse_json},
    models::CommandResult,
    orchestration::{BattlegroupRef, RemoteCommandRunner},
    validation::validate_kube_arg,
};

const BATTLEGROUP_NAMESPACE_PREFIX: &str = "funcom-seabass-";

/// Pod/container pair discovered in a namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodContainerRef {
    /// Pod name.
    pub pod: String,
    /// Container name inside the pod.
    pub container: String,
    /// Workload role label, when present.
    pub role: String,
}

/// Candidate commands for opening a shell into a pod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodShellSpec {
    /// Kubernetes namespace.
    pub namespace: String,
    /// Pod name.
    pub pod: String,
    /// Ordered shell command candidates.
    pub commands: Vec<Vec<String>>,
}

/// Exported log file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    /// Relative path to use when writing the log archive.
    pub relative_path: String,
    /// Log contents.
    pub contents: String,
}

/// Combined battlegroup resource and runtime status snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattlegroupStatusSnapshot {
    /// Raw live BattleGroup custom resource JSON.
    pub battlegroup: Value,
    /// Pods and containers in the battlegroup namespace.
    pub pods: Vec<PodContainerRef>,
    /// Director NodePort, when discovered.
    pub director_node_port: Option<u16>,
}

/// Structured battlegroup operations over a remote command runner.
#[derive(Debug, Clone)]
pub struct StructuredBattlegroupOps<R> {
    runner: R,
}

impl<R> StructuredBattlegroupOps<R>
where
    R: RemoteCommandRunner,
{
    /// Creates operations backed by a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Lists battlegroups from all Kubernetes namespaces.
    pub fn list(&self) -> CommandResult<Vec<BattlegroupRef>> {
        let value = self.runner.run_json(
            "sudo kubectl get battlegroups -A -o json",
            "battlegroup list",
        )?;
        let mut refs = value["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let namespace = item["metadata"]["namespace"].as_str()?.to_string();
                let name = item["metadata"]["name"].as_str()?.to_string();
                Some(BattlegroupRef { namespace, name })
            })
            .filter(|item| item.namespace.starts_with(BATTLEGROUP_NAMESPACE_PREFIX))
            .collect::<Vec<_>>();
        refs.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        Ok(refs)
    }

    /// Returns a battlegroup status snapshot.
    pub fn status(&self, battlegroup: &BattlegroupRef) -> CommandResult<BattlegroupStatusSnapshot> {
        battlegroup.validate()?;
        let bg_command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(&battlegroup.name),
            sh_single_quoted(&battlegroup.namespace)
        );
        let battlegroup_json = self.runner.run_json(&bg_command, "battlegroup status")?;
        Ok(BattlegroupStatusSnapshot {
            battlegroup: battlegroup_json,
            pods: self.list_pods(&battlegroup.namespace)?,
            director_node_port: self.director_node_port(&battlegroup.namespace)?,
        })
    }

    /// Patches the region-related fields in a live BattleGroup resource.
    pub fn patch_region(&self, battlegroup: &BattlegroupRef, region: &str) -> CommandResult<()> {
        battlegroup.validate()?;
        validate_region(region)?;
        let command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(&battlegroup.name),
            sh_single_quoted(&battlegroup.namespace)
        );
        let battlegroup_json = self
            .runner
            .run_json(&command, "battlegroup region source")?;
        let operations = region_patch_operations(&battlegroup_json, region)?;
        let patch_command = format!(
            "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o json",
            sh_single_quoted(&battlegroup.name),
            sh_single_quoted(&battlegroup.namespace),
            sh_single_quoted(&serde_json::to_string(&operations).map_err(|err| {
                failure(format!(
                    "Failed to serialize region patch operations: {err}"
                ))
            })?),
        );
        let output = self.runner.run(&patch_command)?;
        let value: Value = parse_json(&output, "patched battlegroup")?;
        let patched_name = value["metadata"]["name"].as_str().unwrap_or_default();
        if patched_name != battlegroup.name {
            return Err(failure(
                "Region patch did not return the expected battlegroup",
            ));
        }
        Ok(())
    }

    /// Lists pods and containers for a namespace.
    pub fn list_pods(&self, namespace: &str) -> CommandResult<Vec<PodContainerRef>> {
        validate_kube_arg(namespace, "namespace")?;
        let command = format!(
            "sudo kubectl get pods -n {} -o json",
            sh_single_quoted(namespace)
        );
        let value = self.runner.run_json(&command, "pod list")?;
        let mut pods = Vec::new();
        for item in value["items"].as_array().cloned().unwrap_or_default() {
            let pod = item["metadata"]["name"].as_str().unwrap_or_default();
            if pod.is_empty() {
                continue;
            }
            let role = item["metadata"]["labels"]["role"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            for container in item["spec"]["containers"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                let container_name = container["name"].as_str().unwrap_or_default();
                if !container_name.is_empty() {
                    pods.push(PodContainerRef {
                        pod: pod.to_string(),
                        container: container_name.to_string(),
                        role: role.clone(),
                    });
                }
            }
        }
        pods.sort_by(|left, right| {
            left.pod
                .cmp(&right.pod)
                .then(left.container.cmp(&right.container))
        });
        Ok(pods)
    }

    /// Builds command candidates for opening a shell in a pod.
    pub fn pod_shell_spec(&self, namespace: &str, pod: &str) -> CommandResult<PodShellSpec> {
        validate_kube_arg(namespace, "namespace")?;
        validate_kube_arg(pod, "pod")?;
        Ok(PodShellSpec {
            namespace: namespace.to_string(),
            pod: pod.to_string(),
            commands: vec![
                vec![
                    "sudo".into(),
                    "kubectl".into(),
                    "exec".into(),
                    "-it".into(),
                    pod.into(),
                    "-n".into(),
                    namespace.into(),
                    "--".into(),
                    "/bin/bash".into(),
                ],
                vec![
                    "sudo".into(),
                    "kubectl".into(),
                    "exec".into(),
                    "-it".into(),
                    pod.into(),
                    "-n".into(),
                    namespace.into(),
                    "--".into(),
                    "/bin/sh".into(),
                ],
            ],
        })
    }

    /// Exports logs for all containers in a namespace.
    pub fn export_namespace_logs(&self, namespace: &str) -> CommandResult<Vec<LogFile>> {
        let pods = self.list_pods(namespace)?;
        self.collect_logs(namespace, &pods)
    }

    /// Exports logs for all operator containers.
    pub fn export_operator_logs(&self) -> CommandResult<Vec<LogFile>> {
        let pods = self.list_pods("funcom-operators")?;
        self.collect_logs("funcom-operators", &pods)
    }

    fn director_node_port(&self, namespace: &str) -> CommandResult<Option<u16>> {
        validate_kube_arg(namespace, "namespace")?;
        let command = format!(
            "sudo kubectl get svc -n {} -o json",
            sh_single_quoted(namespace)
        );
        let value = self.runner.run_json(&command, "service list")?;
        for service in value["items"].as_array().cloned().unwrap_or_default() {
            for port in service["spec"]["ports"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                if port["port"].as_u64() == Some(11717) {
                    return Ok(port["nodePort"]
                        .as_u64()
                        .and_then(|value| u16::try_from(value).ok()));
                }
            }
        }
        Ok(None)
    }

    fn collect_logs(
        &self,
        namespace: &str,
        pods: &[PodContainerRef],
    ) -> CommandResult<Vec<LogFile>> {
        let mut files = Vec::new();
        for item in pods {
            validate_kube_arg(&item.pod, "pod")?;
            validate_kube_arg(&item.container, "container")?;
            let command = format!(
                "sudo kubectl logs -n {} {} -c {} --timestamps --tail=-1",
                sh_single_quoted(namespace),
                sh_single_quoted(&item.pod),
                sh_single_quoted(&item.container),
            );
            let contents = self.runner.run(&command)?;
            files.push(LogFile {
                relative_path: format!("{}/{}.log", item.pod, item.container),
                contents,
            });
        }
        Ok(files)
    }
}

fn validate_region(region: &str) -> CommandResult<()> {
    match region {
        "Europe Test" | "North America Test" => Ok(()),
        _ => Err(failure("Region must be Europe Test or North America Test")),
    }
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn region_patch_operations(value: &Value, region: &str) -> CommandResult<Vec<Value>> {
    let mut operations = Vec::new();
    collect_region_patch_operations(value, &mut Vec::new(), region, &mut operations);
    if operations.is_empty() {
        return Err(failure("No battlegroup region fields were found to patch"));
    }
    Ok(operations)
}

fn collect_region_patch_operations(
    value: &Value,
    path: &mut Vec<String>,
    region: &str,
    operations: &mut Vec<Value>,
) {
    match value {
        Value::Object(map) => {
            if map
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name == "BATTLEGROUP_REGION_NAME")
                && map.get("value").is_some()
            {
                let mut value_path = path.clone();
                value_path.push("value".to_string());
                operations.push(replace_operation(&value_path, json!(region)));
            }

            for (key, child) in map {
                path.push(key.clone());
                if key == "dataCenter" && child.is_string() {
                    operations.push(replace_operation(path, json!(region)));
                }
                collect_region_patch_operations(child, path, region, operations);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(index.to_string());
                if child
                    .as_str()
                    .is_some_and(|text| text.starts_with("-FarmRegion="))
                {
                    operations.push(replace_operation(
                        path,
                        json!(format!("-FarmRegion={region}")),
                    ));
                }
                collect_region_patch_operations(child, path, region, operations);
                path.pop();
            }
        }
        _ => {}
    }
}

fn replace_operation(path: &[String], value: Value) -> Value {
    json!({
        "op": "replace",
        "path": json_pointer(path),
        "value": value,
    })
}

fn json_pointer(path: &[String]) -> String {
    format!(
        "/{}",
        path.iter()
            .map(|item| item.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    use super::*;

    #[derive(Clone, Default)]
    struct MockRemote {
        outputs: Rc<RefCell<VecDeque<String>>>,
        commands: Rc<RefCell<Vec<String>>>,
    }

    impl MockRemote {
        fn with_outputs(outputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
            Self {
                outputs: Rc::new(RefCell::new(outputs.into_iter().map(Into::into).collect())),
                commands: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl RemoteCommandRunner for MockRemote {
        fn run(&self, command: &str) -> CommandResult<String> {
            self.commands.borrow_mut().push(command.to_string());
            self.outputs
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| failure("no mock output queued"))
        }

        fn run_script(&self, script: &str) -> CommandResult<String> {
            self.run(script)
        }
    }

    #[test]
    fn lists_battlegroups_from_cluster_json() {
        let remote = MockRemote::with_outputs([r#"{
          "items": [
            {"metadata":{"namespace":"default","name":"ignored"}},
            {"metadata":{"namespace":"funcom-seabass-sh-host-bbbbbb","name":"sh-host-bbbbbb"}},
            {"metadata":{"namespace":"funcom-seabass-sh-host-aaaaaa","name":"sh-host-aaaaaa"}}
          ]
        }"#]);
        let ops = StructuredBattlegroupOps::new(remote);
        assert_eq!(
            ops.list().unwrap(),
            vec![
                BattlegroupRef {
                    namespace: "funcom-seabass-sh-host-aaaaaa".to_string(),
                    name: "sh-host-aaaaaa".to_string(),
                },
                BattlegroupRef {
                    namespace: "funcom-seabass-sh-host-bbbbbb".to_string(),
                    name: "sh-host-bbbbbb".to_string(),
                }
            ]
        );
    }

    #[test]
    fn region_patch_uses_rust_built_json_patch_without_jq() {
        let remote = MockRemote::with_outputs([
            r#"{
              "metadata":{"name":"sh-host-abcdef"},
              "spec":{
                "dataCenter":"Old",
                "args":["-FarmRegion=Old"],
                "env":[{"name":"BATTLEGROUP_REGION_NAME","value":"Old"}]
              }
            }"#,
            r#"{"metadata":{"name":"sh-host-abcdef"}}"#,
        ]);
        let commands = remote.commands.clone();
        let ops = StructuredBattlegroupOps::new(remote);
        ops.patch_region(
            &BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            "Europe Test",
        )
        .unwrap();
        let commands = commands.borrow();
        assert!(commands[0].contains("kubectl get battlegroup"));
        assert!(commands[1].contains("kubectl patch battlegroup"));
        assert!(commands[1].contains("--type=json"));
        assert!(
            commands[1].contains("BATTLEGROUP_REGION_NAME") || commands[1].contains("/env/0/value")
        );
        assert!(commands[1].contains("dataCenter"));
        assert!(commands[1].contains("-FarmRegion=Europe Test"));
        assert!(!commands.join("\n").contains("jq"));
        assert!(!commands.join("\n").contains(" sed "));
    }

    #[test]
    fn exports_logs_by_enumerating_pods_and_containers() {
        let remote = MockRemote::with_outputs([
            r#"{
              "items": [{
                "metadata":{"name":"pod-a","labels":{"role":"gateway"}},
                "spec":{"containers":[{"name":"main"},{"name":"sidecar"}]}
              }]
            }"#,
            "main log",
            "sidecar log",
        ]);
        let commands = remote.commands.clone();
        let ops = StructuredBattlegroupOps::new(remote);
        let files = ops
            .export_namespace_logs("funcom-seabass-sh-host-abcdef")
            .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].relative_path, "pod-a/main.log");
        let commands = commands.borrow();
        assert!(commands[0].contains("kubectl get pods"));
        assert!(commands[1].contains("kubectl logs"));
        assert!(commands[1].contains("--timestamps"));
    }

    #[test]
    fn builds_pod_shell_command_candidates() {
        let ops = StructuredBattlegroupOps::new(MockRemote::default());
        let spec = ops
            .pod_shell_spec("funcom-seabass-sh-host-abcdef", "pod-a")
            .unwrap();
        assert_eq!(spec.commands[0].last().unwrap(), "/bin/bash");
        assert_eq!(spec.commands[1].last().unwrap(), "/bin/sh");
    }
}
