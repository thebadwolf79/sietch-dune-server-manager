//! Guest VM access provider trait.

use crate::models::CommandResult;
use crate::orchestration::providers::shared_types::GuestNetworkConfig;

/// Guest VM access provider.
pub trait GuestProvider {
    /// Waits for SSH to become reachable.
    fn wait_for_ssh(&self, ip: &str, timeout_seconds: u64) -> CommandResult<()>;
    /// Uploads bytes to a guest path with a file mode.
    fn upload_bytes(
        &self,
        ip: &str,
        remote_path: &str,
        bytes: &[u8],
        mode: u32,
    ) -> CommandResult<()>;
    /// Writes player-facing IP settings inside the guest.
    fn write_player_settings(&self, ip: &str, player_ip: &str) -> CommandResult<()>;
    /// Applies static guest networking.
    fn apply_static_network(&self, ip: &str, config: &GuestNetworkConfig) -> CommandResult<()>;
    /// Detects the guest's public egress IP, when possible.
    fn detect_public_ip(&self, ip: &str) -> CommandResult<Option<String>>;
}

impl<T> GuestProvider for &T
where
    T: GuestProvider + ?Sized,
{
    fn wait_for_ssh(&self, ip: &str, timeout_seconds: u64) -> CommandResult<()> {
        (*self).wait_for_ssh(ip, timeout_seconds)
    }

    fn upload_bytes(
        &self,
        ip: &str,
        remote_path: &str,
        bytes: &[u8],
        mode: u32,
    ) -> CommandResult<()> {
        (*self).upload_bytes(ip, remote_path, bytes, mode)
    }

    fn write_player_settings(&self, ip: &str, player_ip: &str) -> CommandResult<()> {
        (*self).write_player_settings(ip, player_ip)
    }

    fn apply_static_network(&self, ip: &str, config: &GuestNetworkConfig) -> CommandResult<()> {
        (*self).apply_static_network(ip, config)
    }

    fn detect_public_ip(&self, ip: &str) -> CommandResult<Option<String>> {
        (*self).detect_public_ip(ip)
    }
}
