//! Hyper-V VM power-management commands (issue #28).
//!
//! These are only meaningful when the manager runs *on* the Hyper-V host. When
//! Hyper-V is unreachable — a remote machine, the module isn't installed, or the
//! process lacks rights — the commands return [`SystemState::HostPermissionUnavailable`]
//! so the UI can drop to connect-only mode instead of surfacing a hard error.
//!
//! State authority stays in Rust: each command returns a [`SystemState`] the React
//! UI renders and gates actions on. PowerShell/Hyper-V calls are synchronous and
//! blocking, so they run on `spawn_blocking` to keep the UI responsive.

use dune_manager_core::orchestration::{
    HyperVVmLifecycleOrchestrator, StrictPowerShellHyperV, VmProvider,
};

use crate::commands::shared::command_error_message;
use crate::dto::SystemState;
use crate::logging::TauriOperationSink;

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

/// Reads the current lifecycle state of the named VM (non-destructive).
#[tauri::command]
pub async fn vm_get_state(vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || read_state(&vm_name))
        .await
        .map_err(|err| format!("vm_get_state worker failed: {err}"))?
}

/// Starts the named VM, then reports the resulting state.
#[tauri::command]
pub async fn vm_start(app: tauri::AppHandle, vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(app);
        HyperVVmLifecycleOrchestrator::new(StrictPowerShellHyperV::new())
            .start(&vm_name, &mut sink)
            .map_err(command_error_message)?;
        read_state(&vm_name)
    })
    .await
    .map_err(|err| format!("vm_start worker failed: {err}"))?
}

/// Stops (turns off) the named VM, then reports the resulting state.
#[tauri::command]
pub async fn vm_stop(app: tauri::AppHandle, vm_name: String) -> Result<SystemState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(app);
        HyperVVmLifecycleOrchestrator::new(StrictPowerShellHyperV::new())
            .stop(&vm_name, &mut sink)
            .map_err(command_error_message)?;
        read_state(&vm_name)
    })
    .await
    .map_err(|err| format!("vm_stop worker failed: {err}"))?
}
