use std::{cell::RefCell, rc::Rc};

use crate::orchestration::{
    BattlegroupWrapperOps, KubernetesProvider, VecOperationSink, WrapperOutcome,
};

use super::*;

#[derive(Default)]
struct MockKubernetes {
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
        _namespace: &str,
        _name: &str,
        _stop: bool,
    ) -> CommandResult<()> {
        Ok(())
    }

    fn battlegroup_state(&self, _namespace: &str, _name: &str) -> CommandResult<BattlegroupState> {
        Ok(self
            .battlegroup_states
            .borrow_mut()
            .pop()
            .unwrap_or_default())
    }

    fn director_node_port(&self, _namespace: &str) -> CommandResult<Option<u16>> {
        Ok(self.director_ports.borrow_mut().pop().unwrap_or(None))
    }
}

#[derive(Default)]
struct MockWrapper {
    calls: Rc<RefCell<Vec<String>>>,
    statuses: Rc<RefCell<Vec<BattlegroupState>>>,
}

impl MockWrapper {
    fn record(&self, action: &str, bg: &BattlegroupRef) -> WrapperOutcome {
        self.calls
            .borrow_mut()
            .push(format!("{action}:{}/{}", bg.namespace, bg.name));
        WrapperOutcome {
            action: match action {
                "start" => crate::orchestration::WrapperAction::Start,
                "stop" => crate::orchestration::WrapperAction::Stop,
                "restart" => crate::orchestration::WrapperAction::Restart,
                "update" => crate::orchestration::WrapperAction::Update,
                _ => crate::orchestration::WrapperAction::Status,
            },
            stdout: String::new(),
        }
    }
}

impl BattlegroupWrapperOps for MockWrapper {
    fn status(&self, _battlegroup: &BattlegroupRef) -> CommandResult<BattlegroupState> {
        Ok(self
            .statuses
            .borrow_mut()
            .pop()
            .unwrap_or_else(running_state))
    }

    fn start(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        Ok(self.record("start", battlegroup))
    }

    fn stop(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        Ok(self.record("stop", battlegroup))
    }

    fn restart(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        Ok(self.record("restart", battlegroup))
    }

    fn update(&self, battlegroup: &BattlegroupRef) -> CommandResult<WrapperOutcome> {
        Ok(self.record("update", battlegroup))
    }
}

fn running_state() -> BattlegroupState {
    BattlegroupState {
        phase: "Running".to_string(),
        database_phase: "Running".to_string(),
        server_group_phase: "Running".to_string(),
        director_phase: "Healthy".to_string(),
        ..BattlegroupState::default()
    }
}

fn sample_bg() -> BattlegroupRef {
    BattlegroupRef {
        namespace: "funcom-seabass-sh-host-abcdef".to_string(),
        name: "sh-host-abcdef".to_string(),
    }
}

#[test]
fn restart_invokes_wrapper_restart_once() {
    let wrapper = MockWrapper::default();
    let calls = wrapper.calls.clone();
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes::default(), wrapper);
    let mut sink = VecOperationSink::default();
    orchestrator.restart(&sample_bg(), &mut sink).unwrap();
    assert_eq!(
        calls.borrow().as_slice(),
        &["restart:funcom-seabass-sh-host-abcdef/sh-host-abcdef"]
    );
}

#[test]
fn update_invokes_wrapper_update() {
    let wrapper = MockWrapper::default();
    let calls = wrapper.calls.clone();
    let orchestrator = BattlegroupManagementOrchestrator::new(MockKubernetes::default(), wrapper);
    let mut sink = VecOperationSink::default();
    orchestrator.update(&sample_bg(), &mut sink).unwrap();
    assert_eq!(
        calls.borrow().as_slice(),
        &["update:funcom-seabass-sh-host-abcdef/sh-host-abcdef"]
    );
}

#[test]
fn builds_service_urls_without_shelling_out() {
    let kubernetes = MockKubernetes {
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![Some(32527)])),
        battlegroup_states: Rc::new(RefCell::new(vec![])),
    };
    let orchestrator = BattlegroupManagementOrchestrator::new(kubernetes, MockWrapper::default());
    assert_eq!(
        orchestrator.file_browser_url("10.0.0.4").unwrap().url,
        "http://10.0.0.4:18888/"
    );
    assert_eq!(
        orchestrator
            .director_url(&sample_bg(), "10.0.0.4")
            .unwrap()
            .unwrap()
            .url,
        "http://10.0.0.4:32527/"
    );
}

