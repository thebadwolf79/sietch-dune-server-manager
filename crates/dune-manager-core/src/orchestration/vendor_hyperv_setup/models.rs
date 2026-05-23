//! Public data types exchanged with the vendor Hyper-V setup runner.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Request used to answer the vendor Hyper-V setup prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorHyperVSetupRequest {
    /// Preferred VM destination from the UI. The vendor script only uses the drive.
    pub vm_destination: PathBuf,
    /// Selected network adapter name from the UI.
    pub adapter_name: String,
    /// Requested VM memory in GiB.
    pub memory_gb: u64,
    /// Whether the guest should use static networking.
    pub static_network: bool,
    /// Static guest IP when static networking is selected.
    pub static_ip: String,
    /// Static gateway when static networking is selected.
    pub gateway: String,
    /// Static DNS server when static networking is selected.
    pub dns: String,
    /// Player-facing IP written to server settings.
    pub player_ip: String,
    /// World name for downstream vendor setup prompts.
    pub world_name: String,
    /// Region name for downstream vendor setup prompts.
    pub region: String,
    /// Self-host token for downstream vendor setup prompts.
    pub self_host_token: String,
    /// Whether to accept the low-memory experimental swap prompt.
    pub enable_swap: bool,
}

impl VendorHyperVSetupRequest {
    /// Returns the drive letter the vendor script can use for installation.
    pub fn preferred_drive_name(&self) -> Option<String> {
        self.vm_destination
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_string_lossy().chars().next())
            .filter(|character| character.is_ascii_alphabetic())
            .map(|character| character.to_ascii_uppercase().to_string())
    }
}

/// Result from running the vendor Hyper-V setup script.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorHyperVSetupResult {
    /// Vendor script path that was executed.
    pub script_path: PathBuf,
    /// SHA-256 of the vendor script at execution time.
    pub script_sha256: String,
}

/// Prompt/answer row emitted by the dry-run harness and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorPromptAnswer {
    /// Stable prompt identifier.
    pub prompt_id: &'static str,
    /// Redacted answer value.
    pub answer: String,
}
