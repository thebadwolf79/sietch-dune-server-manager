//! Hyper-V VM power-management commands (issue #28).
//!
//! These are only meaningful when the manager runs *on* the Hyper-V host. When
//! Hyper-V is unreachable — a remote machine, the module isn't installed, or the
//! process lacks rights — the commands return [`SystemState::HostPermissionUnavailable`]
//! (or `vm_host_readiness` reports it) so the UI can drop to connect-only mode
//! instead of surfacing a hard error.
//!
//! State authority stays in Rust: each command returns a [`SystemState`] the React
//! UI renders and gates actions on. PowerShell/Hyper-V calls are synchronous and
//! blocking, so they run on `spawn_blocking` to keep the UI responsive, and long
//! waits stream progress to the UI through [`TauriOperationSink`].

use dune_manager_core::orchestration::{
    wait_for_vm_ipv4, DuneVmCandidate, DuneVmConfidence, DuneVmDetector, HostProvider,
    HostReadiness, HyperVVmLifecycleOrchestrator, StrictPowerShellHyperV, VmProvider,
};

use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::commands::status_data::read_remote_server_status;
use crate::dto::{RemoteServerActionRequest, SystemState};
use crate::logging::TauriOperationSink;

/// Seconds to wait for a freshly started VM to report a routable IPv4 address
/// before reporting back the current (still-starting) state.
const VM_BOOT_IP_TIMEOUT_SECS: u64 = 120;

/// Heuristic: does this host-provider error mean "Hyper-V can't be managed here"
/// (remote machine, Hyper-V not installed, or not authorized) rather than a real
/// operational failure? Used to degrade gracefully to connect-only mode.
fn is_host_unavailable(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("permission")
        || m.contains("access is denied")
        || m.contains("not recognized") // Get-VM cmdlet missing (no Hyper-V module)
        || m.contains("hyper-v")
        || m.contains("is not installed")
}

/// Blocking state read used by the commands below; call inside `spawn_blocking`.
fn read_state(vm_name: &str) -> Result<SystemState, String> {
    match StrictPowerShellHyperV::new().get_vm(vm_name) {
        Ok(Some(record)) => Ok(SystemState::from_vm_state(&record.raw_state)),
        Ok(None) => Ok(SystemState::Error {
            message: format!("VM '{vm_name}' was not found on this host."),
        }),
        Err(err) => {
            let message = command_error_message(err);
            if is_host_unavailable(&message) {
                Ok(SystemState::HostPermissionUnavailable { reason: message })
            } else {
                Err(message)
            }
        }
    }
}

/// Pick the highest-confidence Dune VM candidate's name. Pure (host-free) so the
/// selection logic is unit-testable. `None` when the list is empty.
fn highest_confidence_vm_name(candidates: &[DuneVmCandidate]) -> Option<String> {
    candidates
        .iter()
        .min_by_key(|c| match c.confidence {
            DuneVmConfidence::High => 0u8,
            DuneVmConfidence::Medium => 1,
            DuneVmConfidence::Low => 2,
        })
        .map(|c| c.vm.name.clone())
}

/// Resolve the Hyper-V VM name to actually operate on.
///
/// The UI addresses the VM by the registered server's world/battlegroup id
/// (e.g. `sh-431c…`), which is NOT the Hyper-V VM name — so an exact lookup
/// misses and Start/Stop/state read break with "VM not found". Strategy:
///   1. exact match wins (honors a VM that genuinely carries that name);
///   2. else fall back to host-side Dune VM detection (highest confidence);
///   3. else return the requested name so a real "not found" still surfaces.
///
/// Runs blocking Hyper-V calls — invoke inside `spawn_blocking`.
fn resolve_vm_name(requested: &str) -> String {
    if let Ok(Some(_)) = StrictPowerShellHyperV::new().get_vm(requested) {
        return requested.to_string();
    }
    match DuneVmDetector::new(StrictPowerShellHyperV::new()).detect() {
        Ok(candidates) => {
            highest_confidence_vm_name(&candidates).unwrap_or_else(|| requested.to_string())
        }
        Err(_) => requested.to_string(),
    }
}