#[test]
fn start_waits_for_director_node_port_after_wrapper_start() {
    let kubernetes = MockKubernetes {
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![Some(32527)])),
        battlegroup_states: Rc::new(RefCell::new(vec![running_state()])),
    };
    let wrapper = MockWrapper {
        statuses: Rc::new(RefCell::new(vec![running_state()])),
        ..Default::default()
    };
    let calls = wrapper.calls.clone();
    let orchestrator = BattlegroupManagementOrchestrator::new(kubernetes, wrapper);
    let mut sink = VecOperationSink::default();
    let port = orchestrator
        .start_and_wait_director(&sample_bg(), 0, &mut sink)
        .unwrap();

    assert_eq!(port, Some(32527));
    assert_eq!(
        calls.borrow().as_slice(),
        &["start:funcom-seabass-sh-host-abcdef/sh-host-abcdef"]
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
    // The wait reads from Kubernetes (the stable schema), so the stopped
    // state must come from the kubernetes mock, not the vendor wrapper.
    let stopped_state = BattlegroupState {
        phase: "Stopped".to_string(),
        server_group_phase: "Stopped".to_string(),
        director_phase: "Suspended".to_string(),
        ..BattlegroupState::default()
    };
    let kubernetes = MockKubernetes {
        battlegroup_states: Rc::new(RefCell::new(vec![stopped_state])),
        ..Default::default()
    };
    let orchestrator = BattlegroupManagementOrchestrator::new(kubernetes, MockWrapper::default());
    let mut sink = VecOperationSink::default();
    let result = orchestrator.wait_for_battlegroup_started(&sample_bg(), 0, &mut sink);
    assert!(result.is_err());
}

#[test]
fn start_wait_accepts_reconciling_state_from_kubernetes() {
    // #19/#20: a BG up but lingering at phase=Reconciling (serverGroupPhase
    // Running, director Healthy) must satisfy the wait instead of stalling
    // until timeout. The wrapper status is deliberately left as the broken
    // shape it would produce in the field to prove it is no longer consulted.
    let reconciling = BattlegroupState {
        phase: "Reconciling".to_string(),
        server_group_phase: "Running".to_string(),
        director_phase: "Healthy".to_string(),
        ..BattlegroupState::default()
    };
    let kubernetes = MockKubernetes {
        battlegroup_states: Rc::new(RefCell::new(vec![reconciling])),
        ..Default::default()
    };
    let broken_wrapper_state = BattlegroupState {
        phase: "World".to_string(),
        server_group_phase: "Ready".to_string(),
        director_phase: "2/2".to_string(),
        ..BattlegroupState::default()
    };
    let wrapper = MockWrapper {
        statuses: Rc::new(RefCell::new(vec![broken_wrapper_state])),
        ..Default::default()
    };
    let orchestrator = BattlegroupManagementOrchestrator::new(kubernetes, wrapper);
    let mut sink = VecOperationSink::default();
    let state = orchestrator
        .wait_for_battlegroup_started(&sample_bg(), 0, &mut sink)
        .expect("reconciling BG should satisfy the wait");
    assert_eq!(state.phase, "Reconciling");
}

#[test]
fn status_overlays_spec_stop_from_kubernetes() {
    let kubernetes = MockKubernetes {
        namespaces: vec![],
        director_ports: Rc::new(RefCell::new(vec![])),
        battlegroup_states: Rc::new(RefCell::new(vec![BattlegroupState {
            stop: true,
            ..BattlegroupState::default()
        }])),
    };
    let wrapper = MockWrapper {
        statuses: Rc::new(RefCell::new(vec![running_state()])),
        ..Default::default()
    };
    let orchestrator = BattlegroupManagementOrchestrator::new(kubernetes, wrapper);
    let state = orchestrator.status(&sample_bg()).unwrap();
    assert!(
        state.stop,
        "spec.stop overlay should win even if wrapper says running"
    );
    assert_eq!(state.phase, "Running");
}
