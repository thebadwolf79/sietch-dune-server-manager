use std::{cell::RefCell, rc::Rc};

use crate::orchestration::VecOperationSink;

use super::*;

#[derive(Default)]
struct MockKubernetes {
    calls: Rc<RefCell<Vec<String>>>,
    namespaces: Vec<String>,
    director_ports: Rc<RefCell<Vec<Option<u16>>>>,
    battlegroup_states: Rc<RefCell<Vec<BattlegroupState>>>,
}

impl KubernetesProvider for MockKubernetes {
    fn list_battlegroup_namespaces(&self) -> CommandResult<Vec<String>> {
        Ok(self.namespaces.clone())
    }

    fn patch_battlegroup_stop(
        &self,
        namespace: &str,
        name: &str,
        stop: bool,
    ) -> CommandResult<()> {
        self.calls
            .borrow_mut()
            .push(format!("{namespace}/{name}:{stop}"));
        Ok(())
    }

    fn battlegroup_state(
        &self,
        _namespace: &str,
        _name: &str,
    ) -> CommandResult<BattlegroupState> {
        Ok(self
            .battlegroup_states
            .borrow_mut()
            .pop()
            .unwrap_or_else(running_state))
    }

    fn director_node_port(&self, _namespace: &str) -> CommandResult<Option<u16>> {
        Ok(self.director_ports.borrow_mut().pop().unwrap_or(None))
    }
}

fn running_state() -> BattlegroupState {
    BattlegroupState {
        stop: false,
        phase: "Running".to_string(),
        server_group_phase: "Running".to_string(),
        director_phase: "Healthy".to_string(),
    }
}

#[test]
fn restart_patches_stop_then_start() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes {
        calls: calls.clone(),
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![])),
        battlegroup_states: Rc::new(RefCell::new(vec![])),
    });
    let mut sink = VecOperationSink::default();
    orchestrator
        .restart(
            &BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            &mut sink,
        )
        .unwrap();
    assert_eq!(
        calls.borrow().as_slice(),
        &[
            "funcom-seabass-sh-host-abcdef/sh-host-abcdef:true",
            "funcom-seabass-sh-host-abcdef/sh-host-abcdef:false",
        ]
    );
}

#[test]
fn builds_service_urls_without_shelling_out() {
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes {
        calls: Rc::new(RefCell::new(Vec::new())),
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![Some(32527)])),
        battlegroup_states: Rc::new(RefCell::new(vec![])),
    });
    let bg = BattlegroupRef {
        namespace: "funcom-seabass-sh-host-abcdef".to_string(),
        name: "sh-host-abcdef".to_string(),
    };
    assert_eq!(
        orchestrator.file_browser_url("10.0.0.4").unwrap().url,
        "http://10.0.0.4:18888/"
    );
    assert_eq!(
        orchestrator
            .director_url(&bg, "10.0.0.4")
            .unwrap()
            .unwrap()
            .url,
        "http://10.0.0.4:32527/"
    );
}

#[test]
fn start_waits_for_director_node_port_after_unstop_patch() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes {
        calls: calls.clone(),
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![Some(32527)])),
        battlegroup_states: Rc::new(RefCell::new(vec![running_state()])),
    });
    let mut sink = VecOperationSink::default();
    let port = orchestrator
        .start_and_wait_director(
            &BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            0,
            &mut sink,
        )
        .unwrap();

    assert_eq!(port, Some(32527));
    assert_eq!(
        calls.borrow().as_slice(),
        &["funcom-seabass-sh-host-abcdef/sh-host-abcdef:false"]
    );
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "bg.director.wait-port"));
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "bg.wait-started"));
}

#[test]
fn start_wait_rejects_stopped_phase_even_when_stop_flag_is_false() {
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes {
        calls: Rc::new(RefCell::new(Vec::new())),
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![])),
        battlegroup_states: Rc::new(RefCell::new(vec![BattlegroupState {
            stop: false,
            phase: "Stopped".to_string(),
            server_group_phase: "Stopped".to_string(),
            director_phase: "Suspended".to_string(),
        }])),
    });
    let mut sink = VecOperationSink::default();
    let result = orchestrator.wait_for_battlegroup_started(
        &BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        0,
        &mut sink,
    );

    assert!(result.is_err());
}
