use serde_json::Value;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        OperationSink, OrchestrationEvent, ProviderKind, RemoteCommandRunner, StepAction,
        StepDomain,
    },
    validation::validate_kube_arg,
};

use super::models::{
    ExperimentalSwapRequest, ExperimentalSwapResult, ExperimentalSwapStatus,
    LowMemoryBattlegroupProfileRequest,
};
use super::patch::{
    experimental_swap_patch_operations, experimental_swap_patch_operations_for_swap,
};
use super::scripts::{enable_swap_script, EXPERIMENTAL_SWAP_STATUS_SCRIPT};

/// Enables the guest swap file and patches BattleGroup memory requests/limits.
pub struct ExperimentalSwapOrchestrator<R> {
    runner: R,
}

impl<R> ExperimentalSwapOrchestrator<R>
where
    R: RemoteCommandRunner,
{
    /// Creates an experimental swap orchestrator around a remote guest runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Reads guest swap state and, optionally, BattleGroup memory profile state.
    pub fn status(
        &self,
        battlegroup: Option<(&str, &str)>,
    ) -> CommandResult<ExperimentalSwapStatus> {
        let mut status: ExperimentalSwapStatus = serde_json::from_value(
            self.runner
                .run_json(EXPERIMENTAL_SWAP_STATUS_SCRIPT, "experimental swap status")?,
        )
        .map_err(|err| failure(format!("Failed to parse experimental swap status: {err}")))?;

        if let Some((namespace, battlegroup_name)) = battlegroup {
            validate_kube_arg(namespace, "namespace")?;
            validate_kube_arg(battlegroup_name, "battlegroup name")?;
            let value = self.battlegroup(namespace, battlegroup_name)?;
            status.battlegroup_profile_applied =
                Some(experimental_swap_patch_operations(&value)?.is_empty());
        }

        Ok(status)
    }

    /// Enables swap and applies the experimental low-memory BattleGroup profile.
    pub fn enable(
        &self,
        request: &ExperimentalSwapRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<ExperimentalSwapResult> {
        request.validate()?;

        emit(
            sink,
            "guest-swap.enable",
            "Enabling guest experimental swap.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.runner.run_script(&enable_swap_script(
            request.swap_size_gib,
            request.restart_k3s,
        ))?;

        let operation_count = self.apply_battlegroup_memory_profile(
            &LowMemoryBattlegroupProfileRequest::new(
                &request.namespace,
                &request.battlegroup_name,
                request.swap_size_gib,
            ),
            sink,
        )?;

        emit(
            sink,
            "guest-swap.status",
            "Verifying experimental swap status.",
            StepDomain::Guest,
            StepAction::Check,
        );
        let status = self.status(Some((&request.namespace, &request.battlegroup_name)))?;
        Ok(ExperimentalSwapResult {
            status,
            battlegroup_patch_operations: operation_count,
        })
    }

    /// Applies only the BattleGroup memory profile, without touching swap or k3s.
    pub fn apply_battlegroup_memory_profile(
        &self,
        request: &LowMemoryBattlegroupProfileRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<usize> {
        request.validate()?;

        emit(
            sink,
            "bg-swap.patch-memory",
            "Applying low-memory BattleGroup memory profile.",
            StepDomain::Kubernetes,
            StepAction::Patch,
        );
        let battlegroup = self.battlegroup(&request.namespace, &request.battlegroup_name)?;
        let operations =
            experimental_swap_patch_operations_for_swap(&battlegroup, request.swap_size_gib)?;
        let operation_count = operations.len();
        if !operations.is_empty() {
            let patch = serde_json::to_string(&operations).map_err(|err| {
                failure(format!(
                    "Failed to serialize experimental swap patch: {err}"
                ))
            })?;
            let command = format!(
                "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o json",
                sh_single_quoted(&request.battlegroup_name),
                sh_single_quoted(&request.namespace),
                sh_single_quoted(&patch),
            );
            self.runner
                .run_json(&command, "experimental swap battlegroup patch")?;
        }

        Ok(operation_count)
    }

    fn battlegroup(&self, namespace: &str, battlegroup_name: &str) -> CommandResult<Value> {
        let command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(battlegroup_name),
            sh_single_quoted(namespace),
        );
        self.runner
            .run_json(&command, "experimental swap battlegroup")
    }
}

fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: &str,
    domain: StepDomain,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.to_string(),
        domain,
        action,
        provider: ProviderKind::Ssh,
    });
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    use super::*;

    #[derive(Clone, Default)]
    struct MockRemote {
        outputs: Rc<RefCell<VecDeque<String>>>,
        scripts: Rc<RefCell<Vec<String>>>,
    }

    impl MockRemote {
        fn with_outputs(outputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
            Self {
                outputs: Rc::new(RefCell::new(outputs.into_iter().map(Into::into).collect())),
                scripts: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl RemoteCommandRunner for MockRemote {
        fn run(&self, command: &str) -> CommandResult<String> {
            self.run_script(command)
        }

        fn run_script(&self, script: &str) -> CommandResult<String> {
            self.scripts.borrow_mut().push(script.to_string());
            Ok(self.outputs.borrow_mut().pop_front().unwrap_or_default())
        }
    }

    #[test]
    fn enable_runs_swap_script_and_battlegroup_patch() {
        let remote = MockRemote::with_outputs([
            "",
            r#"{"metadata":{"name":"bg"},"spec":{"serverGroup":{"template":{"spec":{"sets":[{"map":"DeepDesert_1","resources":{"limits":{"memory":"15Gi"},"requests":{"memory":"15Gi"}}}]}}}}}"#,
            r#"{"metadata":{"name":"bg"}}"#,
            r#"{"swapFileExists":true,"swapActive":true,"swapFileBytes":32212254720,"activeSwapBytes":32212254720,"fstabConfigured":true,"openrcSwapEnabled":true,"kubeletSwapConfigured":true,"battlegroupProfileApplied":null}"#,
            r#"{"metadata":{"name":"bg"},"spec":{"serverGroup":{"template":{"spec":{"sets":[{"map":"DeepDesert_1","resources":{"limits":{"memory":"10Gi"},"requests":{"memory":"3Gi"}}}]}}}}}"#,
        ]);
        let scripts = remote.scripts.clone();
        let mut sink = crate::orchestration::VecOperationSink::default();

        let result = ExperimentalSwapOrchestrator::new(remote)
            .enable(
                &ExperimentalSwapRequest::new("funcom-seabass-sh-host-abcdef", "bg"),
                &mut sink,
            )
            .unwrap();

        assert_eq!(result.battlegroup_patch_operations, 2);
        let scripts = scripts.borrow().join("\n");
        assert!(scripts.contains("dd if=/dev/zero of=/swapfile"));
        assert!(scripts.contains("kubectl patch battlegroup"));
        assert!(sink
            .events
            .iter()
            .any(|event| event.step_id == "bg-swap.patch-memory"));
    }
}
