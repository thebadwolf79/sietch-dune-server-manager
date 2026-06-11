use dune_manager_core::orchestration::RemoteCommandRunner;

use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::commands::status_data::remote_records_from_battlegroups;
use crate::dto::{RemoteConnectionRequest, RemoteServerRecord};

#[tauri::command]
pub async fn detect_remote_ubuntu_servers(
    request: RemoteConnectionRequest,
) -> Result<Vec<RemoteServerRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Every Funcom self-hosted VM is Alpine; label detected servers accordingly.
        // (runner_for_remote_kind ignores server_type; the management-service install
        // detects systemd vs OpenRC at runtime, so this is a display/typing label.)
        let request = RemoteConnectionRequest {
            server_type: Some("alpine".to_string()),
            ..request
        };
        let user = request.user.clone().unwrap_or_default();
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host.clone(),
            user,
            request.key_path.clone(),
            Some(request.port),
        )?;
        let value = runner
            .run_json(
                "sudo kubectl get battlegroups -A -o json",
                "remote ubuntu battlegroups",
            )
            .map_err(command_error_message)?;
        Ok(remote_records_from_battlegroups(&request, &value))
    })
    .await
    .map_err(|err| format!("Remote server detection worker failed: {err}"))?
}
