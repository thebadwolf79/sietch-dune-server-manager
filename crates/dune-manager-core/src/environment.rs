//! Host environment detection for setup preflight.
//!
//! This module owns the ordered host preflight used by the desktop shell:
//! administrator/readiness checks first, then network candidate discovery.

use serde::{Deserialize, Serialize};

use crate::{
    models::CommandResult,
    orchestration::{
        DriveCandidate, HostProvider, HostReadiness, NetworkAdapterCandidate,
        StrictPowerShellHyperV,
    },
    shell::run_powershell,
};

/// Ordered setup environment detection result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupEnvironment {
    /// Administrator, virtualization, Hyper-V, service, and host memory state.
    pub readiness: HostReadiness,
    /// Host filesystem drives available for placing the VM data.
    pub drives: Vec<DriveCandidate>,
    /// Physical network adapters suitable for VM networking.
    pub network_adapters: Vec<NetworkAdapterCandidate>,
    /// Public IPv4 address detected from the host, when reachable.
    pub external_ip: Option<String>,
}

/// Detects the host setup environment using the default Windows provider.
pub fn detect_setup_environment() -> CommandResult<SetupEnvironment> {
    detect_setup_environment_with(&StrictPowerShellHyperV::new())
}

/// Detects the host setup environment with an injected provider.
pub fn detect_setup_environment_with<H>(host: &H) -> CommandResult<SetupEnvironment>
where
    H: HostProvider,
{
    let readiness = host.readiness()?;
    let drives = host.drives_with_minimum_free_space(0)?;
    let network_adapters = host.active_physical_adapters()?;
    let external_ip = detect_external_ipv4();
    Ok(SetupEnvironment {
        readiness,
        drives,
        network_adapters,
        external_ip,
    })
}

fn detect_external_ipv4() -> Option<String> {
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'Stop'
$ip = Invoke-RestMethod -Uri 'https://api.ipify.org' -TimeoutSec 5
if ($ip -match '^\d{1,3}(\.\d{1,3}){3}$') { $ip }
"#;

    run_powershell(script).ok().and_then(|value| {
        let trimmed = value.trim();
        if is_ipv4_literal(trimmed) {
            Some(trimmed.to_string())
        } else {
            None
        }
    })
}

fn is_ipv4_literal(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 3
                && part
                    .parse::<u8>()
                    .is_ok_and(|_| part.chars().all(|ch| ch.is_ascii_digit()))
        })
}

#[cfg(test)]
mod tests {
    use super::is_ipv4_literal;

    #[test]
    fn validates_ipv4_literals() {
        assert!(is_ipv4_literal("192.0.2.75"));
        assert!(is_ipv4_literal("8.8.8.8"));
        assert!(!is_ipv4_literal(""));
        assert!(!is_ipv4_literal("999.1.1.1"));
        assert!(!is_ipv4_literal("example.com"));
    }
}
