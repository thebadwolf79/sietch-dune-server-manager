use serde::{Deserialize, Serialize};

use crate::{errors::failure, models::CommandResult, validation::validate_kube_arg};

/// Request for enabling the vendor experimental low-memory profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentalSwapRequest {
    /// Kubernetes namespace containing the BattleGroup.
    pub namespace: String,
    /// BattleGroup resource name.
    pub battlegroup_name: String,
    /// Swap file size in GiB.
    pub swap_size_gib: u64,
    /// Whether k3s should be restarted to apply kubelet swap settings.
    pub restart_k3s: bool,
}

impl ExperimentalSwapRequest {
    /// Creates a request using the vendor-style 30 GiB swap file and k3s restart.
    pub fn new(namespace: impl Into<String>, battlegroup_name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            battlegroup_name: battlegroup_name.into(),
            swap_size_gib: 30,
            restart_k3s: true,
        }
    }

    pub(super) fn validate(&self) -> CommandResult<()> {
        validate_kube_arg(&self.namespace, "namespace")?;
        validate_kube_arg(&self.battlegroup_name, "battlegroup name")?;
        if !(1..=256).contains(&self.swap_size_gib) {
            return Err(failure("--swap-size-gib must be between 1 and 256"));
        }
        Ok(())
    }
}

/// Request for applying the low-memory BattleGroup resource profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowMemoryBattlegroupProfileRequest {
    /// Kubernetes namespace containing the BattleGroup.
    pub namespace: String,
    /// BattleGroup resource name.
    pub battlegroup_name: String,
    /// Swap file size in GiB used to choose the profile strength.
    pub swap_size_gib: u64,
}

impl LowMemoryBattlegroupProfileRequest {
    /// Creates a low-memory resource profile request.
    pub fn new(
        namespace: impl Into<String>,
        battlegroup_name: impl Into<String>,
        swap_size_gib: u64,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            battlegroup_name: battlegroup_name.into(),
            swap_size_gib,
        }
    }

    pub(super) fn validate(&self) -> CommandResult<()> {
        validate_kube_arg(&self.namespace, "namespace")?;
        validate_kube_arg(&self.battlegroup_name, "battlegroup name")?;
        if !(1..=256).contains(&self.swap_size_gib) {
            return Err(failure("swap size must be between 1 and 256 GiB"));
        }
        Ok(())
    }
}

/// Snapshot of the guest experimental swap state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalSwapStatus {
    /// Whether `/swapfile` exists.
    pub swap_file_exists: bool,
    /// Whether `/swapfile` is currently active.
    pub swap_active: bool,
    /// Configured `/swapfile` size in bytes, when known.
    pub swap_file_bytes: Option<u64>,
    /// Active swap size in bytes, when active and reported by the kernel.
    pub active_swap_bytes: Option<u64>,
    /// Whether `/etc/fstab` contains a `/swapfile` entry.
    pub fstab_configured: bool,
    /// Whether OpenRC has the swap service enabled.
    pub openrc_swap_enabled: bool,
    /// Whether the k3s kubelet config enables `failSwapOn: false`.
    pub kubelet_swap_configured: bool,
    /// Whether the BattleGroup memory profile already matches this experimental profile.
    pub battlegroup_profile_applied: Option<bool>,
}

/// Result of applying the experimental swap profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalSwapResult {
    /// Status after applying the profile.
    pub status: ExperimentalSwapStatus,
    /// Number of JSON Patch operations applied to the BattleGroup resource.
    pub battlegroup_patch_operations: usize,
}
