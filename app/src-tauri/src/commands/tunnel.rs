use dune_manager_core::orchestration::LocalForwarder;

use crate::commands::tunnel_helpers::{
    discover_database_tunnel_port, discover_director_tunnel_port, discover_pg_hero_tunnel_port,
    normalize_tunnel_service, tunnel_target, tunnel_url,
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
    let service = normalize_tunnel_service(&request.service)?;
    let remote_port = match service.as_str() {
        "director" => discover_director_tunnel_port(&target, &request.namespace)?,
        "fileBrowser" => 18888,
        "database" => discover_database_tunnel_port(&target, &request.namespace)?,
        "pgHero" => discover_pg_hero_tunnel_port(&target, &request.namespace)?,
        _ => unreachable!(),
    };

    let forwarder =
        LocalForwarder::start(&target, 0, "127.0.0.1", remote_port).map_err(|err| err.message)?;
    let local_port = forwarder.local_port();

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
    if let Some(existing) = tunnels.remove(tunnel_id) {
        existing.forwarder.stop();
    }
    tunnels.insert(
        tunnel_id.to_string(),
        ManagedTunnel {
            forwarder,
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
    if let Some(tunnel) = tunnels.remove(tunnel_id.trim()) {
        tunnel.forwarder.stop();
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
    let Some(tunnel) = tunnels.get(tunnel_id) else {
        return Ok(None);
    };
    if tunnel.forwarder.is_finished() {
        if let Some(stale) = tunnels.remove(tunnel_id) {
            stale.forwarder.stop();
        }
        Ok(None)
    } else {
        Ok(Some(tunnel.status.clone()))
    }
}
