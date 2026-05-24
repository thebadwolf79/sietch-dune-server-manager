use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::commands::status_data::{read_remote_server_components, read_remote_server_status};
use crate::dto::{RemoteServerActionRequest, RemoteServerComponent, RemoteServerStatus};

#[tauri::command]
pub async fn remote_server_status(
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;
        read_remote_server_status(&runner, &request.namespace, &request.battlegroup_name)
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote status worker failed: {err}"))?
}

#[tauri::command]
pub async fn remote_server_components(
    request: RemoteServerActionRequest,
) -> Result<Vec<RemoteServerComponent>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;
        read_remote_server_components(&runner, &request.namespace).map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote component diagnostics worker failed: {err}"))?
}
