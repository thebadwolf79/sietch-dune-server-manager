//! Kubernetes operations provider trait used by battlegroup orchestration.

use crate::models::CommandResult;
use crate::orchestration::providers::shared_types::BattlegroupState;

/// Kubernetes operations needed by battlegroup lifecycle orchestration.
pub trait KubernetesProvider {
    /// Lists battlegroup namespaces.
    fn list_battlegroup_namespaces(&self) -> CommandResult<Vec<String>>;
    /// Patches the battlegroup stop flag.
    fn patch_battlegroup_stop(&self, namespace: &str, name: &str, stop: bool) -> CommandResult<()>;
    /// Returns the current BattleGroup lifecycle state.
    fn battlegroup_state(&self, namespace: &str, name: &str) -> CommandResult<BattlegroupState>;
    /// Returns the Director NodePort for a namespace, when present.
    fn director_node_port(&self, namespace: &str) -> CommandResult<Option<u16>>;
}
