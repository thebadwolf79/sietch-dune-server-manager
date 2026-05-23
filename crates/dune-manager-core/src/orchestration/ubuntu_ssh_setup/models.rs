use serde::{Deserialize, Serialize};

use crate::{errors::failure, models::CommandResult};

pub(super) const DEFAULT_SERVER_ROOT: &str = "/home/dune/.dune";
pub(super) const DEFAULT_LINUX_USER: &str = "dune";
pub(super) const DEFAULT_STEAMCMD_URL: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";
pub(super) const SERVER_APP_ID: &str = "4754530";
pub(super) const LEGACY_SERVER_APP_ID: &str = "3104830";

/// Read-only inventory of a remote Ubuntu host before setup begins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuSshPreflight {
    /// Kernel host name.
    pub hostname: String,
    /// Operating system pretty name from `/etc/os-release`.
    pub os_pretty_name: String,
    /// Distribution identifier from `/etc/os-release`.
    pub os_id: String,
    /// Distribution version identifier.
    pub version_id: String,
    /// CPU architecture reported by Python's platform module.
    pub architecture: String,
    /// Linux kernel release.
    pub kernel_release: String,
    /// Connected SSH username.
    pub user: String,
    /// Effective user id for the SSH session.
    pub uid: u32,
    /// Whether the session can run privileged commands without a password.
    pub passwordless_sudo: bool,
    /// Whether `systemctl` is available.
    pub systemd_available: bool,
    /// Logical CPU count.
    pub logical_processor_count: u32,
    /// Total physical memory in bytes.
    pub total_memory_bytes: u64,
    /// Available physical memory in bytes.
    pub available_memory_bytes: u64,
    /// Configured swap in bytes.
    pub swap_total_bytes: u64,
    /// Root filesystem size in bytes.
    pub root_disk_total_bytes: u64,
    /// Root filesystem free bytes.
    pub root_disk_available_bytes: u64,
    /// Public egress IP detected from the host, if reachable.
    pub public_ip: Option<String>,
    /// Non-loopback IPv4 addresses found on the host.
    pub ipv4_addresses: Vec<String>,
    /// Whether the app-owned SteamCMD path already exists.
    pub steamcmd_installed: bool,
    /// Whether k3s is already installed.
    pub k3s_installed: bool,
    /// Whether kubectl is reachable through k3s.
    pub kubectl_available: bool,
}

/// Request for creating and enabling a native Ubuntu swapfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbuntuSwapRequest {
    /// Swapfile size in GiB.
    pub swap_size_gib: u64,
}

impl UbuntuSwapRequest {
    /// Creates a request for a fixed-size `/swapfile`.
    pub fn new(swap_size_gib: u64) -> Self {
        Self { swap_size_gib }
    }

    /// Validates the requested swapfile size.
    pub fn validate(&self) -> CommandResult<()> {
        if !(1..=256).contains(&self.swap_size_gib) {
            return Err(failure("Ubuntu swap size must be between 1 and 256 GiB"));
        }
        Ok(())
    }
}

/// Result of applying the native Ubuntu swapfile configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuSwapResult {
    /// Whether a swapfile exists after configuration.
    pub swap_file_exists: bool,
    /// Whether swap is active after configuration.
    pub swap_active: bool,
    /// Configured `/swapfile` size in bytes.
    pub swap_file_bytes: u64,
    /// Total swap bytes reported by `/proc/meminfo`.
    pub swap_total_bytes: u64,
    /// Whether `/etc/fstab` contains the `/swapfile` entry.
    pub fstab_configured: bool,
    /// Whether k3s kubelet is configured for limited swap.
    pub kubelet_swap_configured: bool,
}

/// Request for preparing a fresh Ubuntu host for Dune server installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbuntuSshPrepareRequest {
    /// Remote user that owns the server payload and writable config.
    pub linux_user: String,
    /// Root directory for app-managed server state.
    pub server_root: String,
    /// URL for the SteamCMD Linux tarball.
    pub steamcmd_url: String,
}

impl Default for UbuntuSshPrepareRequest {
    fn default() -> Self {
        Self {
            linux_user: DEFAULT_LINUX_USER.to_string(),
            server_root: DEFAULT_SERVER_ROOT.to_string(),
            steamcmd_url: DEFAULT_STEAMCMD_URL.to_string(),
        }
    }
}

impl UbuntuSshPrepareRequest {
    /// Validates names and absolute paths before sending shell to the host.
    pub fn validate(&self) -> CommandResult<()> {
        validate_linux_user(&self.linux_user)?;
        validate_absolute_path(&self.server_root, "server root")?;
        if self.steamcmd_url.trim().is_empty()
            || self.steamcmd_url.contains('\n')
            || self.steamcmd_url.contains('\r')
        {
            return Err(failure("SteamCMD source URL is required"));
        }
        Ok(())
    }
}

/// Remote paths prepared for subsequent Ubuntu setup phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuSshPreparedHost {
    /// Remote user that owns the server files.
    pub linux_user: String,
    /// Server root directory.
    pub server_root: String,
    /// Server payload download directory.
    pub download_path: String,
    /// SteamCMD shell script path.
    pub steamcmd_path: String,
}

/// Result of downloading the Steam server package on Ubuntu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuServerPayload {
    /// Server payload directory.
    pub download_path: String,
    /// Whether the expected setup script is present.
    pub setup_script_present: bool,
    /// Whether the expected battlegroup script is present.
    pub battlegroup_script_present: bool,
}

pub(super) fn validate_linux_user(value: &str) -> CommandResult<()> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        || value.starts_with('-')
    {
        return Err(failure(
            "Linux user must contain only lowercase letters, digits, hyphen, or underscore",
        ));
    }
    Ok(())
}

pub(super) fn validate_absolute_path(value: &str, label: &str) -> CommandResult<()> {
    if !value.starts_with('/') || value == "/" || value.contains('\n') || value.contains('\r') {
        return Err(failure(format!("{label} must be an absolute Linux path")));
    }
    Ok(())
}

pub(super) fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_request_uses_app_owned_guest_paths() {
        let request = UbuntuSshPrepareRequest::default();
        assert_eq!(request.linux_user, "dune");
        assert_eq!(request.server_root, "/home/dune/.dune");
        request.validate().unwrap();
    }

    #[test]
    fn rejects_non_absolute_server_root() {
        let request = UbuntuSshPrepareRequest {
            server_root: "relative".to_string(),
            ..UbuntuSshPrepareRequest::default()
        };
        assert!(request.validate().is_err());
    }
}
