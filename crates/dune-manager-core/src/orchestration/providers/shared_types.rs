//! Shared data models exchanged between provider traits.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::CommandResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Normalized VM power state across host providers.
pub enum VmPowerState {
    /// No VM record was found.
    Missing,
    /// VM is powered off.
    Off,
    /// VM is transitioning to running.
    Starting,
    /// VM is running.
    Running,
    /// VM is transitioning to off.
    Stopping,
    /// VM is saved/checkpointed by Hyper-V.
    Saved,
    /// VM is paused.
    Paused,
    /// Hyper-V returned an unrecognized state.
    Other,
}

impl VmPowerState {
    /// Maps a raw Hyper-V state string into the normalized enum.
    pub fn from_hyperv_state(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "" => Self::Missing,
            "off" => Self::Off,
            "starting" => Self::Starting,
            "running" => Self::Running,
            "stopping" => Self::Stopping,
            "saved" => Self::Saved,
            "paused" => Self::Paused,
            _ => Self::Other,
        }
    }
}

/// Host readiness information required for Hyper-V setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReadiness {
    /// Whether the current process has elevated privileges.
    pub elevated: bool,
    /// Whether the Hyper-V PowerShell module is available.
    pub hyperv_available: bool,
    /// Whether the Hyper-V VM management service is running.
    pub vmms_running: bool,
    /// Whether firmware virtualization is enabled, when the host can report it.
    pub virtualization_firmware_enabled: Option<bool>,
    /// Total physical memory on the host in bytes.
    pub total_physical_memory_bytes: u64,
    /// Currently available physical memory on the host in bytes.
    pub available_physical_memory_bytes: u64,
    /// Logical processor count reported by the host.
    pub logical_processor_count: u32,
}

/// Host drive candidate suitable for placing VM files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveCandidate {
    /// Display name for the drive.
    pub name: String,
    /// Root path such as `C:\`.
    pub root: String,
    /// Available free space in bytes.
    pub free_bytes: u64,
}

/// Physical IPv4 network adapter candidate for a Hyper-V external switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapterCandidate {
    /// Adapter name used by Hyper-V commands.
    pub name: String,
    /// Adapter hardware/interface description.
    pub interface_description: String,
    /// Active IPv4 address on the adapter.
    pub ipv4_address: String,
    /// IPv4 CIDR prefix length.
    pub prefix_length: u8,
    /// Default gateway for the adapter.
    pub gateway: String,
    /// Suggested static IPv4 address for a VM on this adapter subnet.
    pub suggested_ipv4_address: String,
    /// Existing external switch bound to this adapter, if any.
    pub existing_external_switch: String,
}

/// Hyper-V external switch record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSwitch {
    /// Switch name.
    pub name: String,
    /// Adapter description backing the switch.
    pub net_adapter_interface_description: String,
}

/// VM inventory snapshot from the host provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmInventoryRecord {
    /// VM name.
    pub name: String,
    /// Normalized power state.
    pub state: VmPowerState,
    /// Raw provider-specific power state.
    pub raw_state: String,
    /// Hyper-V configuration location.
    pub configuration_location: String,
    /// VM path.
    pub path: String,
    /// Assigned memory in bytes.
    pub memory_assigned_bytes: u64,
    /// Virtual processor count assigned to the VM.
    pub processor_count: u32,
    /// Uptime in seconds.
    pub uptime_seconds: u64,
    /// IPv4 addresses reported for the VM.
    pub ipv4_addresses: Vec<String>,
    /// Attached virtual hard disk paths.
    pub hard_disk_paths: Vec<String>,
    /// Sum of attached virtual hard disk maximum sizes in bytes.
    pub disk_size_bytes: u64,
    /// Sum of attached virtual hard disk file sizes in bytes.
    pub disk_file_size_bytes: u64,
    /// Connected Hyper-V switch names.
    pub switch_names: Vec<String>,
}