/// Reports host readiness so the UI can choose connect-only vs. power-capable mode.
///
/// `hyperv_available && vmms_running` means this machine can power the VM; on a
/// remote/non-Hyper-V host this errors or reports those flags false, and the UI
/// hides the VM power controls.
#[tauri::command]
pub async fn vm_host_readiness() -> Result<HostReadiness, String> {
    tauri::async_runtime::spawn_blocking(|| {
        StrictPowerShellHyperV::new()
            .readiness()
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("vm_host_readiness worker failed: {err}"))?
}

/// Reads the current lifecycle state of the named VM (non-destructive).
#[tauri::command]
pub async fn vm_get_state(vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || read_state(&resolve_vm_name(&vm_name)))
        .await
        .map_err(|err| format!("vm_get_state worker failed: {err}"))?
}

/// Reads the live battlegroup status over SSH and maps it to a battlegroup-level
/// [`SystemState`] (unified vocabulary with the VM-level states).
#[tauri::command]
pub async fn battlegroup_system_state(
    request: RemoteServerActionRequest,
) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;
        let status =
            read_remote_server_status(&runner, &request.namespace, &request.battlegroup_name)
                .map_err(command_error_message)?;
        Ok(SystemState::from_battlegroup_status(&status.battlegroup))
    })
    .await
    .map_err(|err| format!("battlegroup_system_state worker failed: {err}"))?
}

/// Starts the named VM, waits for it to boot and acquire a routable IP (streaming
/// progress to the UI), then reports the resulting state. Idempotent: if the VM is
/// already running or transitioning, it returns the current state without acting.
#[tauri::command]
pub async fn vm_start(app: tauri::AppHandle, vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(app);
        // Resolve the world/battlegroup id the UI passes to the real Hyper-V VM
        // name before any state read or power action.
        let vm_name = resolve_vm_name(&vm_name);

        // Idempotent guard: only Off/Saved/Paused can be started (Grok risk matrix).
        let current = read_state(&vm_name)?;
        if !current.can_start_vm() {
            sink.info(
                "vm.start",
                format!("VM '{vm_name}' is already running or transitioning; nothing to start."),
            );
            return Ok(current);
        }

        let provider = StrictPowerShellHyperV::new();
        sink.info("vm.start", format!("Starting VM '{vm_name}'..."));
        HyperVVmLifecycleOrchestrator::new(StrictPowerShellHyperV::new())
            .start(&vm_name, &mut sink)
            .map_err(command_error_message)?;

        sink.info(
            "vm.start",
            "Waiting for the VM to boot and acquire an IP address...",
        );
        match wait_for_vm_ipv4(&provider, &vm_name, VM_BOOT_IP_TIMEOUT_SECS) {
            Ok(ip) => sink.info("vm.start", format!("VM is up at {ip}.")),
            // Not fatal: the VM may simply still be booting. Report whatever state
            // it is in now so the UI can keep polling rather than hard-failing.
            Err(err) => sink.warn(
                "vm.start",
                format!(
                    "VM started but no IP within {VM_BOOT_IP_TIMEOUT_SECS}s: {}",
                    command_error_message(err)
                ),
            ),
        }

        read_state(&vm_name)
    })
    .await
    .map_err(|err| format!("vm_start worker failed: {err}"))?
}

/// Stops (turns off) the named VM, then reports the resulting state. Idempotent:
/// if the VM is not running there is nothing to stop, so it returns current state.
#[tauri::command]
pub async fn vm_stop(app: tauri::AppHandle, vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(app);
        let vm_name = resolve_vm_name(&vm_name);

        let current = read_state(&vm_name)?;
        if !current.battlegroup_actions_enabled() {
            sink.info("vm.stop", format!("VM '{vm_name}' is not running; nothing to stop."));
            return Ok(current);
        }

        HyperVVmLifecycleOrchestrator::new(StrictPowerShellHyperV::new())
            .stop(&vm_name, &mut sink)
            .map_err(command_error_message)?;
        read_state(&vm_name)
    })
    .await
    .map_err(|err| format!("vm_stop worker failed: {err}"))?
}

/// Best-effort connection defaults for the local Funcom VM, used to pre-fill the
/// "Add Remote Server" dialog. Every field is advisory; the user can edit them.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VmConnectionDefaults {
    /// True when a Dune VM was found on this host.
    pub found: bool,
    /// The VM's routable IPv4, when it is running and reports one.
    pub host: Option<String>,
    /// SSH user the vendor VM uses.
    pub user: String,
    /// Default SSH port.
    pub port: u16,
    /// Path to the Funcom-generated SSH key, when present on this machine.
    pub key_path: Option<String>,
    /// Detected VM name.
    pub vm_name: Option<String>,
    /// The Funcom self-hosted VM is Alpine.
    pub server_type: String,
    /// Detection confidence (high/medium/low) when a VM was found.
    pub confidence: Option<String>,
    /// Human-readable note (e.g. why host/key couldn't be auto-filled).
    pub note: Option<String>,
}

