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
        "Europe",
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
    assert!(commands[1].contains("-FarmRegion=Europe"));
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
