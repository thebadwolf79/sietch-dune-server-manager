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
    wait_for_vm_ipv4, HostProvider, HostReadiness, HyperVVmLifecycleOrchestrator,
    StrictPowerShellHyperV, VmProvider,
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
    tauri::async_runtime::spawn_blocking(move || read_state(&vm_name))
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