/// Auto-detects the local Funcom VM + SSH key to pre-fill the add-server form.
/// Assumes the operator has run Funcom's setup at least once (which creates the VM
/// and the key at `%LOCALAPPDATA%\DuneAwakeningServer\sshKey`). Host-only and
/// best-effort: it never errors the dialog — on a remote/no-Hyper-V machine it just
/// returns the safe defaults (user `dune`, port 22) for manual entry.
#[tauri::command]
pub async fn detect_local_vm_connection() -> Result<VmConnectionDefaults, String> {
    tauri::async_runtime::spawn_blocking(|| {
        // SSH key at the vendor location — independent of Hyper-V availability.
        let key_path = std::env::var("LOCALAPPDATA").ok().and_then(|base| {
            let p = format!(r"{base}\DuneAwakeningServer\sshKey");
            std::path::Path::new(&p).exists().then_some(p)
        });

        let mut defaults = VmConnectionDefaults {
            found: false,
            host: None,
            user: "dune".to_string(),
            port: 22,
            key_path,
            vm_name: None,
            server_type: "alpine".to_string(),
            confidence: None,
            note: None,
        };

        match DuneVmDetector::new(StrictPowerShellHyperV::new()).detect() {
            Ok(mut candidates) if !candidates.is_empty() => {
                candidates.sort_by_key(|c| match c.confidence {
                    DuneVmConfidence::High => 0,
                    DuneVmConfidence::Medium => 1,
                    DuneVmConfidence::Low => 2,
                });
                let top = &candidates[0];
                defaults.found = true;
                defaults.vm_name = Some(top.vm.name.clone());
                defaults.confidence = Some(
                    match top.confidence {
                        DuneVmConfidence::High => "high",
                        DuneVmConfidence::Medium => "medium",
                        DuneVmConfidence::Low => "low",
                    }
                    .to_string(),
                );
                match top
                    .vm
                    .ipv4_addresses
                    .iter()
                    .find(|ip| !ip.starts_with("169.254.") && !ip.trim().is_empty())
                {
                    Some(ip) => defaults.host = Some(ip.clone()),
                    None => {
                        defaults.note = Some(format!(
                            "Found VM '{}' but it has no IP yet — start it, then reopen this dialog.",
                            top.vm.name
                        ));
                    }
                }
            }
            Ok(_) => {
                defaults.note = Some(
                    "No Dune VM found on this host — enter the host details manually.".to_string(),
                );
            }
            Err(_) => {
                defaults.note = Some(
                    "Hyper-V isn't available here (remote/connect-only) — enter details manually."
                        .to_string(),
                );
            }
        }

        Ok(defaults)
    })
    .await
    .map_err(|err| format!("detect_local_vm_connection worker failed: {err}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use dune_manager_core::orchestration::{VmInventoryRecord, VmPowerState};

    fn candidate(name: &str, confidence: DuneVmConfidence) -> DuneVmCandidate {
        DuneVmCandidate {
            vm: VmInventoryRecord {
                name: name.to_string(),
                state: VmPowerState::Off,
                raw_state: "Off".to_string(),
                configuration_location: String::new(),
                path: String::new(),
                memory_assigned_bytes: 0,
                processor_count: 0,
                uptime_seconds: 0,
                ipv4_addresses: vec![],
                hard_disk_paths: vec![],
                disk_size_bytes: 0,
                disk_file_size_bytes: 0,
                switch_names: vec![],
            },
            confidence,
            reasons: vec![],
        }
    }

    #[test]
    fn empty_candidates_resolve_to_none() {
        assert_eq!(highest_confidence_vm_name(&[]), None);
    }

    #[test]
    fn prefers_highest_confidence_regardless_of_order() {
        let cands = vec![
            candidate("low-vm", DuneVmConfidence::Low),
            candidate("high-vm", DuneVmConfidence::High),
            candidate("medium-vm", DuneVmConfidence::Medium),
        ];
        assert_eq!(
            highest_confidence_vm_name(&cands).as_deref(),
            Some("high-vm")
        );
    }

    #[test]
    fn single_candidate_is_chosen() {
        let cands = vec![candidate("dune-awakening", DuneVmConfidence::Medium)];
        assert_eq!(
            highest_confidence_vm_name(&cands).as_deref(),
            Some("dune-awakening")
        );
    }
}
