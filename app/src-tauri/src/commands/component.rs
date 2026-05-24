use dune_manager_core::models::CommandResult;
use dune_manager_core::orchestration::{RemoteCommandRunner, RusshRunner};
use dune_manager_core::security::redact_text;

use crate::commands::shared::{command_error_message, runner_for_remote_kind, sh_single_quoted};
use crate::dto::{
    RemoteComponentLogRequest, RemoteComponentLogResult, RemoteComponentRestartRequest,
    RemoteComponentRestartResult,
};

#[tauri::command]
pub async fn remote_component_log_tail(
    request: RemoteComponentLogRequest,
) -> Result<RemoteComponentLogResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;
        read_remote_component_log_tail(
            &runner,
            &request.namespace,
            &request.component,
            request.tail,
        )
        .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote component log worker failed: {err}"))?
}

#[tauri::command]
pub async fn restart_remote_component(
    request: RemoteComponentRestartRequest,
) -> Result<RemoteComponentRestartResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
            Some(request.port),
        )?;
        restart_remote_component_inner(&runner, &request.namespace, &request.component)
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote component restart worker failed: {err}"))?
}

fn read_remote_component_log_tail(
    runner: &RusshRunner,
    namespace: &str,
    component: &str,
    tail: u32,
) -> CommandResult<RemoteComponentLogResult> {
    let component = component.trim();
    let (mode, pattern) = component_pod_selection(component)?;
    let tail = tail.clamp(20, 500);
    let script = format!(
        r#"
ns={ns}
mode={mode}
pattern={pattern}
tail_lines={tail}
component={component}

if [ "$mode" = "role" ]; then
  pods=$(sudo kubectl get pods -n "$ns" -l "role=$pattern" --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null || true)
elif [ "$mode" = "roles" ]; then
  pods=$(sudo kubectl get pods -n "$ns" --no-headers -o custom-columns=NAME:.metadata.name,ROLE:.metadata.labels.role 2>/dev/null | grep -E "$pattern" | awk '{{print $1}}' || true)
else
  pods=$(sudo kubectl get pods -n "$ns" --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null | grep -- "$pattern" || true)
fi

if [ -z "$pods" ]; then
  echo "No pods found for $component."
  exit 0
fi

for pod in $pods; do
  echo "== $pod =="
  sudo kubectl logs -n "$ns" "$pod" --all-containers --tail="$tail_lines" 2>&1 || true
done
"#,
        ns = sh_single_quoted(namespace),
        mode = sh_single_quoted(mode),
        pattern = sh_single_quoted(pattern),
        tail = tail,
        component = sh_single_quoted(component),
    );
    let output = runner.run_script(&script)?;
    Ok(RemoteComponentLogResult {
        component: component.to_string(),
        output: redact_text(&output),
    })
}

fn restart_remote_component_inner(
    runner: &RusshRunner,
    namespace: &str,
    component: &str,
) -> CommandResult<RemoteComponentRestartResult> {
    let component = component.trim();
    let (mode, pattern) = component_pod_selection(component)?;
    let script = format!(
        r#"
ns={ns}
mode={mode}
pattern={pattern}
component={component}

if [ "$mode" = "role" ]; then
  pods=$(sudo kubectl get pods -n "$ns" -l "role=$pattern" --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null || true)
elif [ "$mode" = "roles" ]; then
  pods=$(sudo kubectl get pods -n "$ns" --no-headers -o custom-columns=NAME:.metadata.name,ROLE:.metadata.labels.role 2>/dev/null | grep -E "$pattern" | awk '{{print $1}}' || true)
else
  pods=$(sudo kubectl get pods -n "$ns" --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null | grep -- "$pattern" || true)
fi

if [ -z "$pods" ]; then
  echo "No pods found for $component."
  exit 0
fi

for pod in $pods; do
  echo "Restarting $pod"
  sudo kubectl delete pod -n "$ns" "$pod" --wait=false
done
"#,
        ns = sh_single_quoted(namespace),
        mode = sh_single_quoted(mode),
        pattern = sh_single_quoted(pattern),
        component = sh_single_quoted(component),
    );
    let output = runner.run_script(&script)?;
    Ok(RemoteComponentRestartResult {
        component: component.to_string(),
        output: redact_text(&output),
    })
}

fn component_pod_selection(component: &str) -> CommandResult<(&'static str, &'static str)> {
    match component {
        "database" => Ok(("role", "igw-database")),
        "database-utilities" => Ok((
            "roles",
            "igw-database-utility|igw-database-monitor|igw-database-pghero",
        )),
        "message-queue" => Ok(("role", "igw-message-queue")),
        "director" => Ok(("role", "igw-battlegroup-director")),
        "gateway" | "gateway-resource" => Ok(("role", "igw-server-gateway")),
        "text-router" => Ok(("role", "igw-text-router")),
        "file-browser" => Ok(("role", "igw-filebrowser")),
        "server-group" => Ok(("role", "igw-server")),
        "map-survival-1" => Ok(("name", "-sg-survival-1-")),
        "map-overmap" => Ok(("name", "-sg-overmap-")),
        "map-deepdesert" => Ok(("name", "-sg-deepdesert-")),
        "map-social-arrakeen" => Ok(("name", "-sg-sh-arrakeen-")),
        "map-social-harkovillage" => Ok(("name", "-sg-sh-harkovillage-")),
        _ => Err(dune_manager_core::errors::failure(format!(
            "Unknown component key: {component}"
        ))),
    }
}
