use std::{thread, time::Duration};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        BattlegroupState, BattlegroupWrapperOps, KubernetesProvider, OperationSink,
        OrchestrationEvent, ProviderKind, StepAction, StepDomain,
    },
};

use super::models::{is_started_state, validate_ipv4ish, BattlegroupRef, ServiceUrl};

/// Performs routine BattleGroup lifecycle operations.
///
/// Start/stop/restart/update are delegated to the vendor wrapper at
/// `/home/dune/.dune/bin/battlegroup`. Waits and admin URL discovery use the
/// Kubernetes provider directly because the wrapper does not expose those.
pub struct BattlegroupManagementOrchestrator<K, W> {
    kubernetes: K,
    wrapper: W,
}

impl<K, W> BattlegroupManagementOrchestrator<K, W>
where
    K: KubernetesProvider,
    W: BattlegroupWrapperOps,
{
    /// Creates an orchestrator from a Kubernetes provider and vendor wrapper.
    pub fn new(kubernetes: K, wrapper: W) -> Self {
        Self {
            kubernetes,
            wrapper,
        }
    }

    /// Borrows the underlying Kubernetes provider.
    pub fn kubernetes(&self) -> &K {
        &self.kubernetes
    }

    /// Borrows the underlying vendor wrapper.
    pub fn wrapper(&self) -> &W {
        &self.wrapper
    }

    /// Starts a BattleGroup via the vendor wrapper's `start` action.
    pub fn start(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(sink, "bg.start", "Starting battlegroup.", StepAction::Start);
        self.wrapper.start(battlegroup).map(|_| ())
    }

    /// Stops a BattleGroup via the vendor wrapper's `stop` action.
    pub fn stop(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(sink, "bg.stop", "Stopping battlegroup.", StepAction::Stop);
        self.wrapper.stop(battlegroup).map(|_| ())
    }

    /// Restarts a BattleGroup via the vendor wrapper's `restart` action.
    pub fn restart(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.restart",
            "Restarting battlegroup.",
            StepAction::Start,
        );
        self.wrapper.restart(battlegroup).map(|_| ())
    }

    /// Updates a BattleGroup via the vendor wrapper's `update` action.
    ///
    /// The wrapper runs steamcmd, refreshes operators and map manifests,
    /// loads new images via `ctr`, and patches the battlegroup's image fields.
    /// This call blocks for the full duration of the update.
    pub fn update(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<String> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.update",
            "Updating battlegroup via vendor wrapper.",
            StepAction::Patch,
        );
        let outcome = self.wrapper.update(battlegroup)?;
        Ok(outcome.stdout)
    }

    /// Reads battlegroup state via the wrapper's `status` action.
    ///
    /// The wrapper does not surface `.spec.stop`; this method overlays it from
    /// the Kubernetes provider so callers get a complete state in one call.
    pub fn status(&self, battlegroup: &BattlegroupRef) -> CommandResult<BattlegroupState> {
        battlegroup.validate()?;
        let mut state = self.wrapper.status(battlegroup)?;
        match self
            .kubernetes
            .battlegroup_state(&battlegroup.namespace, &battlegroup.name)
        {
            Ok(kube) => state.stop = kube.stop,
            Err(err) => {
                // The wrapper's status is the source of truth; if .spec.stop
                // read fails (e.g., RBAC), prefer a `stop=true` fallback when
                // the status row clearly says the BG is stopped.
                if status_phase_looks_stopped(&state.phase) {
                    state.stop = true;
                }
                let _ = err;
            }
        }
        Ok(state)
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

    /// Polls the wrapper's status until the BattleGroup leaves a stopped state
    /// or the timeout expires.
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
            // Read started-ness from the stable Kubernetes schema, not the
            // vendor wrapper's `status` text. That text layout drifts across
            // Funcom releases (status="World", director="2/2", etc.) and was
            // misparsed into unrecognised phases, so the wait never saw the BG
            // as started and stalled until timeout even when it was up (#19).
            match self
                .kubernetes
                .battlegroup_state(&battlegroup.namespace, &battlegroup.name)
            {
                Ok(state) => {
                    if is_started_state(&state) {
                        return Ok(state);
                    }
                    last = Some(state);
                }
                Err(_) => {
                    // Keep polling on transient errors; kubectl can briefly
                    // fail while the BG is reconciling.
                }
            }
            thread::sleep(Duration::from_secs(2));
            elapsed += 2;
        }
        let detail = last
            .map(|state| {
                format!(
                    "last status={}, stop={}, database={}, gateway={}, director={}",
                    state.phase,
                    state.stop,
                    state.database_phase,
                    state.server_group_phase,
                    state.director_phase
                )
            })
            .unwrap_or_else(|| "no BattleGroup status was read".to_string());
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

fn status_phase_looks_stopped(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "stopped" | "suspended" | "notready" | "not_ready"
    )
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