/// Compatibility result for importing a packaged VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmCompatibilityReport {
    /// Whether the VM can be imported.
    pub compatible: bool,
    /// Human-readable incompatibility reasons.
    pub incompatibilities: Vec<String>,
}

/// Result of importing a VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedVm {
    /// Imported VM name.
    pub name: String,
    /// Hyper-V configuration location.
    pub configuration_location: String,
}

/// Request to import a packaged VM into a destination directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmImportRequest {
    /// Source `.vmcx` path.
    pub vmcx_path: String,
    /// Destination path for the imported VM files.
    pub destination_path: String,
}

/// Request to create or locate a Hyper-V external switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsureSwitchRequest {
    /// Desired switch name.
    pub switch_name: String,
    /// Host adapter to bind.
    pub adapter_name: String,
}

/// Static guest network configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestNetworkConfig {
    /// Guest interface name.
    pub interface: String,
    /// Static address with CIDR prefix.
    pub address_cidr: String,
    /// Static gateway.
    pub gateway: String,
    /// DNS server list or value.
    pub dns: String,
}

/// Minimal BattleGroup lifecycle state used by start/restart waits.
///
/// The phase fields mirror the columns shown by the vendor
/// `/home/dune/.dune/bin/battlegroup status` wrapper. `stop` is read separately
/// from `.spec.stop` because the wrapper does not surface it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BattlegroupState {
    /// Vendor stop flag from `spec.stop`.
    pub stop: bool,
    /// Top-level BattleGroup status phase (the wrapper's `Status` column).
    pub phase: String,
    /// Database phase from the wrapper's `Database` column.
    pub database_phase: String,
    /// Gateway phase from the wrapper's `Gateway` column.
    ///
    /// Kept under the old `server_group_phase` name in the struct for
    /// backwards compatibility with existing callers.
    pub server_group_phase: String,
    /// Director phase from the wrapper's `Director` column.
    pub director_phase: String,
    /// Optional uptime string from the wrapper's `Uptime` column.
    pub uptime: String,
    /// Per-map server stats parsed from the wrapper's `Game Servers` table.
    pub server_stats: Vec<ServerStatRow>,
}

/// One row of the vendor wrapper's `Game Servers` table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerStatRow {
    /// Map name (e.g. `Survival_1`, `DeepDesert_1`, `SH_Arrakeen`).
    pub map: String,
    /// Server pod phase.
    pub phase: String,
    /// Pod readiness as reported by the wrapper.
    pub ready: String,
    /// Connected players count.
    pub players: String,
    /// Pod age.
    pub age: String,
}

/// Request to render and create a world manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldManifestRequest {
    /// User-facing world name.
    pub world_name: String,
    /// Vendor region label.
    pub world_region: String,
    /// Player-facing IPv4 address advertised through gateway metadata.
    pub player_ip: String,
    /// Unique Kubernetes battlegroup/world name.
    pub world_unique_name: String,
    /// Self-host token used by the vendor manifest.
    pub self_host_token: String,
}

/// Result of creating world resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedWorld {
    /// Created namespace.
    pub namespace: String,
    /// Created battlegroup resource name.
    pub battlegroup_name: String,
}

/// Returns packaged `.vmcx` candidates under a server install path.
pub fn packaged_vmcx_candidates(install_path: &Path) -> CommandResult<Vec<String>> {
    let vm_dir = install_path.join("Virtual Machines");
    let entries = std::fs::read_dir(&vm_dir).map_err(|err| {
        crate::errors::failure(format!("Failed to read {}: {err}", vm_dir.display()))
    })?;
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("vmcx"))
        })
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_hyperv_power_states() {
        assert_eq!(
            VmPowerState::from_hyperv_state("Running"),
            VmPowerState::Running
        );
        assert_eq!(
            VmPowerState::from_hyperv_state("Starting"),
            VmPowerState::Starting
        );
        assert_eq!(VmPowerState::from_hyperv_state("Off"), VmPowerState::Off);
        assert_eq!(
            VmPowerState::from_hyperv_state("SomethingElse"),
            VmPowerState::Other
        );
    }
}
