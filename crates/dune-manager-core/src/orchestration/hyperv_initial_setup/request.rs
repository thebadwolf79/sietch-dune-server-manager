use serde::Serialize;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        GuestBootstrapPlan, GuestBootstrapResult, GuestNetworkConfig, HyperVVmSetupRequest,
        HyperVVmSetupResult,
    },
};

/// Guest network mode applied during initial setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestNetworkPlan {
    /// Keep the VM on DHCP after first boot.
    Dhcp,
    /// Reconfigure the guest to use a static address.
    Static(GuestNetworkConfig),
}

/// Full request for creating the Hyper-V VM and bootstrapping the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperVInitialSetupRequest {
    /// Host-side Hyper-V import and VM preparation request.
    pub vm: HyperVVmSetupRequest,
    /// Network configuration to apply after the first DHCP boot.
    pub guest_network: GuestNetworkPlan,
    /// Guest bootstrap plan used after SSH becomes available.
    pub guest_bootstrap: GuestBootstrapPlan,
    /// Seconds to wait for Hyper-V to report a guest IPv4 address.
    pub vm_ip_timeout_seconds: u64,
    /// Seconds to wait for SSH reachability.
    pub ssh_timeout_seconds: u64,
}

impl HyperVInitialSetupRequest {
    /// Validates the complete initial setup request.
    pub fn validate(&self) -> CommandResult<()> {
        self.vm.validate()?;
        self.guest_bootstrap.validate()?;
        if self.vm_ip_timeout_seconds == 0 {
            return Err(failure("VM IP timeout must be greater than zero"));
        }
        if self.ssh_timeout_seconds == 0 {
            return Err(failure("SSH timeout must be greater than zero"));
        }
        Ok(())
    }
}

/// Result of a completed Hyper-V initial setup flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperVInitialSetupResult {
    /// Host-side VM setup result.
    pub vm: HyperVVmSetupResult,
    /// Final guest IP used for bootstrap.
    pub guest_ip: String,
    /// Guest bootstrap result.
    pub bootstrap: GuestBootstrapResult,
}

/// Suggested addresses for player-facing server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAddressCandidates {
    /// LAN address currently assigned to the guest.
    pub guest_lan_ip: String,
    /// Public address detected from inside the guest, when reachable.
    pub public_ip: Option<String>,
}
