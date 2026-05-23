//! Structured battlegroup operations backed by a remote command runner.

use serde_json::Value;

use crate::{
    errors::{failure, parse_json},
    models::CommandResult,
    orchestration::{BattlegroupRef, RemoteCommandRunner},
    validation::validate_kube_arg,
};

use super::region_patch::{region_patch_operations, sh_single_quoted, validate_region};
use super::types::{BattlegroupStatusSnapshot, LogFile, PodContainerRef, PodShellSpec};

const BATTLEGROUP_NAMESPACE_PREFIX: &str = "funcom-seabass-";

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

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
