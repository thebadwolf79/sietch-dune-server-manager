use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use dune_manager_core::orchestration::{LocalForwarder, RusshTarget};

use crate::commands::tunnel_helpers::{
    discover_database_tunnel_port, discover_director_tunnel_port, discover_pg_hero_tunnel_port,
    normalize_tunnel_service, tunnel_target, tunnel_url,
};
use crate::dto::{
    CustomTunnelStartRequest, ServerTunnelStartRequest, ServerTunnelStatus, ServerTunnelStopRequest,
};
use crate::state::{ManagedTunnel, TunnelRegistry};

const MANAGEMENT_API_PORT: u16 = 29187;
const LEGACY_MANAGEMENT_API_PORT: u16 = 8787;

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

#[tauri::command]
pub async fn start_custom_tunnel(
    registry: tauri::State<'_, TunnelRegistry>,
    request: CustomTunnelStartRequest,
) -> Result<ServerTunnelStatus, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_custom_tunnel_inner(&registry, request))
        .await
        .map_err(|err| format!("Tunnel worker failed: {err}"))?
}

fn start_custom_tunnel_inner(
    registry: &TunnelRegistry,
    request: CustomTunnelStartRequest,
) -> Result<ServerTunnelStatus, String> {
    let tunnel_id = request.tunnel_id.trim();
    if tunnel_id.is_empty() {
        return Err("Tunnel id is required.".to_string());
    }
    if let Some(status) = existing_running_tunnel(registry, tunnel_id)? {
        return Ok(status);
    }

    let target = match request.server_kind.trim() {
        "ubuntu" => {
            let mut t = RusshTarget::new(
                PathBuf::from(request.key_path.as_deref().unwrap_or_default().trim()),
                request.user.trim().to_string(),
                request.host.trim().to_string(),
            );
            if request.port != 0 {
                t.port = request.port;
            }
            t.validate().map_err(|err| err.message)?;
            t
        }
        other => return Err(format!("Unsupported remote server kind: {other}")),
    };

    let forwarder = LocalForwarder::start(
        &target,
        request.local_port,
        "127.0.0.1",
        request.remote_port,
    )
    .map_err(|err| err.message)?;
    let local_port = forwarder.local_port();

    let url = match request.protocol.trim() {
        "https" => format!("https://127.0.0.1:{local_port}/"),
        "postgresql" => format!("postgresql://127.0.0.1:{local_port}/"),
        _ => format!("http://127.0.0.1:{local_port}/"),
    };

    let status = ServerTunnelStatus {
        tunnel_id: tunnel_id.to_string(),
        service: "custom".to_string(),
        local_port,
        remote_port: request.remote_port,
        url,
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
        "managementApi" => MANAGEMENT_API_PORT,
        _ => unreachable!(),
    };

    if service == "managementApi" {
        return start_management_api_tunnel(registry, tunnel_id, &target, &service);
    }

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

fn start_management_api_tunnel(
    registry: &TunnelRegistry,
    tunnel_id: &str,
    target: &RusshTarget,
    service: &str,
) -> Result<ServerTunnelStatus, String> {
    let mut last_error = String::new();
    for remote_port in [MANAGEMENT_API_PORT, LEGACY_MANAGEMENT_API_PORT] {
        let forwarder = LocalForwarder::start(target, 0, "127.0.0.1", remote_port)
            .map_err(|err| err.message)?;
        let local_port = forwarder.local_port();
        match probe_management_api(local_port) {
            Ok(()) => {
                let status = ServerTunnelStatus {
                    tunnel_id: tunnel_id.to_string(),
                    url: tunnel_url(service, local_port),
                    service: service.to_string(),
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
                return Ok(status);
            }
            Err(err) => {
                last_error = format!("127.0.0.1:{remote_port}: {err}");
                forwarder.stop();
            }
        }
    }

    Err(format!(
        "management service did not answer on port {MANAGEMENT_API_PORT} or legacy port {LEGACY_MANAGEMENT_API_PORT}; last probe: {last_error}"
    ))
}

fn probe_management_api(local_port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{local_port}");
    let timeout = Duration::from_millis(1500);
    let socket_addr: std::net::SocketAddr =
        addr.parse().map_err(|err| format!("bad addr: {err}"))?;
    let mut stream = TcpStream::connect_timeout(&socket_addr, timeout)
        .map_err(|err| format!("connect failed: {err}"))?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|err| format!("write failed: {err}"))?;
    let mut buf = [0u8; 256];
    let n = stream
        .read(&mut buf)
        .map_err(|err| format!("read failed: {err}"))?;
    if n == 0 {
        return Err("remote closed without an HTTP response".to_string());
    }
    let head = String::from_utf8_lossy(&buf[..n]);
    if head.starts_with("HTTP/1.1 200") || head.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(format!("unexpected health response: {}", head.trim()))
    }
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
