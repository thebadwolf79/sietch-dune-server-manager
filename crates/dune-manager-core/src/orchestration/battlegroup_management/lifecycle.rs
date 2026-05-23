use std::{thread, time::Duration};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        BattlegroupState, KubernetesProvider, OperationSink, OrchestrationEvent, ProviderKind,
        StepAction, StepDomain,
    },
};

use super::models::{is_started_state, validate_ipv4ish, BattlegroupRef, ServiceUrl};

/// Performs routine BattleGroup lifecycle operations through Kubernetes.
pub struct BattlegroupManagementOrchestrator<K> {
    kubernetes: K,
}

impl<K> BattlegroupManagementOrchestrator<K>
where
    K: KubernetesProvider,
{
    /// Creates an orchestrator around a Kubernetes provider.
    pub fn new(kubernetes: K) -> Self {
        Self { kubernetes }
    }

    /// Starts a BattleGroup by clearing the vendor stop flag.
    pub fn start(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(sink, "bg.start", "Starting battlegroup.", StepAction::Start);
        self.kubernetes
            .patch_battlegroup_stop(&battlegroup.namespace, &battlegroup.name, false)
    }

    /// Starts a BattleGroup and waits for the Director NodePort to appear.
    pub fn start_and_wait_director(
        &self,
        battlegroup: &BattlegroupRef,
        timeout_seconds: u64,
        sink: &mut impl OperationSink,
    ) -> CommandResult<Option<u16>> {
        self.start(battlegroup, sink)?;
        self.wait_for_battlegroup_started(battlegroup, timeout_seconds, sink)?;
        self.wait_for_director_node_port(battlegroup, timeout_seconds, sink)
    }

    /// Stops a BattleGroup by setting the vendor stop flag.
    pub fn stop(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(sink, "bg.stop", "Stopping battlegroup.", StepAction::Stop);
        self.kubernetes
            .patch_battlegroup_stop(&battlegroup.namespace, &battlegroup.name, true)
    }

    /// Restarts a BattleGroup by applying stop and start patches in order.
    pub fn restart(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.restart.stop",
            "Stopping battlegroup for restart.",
            StepAction::Stop,
        );
        self.kubernetes
            .patch_battlegroup_stop(&battlegroup.namespace, &battlegroup.name, true)?;
        emit(
            sink,
            "bg.restart.start",
            "Starting battlegroup after restart.",
            StepAction::Start,
        );
        self.kubernetes
            .patch_battlegroup_stop(&battlegroup.namespace, &battlegroup.name, false)
    }

    /// Restarts a BattleGroup and waits for the Director NodePort to appear.
    pub fn restart_and_wait_director(
        &self,
        battlegroup: &BattlegroupRef,
        timeout_seconds: u64,
        sink: &mut impl OperationSink,
    ) -> CommandResult<Option<u16>> {
        self.restart(battlegroup, sink)?;
        self.wait_for_battlegroup_started(battlegroup, timeout_seconds, sink)?;
        self.wait_for_director_node_port(battlegroup, timeout_seconds, sink)
    }

    /// Polls Kubernetes until the BattleGroup moves out of a stopped state.
    pub fn wait_for_battlegroup_started(
        &self,
        battlegroup: &BattlegroupRef,
        timeout_seconds: u64,
        sink: &mut impl OperationSink,
    ) -> CommandResult<BattlegroupState> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.wait-started",
            "Waiting for battlegroup to leave stopped state.",
            StepAction::Wait,
        );
        let mut elapsed = 0;
        let mut last = None;
        while elapsed <= timeout_seconds {
            let state = self
                .kubernetes
                .battlegroup_state(&battlegroup.namespace, &battlegroup.name)?;
            if is_started_state(&state) {
                return Ok(state);
            }
            last = Some(state);
            thread::sleep(Duration::from_secs(2));
            elapsed += 2;
        }
        let detail = last
            .map(|state| {
                format!(
                    "last phase={}, stop={}, serverGroup={}, director={}",
                    state.phase, state.stop, state.server_group_phase, state.director_phase
                )
            })
            .unwrap_or_else(|| "no BattleGroup state was read".to_string());
        Err(failure(format!(
            "BattleGroup did not leave stopped state within {timeout_seconds}s ({detail})"
        )))
    }

    /// Builds the file-browser URL for a VM IP.
    pub fn file_browser_url(&self, vm_ip: &str) -> CommandResult<ServiceUrl> {
        validate_ipv4ish(vm_ip, "VM IP")?;
        Ok(ServiceUrl {
            url: format!("http://{vm_ip}:18888/"),
        })
    }

    /// Discovers and builds the Director URL for a BattleGroup, if exposed.
    pub fn director_url(
        &self,
        battlegroup: &BattlegroupRef,
        vm_ip: &str,
    ) -> CommandResult<Option<ServiceUrl>> {
        battlegroup.validate()?;
        validate_ipv4ish(vm_ip, "VM IP")?;
        let Some(port) = self.kubernetes.director_node_port(&battlegroup.namespace)? else {
            return Ok(None);
        };
        Ok(Some(ServiceUrl {
            url: format!("http://{vm_ip}:{port}/"),
        }))
    }

    /// Returns the only BattleGroup namespace when exactly one is present.
    pub fn discover_single_battlegroup_namespace(&self) -> CommandResult<Option<String>> {
        let namespaces = self.kubernetes.list_battlegroup_namespaces()?;
        match namespaces.as_slice() {
            [] => Ok(None),
            [namespace] => Ok(Some(namespace.clone())),
            _ => Err(failure("Multiple battlegroup namespaces were found")),
        }
    }

    /// Polls Kubernetes until the Director service has a NodePort or times out.
    pub fn wait_for_director_node_port(
        &self,
        battlegroup: &BattlegroupRef,
        timeout_seconds: u64,
        sink: &mut impl OperationSink,
    ) -> CommandResult<Option<u16>> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.director.wait-port",
            "Waiting for Director service port.",
            StepAction::Wait,
        );
        let mut elapsed = 0;
        while elapsed <= timeout_seconds {
            if let Some(port) = self.kubernetes.director_node_port(&battlegroup.namespace)? {
                return Ok(Some(port));
            }
            thread::sleep(Duration::from_secs(2));
            elapsed += 2;
        }
        Ok(None)
    }
}

pub(super) fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: impl Into<String>,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain: StepDomain::Kubernetes,
        action,
        provider: ProviderKind::Kubernetes,
    });
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
