use std::process::{Command, Stdio};

use dune_manager_core::orchestration::openssh_base_args;

use crate::commands::tunnel_helpers::{
    discover_database_tunnel_port, discover_director_tunnel_port, discover_pg_hero_tunnel_port,
    is_local_port_available, normalize_tunnel_service, pick_available_local_port, tunnel_target,
    tunnel_url, wait_for_local_tunnel,
};
use crate::dto::{ServerTunnelStartRequest, ServerTunnelStatus, ServerTunnelStopRequest};
use crate::state::{ManagedTunnel, TunnelRegistry};

#[tauri::command]
pub async fn start_server_tunnel(
    registry: tauri::State<'_, TunnelRegistry>,
    request: ServerTunnelStartRequest,
) -> Result<ServerTunnelStatus, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_server_tunnel_inner(&registry, request))
        .await
        .map_err(|err| format!("Tunnel worker failed: {err}"))?
}

#[tauri::command]
pub async fn stop_server_tunnel(
    registry: tauri::State<'_, TunnelRegistry>,
    request: ServerTunnelStopRequest,
) -> Result<(), String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        stop_server_tunnel_inner(&registry, &request.tunnel_id)
    })
    .await
    .map_err(|err| format!("Tunnel stop worker failed: {err}"))?
}

#[tauri::command]
pub async fn server_tunnel_status(
    registry: tauri::State<'_, TunnelRegistry>,
    request: ServerTunnelStopRequest,
) -> Result<Option<ServerTunnelStatus>, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        existing_running_tunnel(&registry, request.tunnel_id.trim())
    })
    .await
    .map_err(|err| format!("Tunnel status worker failed: {err}"))?
}

#[tauri::command]
pub async fn stop_all_tunnels(registry: tauri::State<'_, TunnelRegistry>) -> Result<(), String> {
    registry.stop_all();
    Ok(())
}

fn start_server_tunnel_inner(
    registry: &TunnelRegistry,
    request: ServerTunnelStartRequest,
) -> Result<ServerTunnelStatus, String> {
    let tunnel_id = request.tunnel_id.trim();
    if tunnel_id.is_empty() {
        return Err("Tunnel id is required.".to_string());
    }
    if let Some(status) = existing_running_tunnel(registry, tunnel_id)? {
        return Ok(status);
    }

    let target = tunnel_target(&request)?;
    target.validate().map_err(|err| err.message)?;
    let service = normalize_tunnel_service(&request.service)?;
    let remote_port = match service.as_str() {
        "director" => discover_director_tunnel_port(&target, &request.namespace)?,
        "fileBrowser" => 18888,
        "database" => discover_database_tunnel_port(&target, &request.namespace)?,
        "pgHero" => discover_pg_hero_tunnel_port(&target, &request.namespace)?,
        _ => unreachable!(),
    };
    let local_port = pick_available_local_port()?;
    if !is_local_port_available(local_port) {
        return Err(format!("Local port {local_port} is already in use."));
    }

    let mut command = Command::new(&target.ssh_path);
    let mut args = openssh_base_args(&target);
    args.extend([
        "-o".to_string(),
        "ExitOnForwardFailure=yes".to_string(),
        "-N".to_string(),
        "-L".to_string(),
        format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}"),
        target.destination(),
    ]);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    dune_manager_core::shell::suppress_console_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|err| format!("Failed to start SSH tunnel: {err}"))?;
    std::thread::sleep(std::time::Duration::from_millis(700));
    if let Some(status) = child
        .try_wait()
        .map_err(|err| format!("Failed to inspect SSH tunnel: {err}"))?
    {
        return Err(format!(
            "SSH tunnel exited immediately with status {status}."
        ));
    }
    if !wait_for_local_tunnel(local_port, std::time::Duration::from_secs(3)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "SSH tunnel did not start listening on local port {local_port}."
        ));
    }

    let status = ServerTunnelStatus {
        tunnel_id: tunnel_id.to_string(),
        url: tunnel_url(&service, local_port),
        service,
        local_port,
        remote_port,
    };
    let mut tunnels = registry
        .tunnels
        .lock()
        .map_err(|_| "Tunnel registry is unavailable.".to_string())?;
    if let Some(mut existing) = tunnels.remove(tunnel_id) {
        let _ = existing.child.kill();
        let _ = existing.child.wait();
    }
    tunnels.insert(
        tunnel_id.to_string(),
        ManagedTunnel {
            child,
            status: status.clone(),
        },
    );
    Ok(status)
}

fn stop_server_tunnel_inner(registry: &TunnelRegistry, tunnel_id: &str) -> Result<(), String> {
    let mut tunnels = registry
        .tunnels
        .lock()
        .map_err(|_| "Tunnel registry is unavailable.".to_string())?;
    if let Some(mut tunnel) = tunnels.remove(tunnel_id.trim()) {
        let _ = tunnel.child.kill();
        let _ = tunnel.child.wait();
    }
    Ok(())
}

fn existing_running_tunnel(
    registry: &TunnelRegistry,
    tunnel_id: &str,
) -> Result<Option<ServerTunnelStatus>, String> {
    let mut tunnels = registry
        .tunnels
        .lock()
        .map_err(|_| "Tunnel registry is unavailable.".to_string())?;
    let Some(tunnel) = tunnels.get_mut(tunnel_id) else {
        return Ok(None);
    };
    match tunnel
        .child
        .try_wait()
        .map_err(|err| format!("Failed to inspect SSH tunnel: {err}"))?
    {
        None => Ok(Some(tunnel.status.clone())),
        Some(_) => {
            tunnels.remove(tunnel_id);
            Ok(None)
        }
    }
}
