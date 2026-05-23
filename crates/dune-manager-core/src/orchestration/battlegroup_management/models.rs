use serde::Serialize;

use crate::{
    errors::failure, models::CommandResult, orchestration::BattlegroupState,
    validation::validate_kube_arg,
};

/// Names a live BattleGroup custom resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattlegroupRef {
    /// Kubernetes namespace containing the BattleGroup.
    pub namespace: String,
    /// BattleGroup resource name.
    pub name: String,
}

impl BattlegroupRef {
    /// Validates the namespace and resource name for safe kubectl usage.
    pub fn validate(&self) -> CommandResult<()> {
        validate_kube_arg(&self.namespace, "namespace")?;
        validate_kube_arg(&self.name, "battlegroup name")?;
        Ok(())
    }
}

/// Browser-openable URL for a service exposed from the VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceUrl {
    /// Fully qualified HTTP URL.
    pub url: String,
}

/// Returns whether the live BattleGroup state is operational enough to treat as started.
pub fn is_started_state(state: &BattlegroupState) -> bool {
    !state.stop
        && is_started_phase(&state.phase)
        && is_started_phase(&state.server_group_phase)
        && is_director_ready_phase(&state.director_phase)
}

fn is_started_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "running" | "ready" | "healthy" | "available" | "reconciling"
    )
}

fn is_director_ready_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "running" | "ready" | "healthy" | "available" | "reconciling"
        )
}

pub(super) fn validate_ipv4ish(value: &str, label: &str) -> CommandResult<()> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok()) {
        Ok(())
    } else {
        Err(failure(format!("{label} must be an IPv4 address")))
    }
}
