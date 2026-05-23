use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use dune_manager_core::models::{CommandFailure, CommandResult};
use dune_manager_core::orchestration::{
    is_started_state, openssh_base_args, BattlegroupManagementOrchestrator, BattlegroupRef,
    BattlegroupUpdateOrchestrator, KubernetesProvider, OpenSshRunner, OpenSshTarget,
    OperationSink, OrchestrationEvent, RemoteCommandRunner, SshGuestBootstrapProvider,
    StructuredKubectl, UbuntuSshPrepareRequest, UbuntuSshSetup,
};
use dune_manager_core::security::redact_text;
use dune_manager_core::toolchain::{ManagedTool, Toolchain};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteConnectionRequest {
    host: String,
    key_path: Option<String>,
    server_type: Option<String>,
    user: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerActionRequest {
    server_type: Option<String>,
    host: String,
    user: String,
    key_path: Option<String>,
    namespace: String,
    battlegroup_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerTunnelStartRequest {
    tunnel_id: String,
    server_kind: String,
    service: String,
    host: String,
    user: Option<String>,
    key_path: Option<String>,
    namespace: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerTunnelStopRequest {
    tunnel_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerTunnelStatus {
    tunnel_id: String,
    service: String,
    local_port: u16,
    remote_port: u16,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteBattlegroupStatus {
    stop: bool,
    phase: String,
    server_group_phase: String,
    director_phase: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerStatus {
    battlegroup: RemoteBattlegroupStatus,
    package: RemoteServerPackageStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerPackageStatus {
    installed_build_id: Option<String>,
    battlegroup_version: Option<String>,
    live_battlegroup_version: Option<String>,
    operator_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerComponent {
    name: String,
    log_key: String,
    category: String,
    state: String,
    tone: String,
    summary: String,
    details: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteComponentLogRequest {
    server_type: Option<String>,
    host: String,
    user: String,
    key_path: Option<String>,
    namespace: String,
    component: String,
    tail: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteComponentLogResult {
    component: String,
    output: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteComponentRestartRequest {
    server_type: Option<String>,
    host: String,
    user: String,
    key_path: Option<String>,
    namespace: String,
    component: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteComponentRestartResult {
    component: String,
    output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationLogPayload {
    level: &'static str,
    scope: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerRecord {
    #[serde(rename = "type")]
    server_type: String,
    id: String,
    name: String,
    host: String,
    user: String,
    key_path: String,
    namespace: String,
    battlegroup_name: String,
    world_unique_name: String,
    phase: String,
}

struct TauriOperationSink {
    app: tauri::AppHandle,
}

#[derive(Default, Clone)]
struct TunnelRegistry {
    tunnels: Arc<Mutex<HashMap<String, ManagedTunnel>>>,
}

struct ManagedTunnel {
    child: Child,
    status: ServerTunnelStatus,
}

impl TunnelRegistry {
    fn stop_all(&self) {
        let Ok(mut tunnels) = self.tunnels.lock() else {
            return;
        };
        for (_, mut tunnel) in tunnels.drain() {
            let _ = tunnel.child.kill();
            let _ = tunnel.child.wait();
        }
    }
}

impl TauriOperationSink {
    fn info(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "operation-log",
            OperationLogPayload {
                level: "info",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }

    fn warn(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "operation-log",
            OperationLogPayload {
                level: "warn",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }
}

impl OperationSink for TauriOperationSink {
    fn emit(&mut self, event: OrchestrationEvent) {
        self.info(event.step_id, event.message);
    }
}

#[tauri::command]
async fn detect_remote_ubuntu_servers(
    request: RemoteConnectionRequest,
) -> Result<Vec<RemoteServerRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let request = RemoteConnectionRequest {
            server_type: Some("ubuntu".to_string()),
            user: Some("root".to_string()),
            ..request
        };
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host.clone(),
            request.user.as_deref().unwrap_or("root").to_string(),
            request.key_path.clone(),
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

#[tauri::command]
async fn remote_server_status(
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        read_remote_server_status(&runner, &request.namespace, &request.battlegroup_name)
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote status worker failed: {err}"))?
}

#[tauri::command]
async fn remote_server_components(
    request: RemoteServerActionRequest,
) -> Result<Vec<RemoteServerComponent>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        read_remote_server_components(&runner, &request.namespace).map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote component diagnostics worker failed: {err}"))?
}

#[tauri::command]
async fn start_server_tunnel(
    registry: tauri::State<'_, TunnelRegistry>,
    request: ServerTunnelStartRequest,
) -> Result<ServerTunnelStatus, String> {
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_server_tunnel_inner(&registry, request))
        .await
        .map_err(|err| format!("Tunnel worker failed: {err}"))?
}

#[tauri::command]
async fn stop_server_tunnel(
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
async fn server_tunnel_status(
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
async fn stop_all_tunnels(registry: tauri::State<'_, TunnelRegistry>) -> Result<(), String> {
    registry.stop_all();
    Ok(())
}

#[tauri::command]
async fn remote_component_log_tail(
    request: RemoteComponentLogRequest,
) -> Result<RemoteComponentLogResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
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
async fn restart_remote_component(
    request: RemoteComponentRestartRequest,
) -> Result<RemoteComponentRestartResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        restart_remote_component_inner(&runner, &request.namespace, &request.component)
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote component restart worker failed: {err}"))?
}

#[tauri::command]
async fn start_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    run_remote_battlegroup_action(app, request, false).await
}

#[tauri::command]
async fn stop_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    run_remote_battlegroup_action(app, request, true).await
}

#[tauri::command]
async fn update_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.info("bg.update", "Checking remote battlegroup update.");
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        run_battlegroup_update_with_runner(
            &runner,
            &mut sink,
            request.namespace,
            request.battlegroup_name,
        )
    })
    .await
    .map_err(|err| format!("Remote battlegroup update worker failed: {err}"))?
}

async fn run_remote_battlegroup_action(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.info("bg.check", "Checking remote battlegroup state.");
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        run_battlegroup_action_with_runner(
            &runner,
            &mut sink,
            request.namespace,
            request.battlegroup_name,
            stop,
        )
    })
    .await
    .map_err(|err| format!("Remote battlegroup action worker failed: {err}"))?
}

fn run_battlegroup_action_with_runner(
    runner: &OpenSshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let kubernetes = StructuredKubectl::new(runner.clone());
    let before = kubernetes
        .battlegroup_state(&namespace, &battlegroup_name)
        .map_err(command_error_message)?;
    let before_started = is_started_state(&before);
    if stop && !before_started {
        return Err(format!(
            "Battlegroup is not running (phase={}, stop={}, serverGroup={}, director={}).",
            before.phase, before.stop, before.server_group_phase, before.director_phase
        ));
    }
    if !stop && before_started {
        return Err("Battlegroup is already started.".to_string());
    }
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let manager = BattlegroupManagementOrchestrator::new(kubernetes);
    if stop {
        manager
            .stop(&battlegroup, sink)
            .map_err(command_error_message)?;
    } else {
        manager
            .start_and_wait_director(&battlegroup, 180, sink)
            .map_err(command_error_message)?;
    }
    sink.info("bg.check", "Refreshing battlegroup state.");
    read_remote_server_status(runner, &battlegroup.namespace, &battlegroup.name)
        .map_err(command_error_message)
}

fn wait_for_battlegroup_fully_stopped(
    kubernetes: &StructuredKubectl<OpenSshRunner>,
    battlegroup: &BattlegroupRef,
    timeout_seconds: u64,
    sink: &mut TauriOperationSink,
) -> CommandResult<()> {
    sink.info("bg.update", "Verifying BattleGroup is fully stopped.");
    let mut elapsed = 0;
    let mut last = None;
    while elapsed <= timeout_seconds {
        let state = kubernetes.battlegroup_state(&battlegroup.namespace, &battlegroup.name)?;
        if is_fully_stopped_state(&state) {
            return Ok(());
        }
        last = Some(state);
        std::thread::sleep(std::time::Duration::from_secs(5));
        elapsed += 5;
    }
    let detail = last
        .map(|state| {
            format!(
                "last phase={}, stop={}, serverGroup={}, director={}",
                state.phase, state.stop, state.server_group_phase, state.director_phase
            )
        })
        .unwrap_or_else(|| "no BattleGroup state was read".to_string());
    Err(dune_manager_core::errors::failure(format!(
        "BattleGroup did not fully stop within {timeout_seconds}s ({detail})"
    )))
}

fn is_fully_stopped_state(state: &dune_manager_core::orchestration::BattlegroupState) -> bool {
    state.stop
        && stoppedish_phase(&state.phase)
        && stoppedish_phase(&state.server_group_phase)
        && !director_running_phase(&state.director_phase)
}

fn stoppedish_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "stopped" | "suspended" | "notready" | "not_ready" | "unknown"
        )
}

fn director_running_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "running" | "ready" | "healthy" | "available" | "reconciling"
    )
}

fn run_battlegroup_update_with_runner(
    runner: &OpenSshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
) -> Result<RemoteServerStatus, String> {
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let kubernetes = StructuredKubectl::new(runner.clone());
    let manager = BattlegroupManagementOrchestrator::new(kubernetes);
    sink.warn(
        "bg.update",
        "Stopping BattleGroup before applying the server update.",
    );
    manager
        .stop(&battlegroup, sink)
        .map_err(command_error_message)?;
    let verifier = StructuredKubectl::new(runner.clone());
    wait_for_battlegroup_fully_stopped(&verifier, &battlegroup, 600, sink)
        .map_err(command_error_message)?;
    let provider = SshGuestBootstrapProvider::new(runner.clone());
    let ubuntu = UbuntuSshSetup::new(runner.clone());
    let prepare = UbuntuSshPrepareRequest::default();
    ubuntu
        .install_server_payload(&prepare, sink)
        .map_err(command_error_message)?;
    BattlegroupUpdateOrchestrator::new(provider)
        .update_from_downloads(&battlegroup, sink)
        .map_err(command_error_message)?;
    sink.warn("bg.update", "Starting BattleGroup after update.");
    manager
        .start_and_wait_director(&battlegroup, 600, sink)
        .map_err(command_error_message)?;
    sink.info("bg.update", "Refreshing battlegroup state.");
    read_remote_server_status(runner, &battlegroup.namespace, &battlegroup.name)
        .map_err(command_error_message)
}

fn remote_records_from_battlegroups(
    request: &RemoteConnectionRequest,
    value: &Value,
) -> Vec<RemoteServerRecord> {
    value
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| remote_record_from_battlegroup(request, item))
        .collect()
}

fn remote_record_from_battlegroup(
    request: &RemoteConnectionRequest,
    item: &Value,
) -> Option<RemoteServerRecord> {
    let namespace = item
        .get("metadata")?
        .get("namespace")?
        .as_str()?
        .to_string();
    let battlegroup_name = item.get("metadata")?.get("name")?.as_str()?.to_string();
    let title = item
        .get("spec")
        .and_then(|spec| spec.get("title"))
        .and_then(Value::as_str)
        .unwrap_or(&battlegroup_name)
        .to_string();
    let phase = item
        .get("status")
        .and_then(|status| status.get("phase"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string();
    let server_type = request
        .server_type
        .as_deref()
        .unwrap_or("ubuntu")
        .trim()
        .to_string();
    let user = request
        .user
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("root")
        .to_string();
    Some(RemoteServerRecord {
        id: remote_record_id(&server_type, &request.host, request.key_path.as_deref()),
        name: title,
        host: request.host.clone(),
        user,
        key_path: request.key_path.clone().unwrap_or_default(),
        server_type,
        namespace,
        battlegroup_name: battlegroup_name.clone(),
        world_unique_name: battlegroup_name,
        phase,
    })
}

fn remote_record_id(_server_type: &str, host: &str, key_path: Option<&str>) -> String {
    format!(
        "ubuntu:{}:{}",
        host.trim().to_lowercase(),
        key_path.unwrap_or_default().trim().to_lowercase()
    )
}

fn remote_runner(host: String, user: String, key_path: String) -> Result<OpenSshRunner, String> {
    let toolchain = Toolchain::from_default_root().map_err(|err| err.message)?;
    toolchain
        .install(ManagedTool::OpenSsh, false, None)
        .map_err(|err| err.message)?;
    let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
    Ok(OpenSshRunner::new(OpenSshTarget::new(
        ssh_path,
        PathBuf::from(key_path),
        user,
        host,
    )))
}

fn runner_for_remote_kind(
    _server_type: Option<&str>,
    host: String,
    user: String,
    key_path: Option<String>,
) -> Result<OpenSshRunner, String> {
    let key_path = key_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SSH private key is required for remote Ubuntu servers.".to_string())?;
    remote_runner(host, user, key_path)
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

fn tunnel_target(request: &ServerTunnelStartRequest) -> Result<OpenSshTarget, String> {
    let toolchain = Toolchain::from_default_root().map_err(|err| err.message)?;
    toolchain
        .install(ManagedTool::OpenSsh, false, None)
        .map_err(|err| err.message)?;
    let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
    match request.server_kind.trim() {
        "ubuntu" => Ok(OpenSshTarget::new(
            ssh_path,
            PathBuf::from(
                request
                    .key_path
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            ),
            request.user.as_deref().unwrap_or("root").trim().to_string(),
            request.host.trim().to_string(),
        )),
        other => Err(format!("Unsupported remote server kind: {other}")),
    }
}

fn normalize_tunnel_service(service: &str) -> Result<String, String> {
    match service.trim() {
        "director" => Ok("director".to_string()),
        "fileBrowser" => Ok("fileBrowser".to_string()),
        "database" => Ok("database".to_string()),
        "pgHero" => Ok("pgHero".to_string()),
        other => Err(format!("Unsupported tunnel service: {other}")),
    }
}

fn tunnel_url(service: &str, local_port: u16) -> String {
    if service == "database" {
        format!("postgresql://127.0.0.1:{local_port}/dune")
    } else {
        format!("http://127.0.0.1:{local_port}/")
    }
}

fn discover_director_tunnel_port(target: &OpenSshTarget, namespace: &str) -> Result<u16, String> {
    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the Director tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get svc -n {} -o json",
                sh_single_quoted(namespace)
            ),
            "director service list",
        )
        .map_err(command_error_message)?;
    for service in value["items"].as_array().cloned().unwrap_or_default() {
        for port in service["spec"]["ports"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if port["port"].as_u64() == Some(11717) {
                if let Some(node_port) = port["nodePort"]
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                {
                    return Ok(node_port);
                }
            }
        }
    }
    Err("Director service is not currently exposed in Kubernetes.".to_string())
}

fn discover_database_tunnel_port(target: &OpenSshTarget, namespace: &str) -> Result<u16, String> {
    const DEFAULT_DATABASE_PORT: u16 = dune_manager_core::database::DEFAULT_DUNE_DATABASE_PORT;

    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the database tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get databasedeployments -n {} -o json",
                sh_single_quoted(namespace)
            ),
            "database deployment list",
        )
        .map_err(command_error_message)?;
    for deployment in value["items"].as_array().cloned().unwrap_or_default() {
        if let Some(port) = deployment["spec"]["port"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
        {
            return Ok(port);
        }
    }
    Ok(DEFAULT_DATABASE_PORT)
}

fn discover_pg_hero_tunnel_port(target: &OpenSshTarget, namespace: &str) -> Result<u16, String> {
    const DEFAULT_PG_HERO_PORT: u16 = 21111;

    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the PgHero tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get pods -n {} -l role=igw-database-pghero -o json",
                sh_single_quoted(namespace)
            ),
            "PgHero pod list",
        )
        .map_err(command_error_message)?;
    for pod in value["items"].as_array().cloned().unwrap_or_default() {
        for container in pod["spec"]["containers"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            for env in container["env"].as_array().cloned().unwrap_or_default() {
                if env["name"].as_str() == Some("PORT") {
                    if let Some(port) = env["value"]
                        .as_str()
                        .and_then(|value| value.parse::<u16>().ok())
                    {
                        return Ok(port);
                    }
                }
            }
        }
    }
    Ok(DEFAULT_PG_HERO_PORT)
}

fn pick_available_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("Failed to reserve a local tunnel port: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Failed to read local tunnel port: {err}"))?
        .port();
    drop(listener);
    Ok(port)
}

fn is_local_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn wait_for_local_tunnel(port: u16, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

fn command_error_message(err: CommandFailure) -> String {
    let mut parts = vec![err.message];
    if !err.stderr.trim().is_empty() {
        parts.push(err.stderr);
    }
    if !err.stdout.trim().is_empty() {
        parts.push(err.stdout);
    }
    parts.join("\n")
}

fn read_remote_server_status(
    runner: &OpenSshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerStatus> {
    let kubernetes = StructuredKubectl::new(runner.clone());
    let battlegroup = kubernetes.battlegroup_state(namespace, battlegroup_name)?;
    let package = read_guest_package_status(runner, namespace, battlegroup_name)?;
    Ok(RemoteServerStatus {
        battlegroup: RemoteBattlegroupStatus {
            stop: battlegroup.stop,
            phase: battlegroup.phase,
            server_group_phase: battlegroup.server_group_phase,
            director_phase: battlegroup.director_phase,
        },
        package,
    })
}

fn read_guest_package_status(
    runner: &OpenSshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerPackageStatus> {
    let script = r#"
set -u
download=/home/dune/.dune/download
manifest="$download/steamapps/appmanifest_4754530.acf"
ns=__NAMESPACE__
bg=__BATTLEGROUP__
read_vdf_value() {
  key="$1"
  file="$2"
  [ -f "$file" ] || return 0
  awk -F '"' -v wanted="$key" '$2 == wanted { print $4; exit }' "$file" 2>/dev/null || true
}
read_file() {
  file="$1"
  [ -f "$file" ] || return 0
  head -n 1 "$file" 2>/dev/null | tr -d '\r\n'
}
printf 'installedBuildId=%s\n' "$(read_vdf_value buildid "$manifest")"
printf 'battlegroupVersion=%s\n' "$(read_file "$download/images/battlegroup/version.txt")"
printf 'operatorVersion=%s\n' "$(read_file "$download/images/operators/version.txt")"
live_image=$(sudo kubectl get battlegroup "$bg" -n "$ns" -o jsonpath='{..image}' 2>/dev/null | tr ' ' '\n' | awk -F: '/self-hosting\/(igw-server|seabass-server):/ { print $NF; exit }' || true)
printf 'liveBattlegroupVersion=%s\n' "$live_image"
"#
    .replace("__NAMESPACE__", &sh_single_quoted(namespace))
    .replace("__BATTLEGROUP__", &sh_single_quoted(battlegroup_name));
    let output = runner.run_script(&script)?;
    let value = |key: &str| {
        output.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
    };
    Ok(RemoteServerPackageStatus {
        installed_build_id: value("installedBuildId"),
        battlegroup_version: value("battlegroupVersion"),
        live_battlegroup_version: value("liveBattlegroupVersion"),
        operator_version: value("operatorVersion"),
    })
}

fn read_remote_server_components(
    runner: &OpenSshRunner,
    namespace: &str,
) -> CommandResult<Vec<RemoteServerComponent>> {
    let pods = runner.run_json(
        &format!(
            "sudo kubectl get pods -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server pods",
    )?;
    let resources = runner.run_json(
        &format!(
            "sudo kubectl get servergroups,servergateways,serversets -n {} -o json",
            sh_single_quoted(namespace)
        ),
        "remote server resources",
    )?;

    let mut components = vec![
        pod_component("Database", "database", &pods, |role, name| {
            role.contains("database") && !name.contains("-util-")
        }),
        pod_component(
            "Database utilities",
            "database-utilities",
            &pods,
            |role, _| {
                role.contains("database-utility")
                    || role.contains("database-monitor")
                    || role.contains("database-pghero")
            },
        ),
        pod_component("Message Queue", "message-queue", &pods, |role, name| {
            role.contains("message-queue") || name.contains("-mq-")
        }),
        pod_component("Director", "director", &pods, |role, name| {
            role.contains("battlegroup-director") || name.contains("-bgd-")
        }),
        pod_component("Gateway", "gateway", &pods, |role, name| {
            role.contains("server-gateway") || name.contains("-sgw-")
        }),
        pod_component("Text Router", "text-router", &pods, |role, name| {
            role.contains("text-router") || name.contains("-tr-")
        }),
        pod_component("File Browser", "file-browser", &pods, |role, name| {
            role.contains("filebrowser") || name.contains("-fb-")
        }),
    ];
    components.extend(server_resource_components(&resources));
    Ok(components
        .into_iter()
        .filter(|component| component.state != "Not present")
        .collect())
}

fn read_remote_component_log_tail(
    runner: &OpenSshRunner,
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
    runner: &OpenSshRunner,
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

fn pod_component(
    label: &str,
    log_key: &str,
    pods: &Value,
    matches: impl Fn(&str, &str) -> bool,
) -> RemoteServerComponent {
    let mut total = 0usize;
    let mut ready = 0usize;
    let mut restarts = 0u64;
    let mut reasons = Vec::new();
    let mut phases = Vec::new();
    for item in pods["items"].as_array().cloned().unwrap_or_default() {
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        let role = item["metadata"]["labels"]["role"]
            .as_str()
            .unwrap_or_default();
        if !matches(role, name) {
            continue;
        }
        total += 1;
        let phase = item["status"]["phase"].as_str().unwrap_or_default();
        if !phase.is_empty() {
            phases.push(phase.to_string());
        }
        let statuses = item["status"]["containerStatuses"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let pod_ready = !statuses.is_empty()
            && statuses
                .iter()
                .all(|status| status["ready"].as_bool().unwrap_or(false));
        if pod_ready || phase == "Succeeded" {
            ready += 1;
        }
        for status in statuses {
            restarts += status["restartCount"].as_u64().unwrap_or_default();
            if let Some(reason) = status["state"]["waiting"]["reason"].as_str() {
                reasons.push(reason.to_string());
            }
            if let Some(reason) = status["state"]["terminated"]["reason"].as_str() {
                if reason != "Completed" {
                    reasons.push(reason.to_string());
                }
            }
        }
    }

    if total == 0 {
        return component(
            label,
            log_key,
            "system",
            "Not present",
            "gray",
            "No matching runtime component was found.",
            vec![],
        );
    }
    let details = compact_details(vec![
        format!("{ready}/{total} pods ready"),
        if restarts > 0 {
            format!("{restarts} container restarts")
        } else {
            String::new()
        },
        if reasons.is_empty() {
            String::new()
        } else {
            format!("Reason: {}", reasons.join(", "))
        },
    ]);
    if ready == total && reasons.is_empty() {
        component(
            label,
            log_key,
            "system",
            "Ready",
            "green",
            "All pods are ready.",
            details,
        )
    } else if reasons.iter().any(|reason| is_bad_reason(reason))
        || phases.iter().any(|phase| phase == "Failed")
    {
        component(
            label,
            log_key,
            "system",
            "Problem",
            "red",
            "One or more pods are failing.",
            details,
        )
    } else {
        component(
            label,
            log_key,
            "system",
            "Starting",
            "amber",
            "Waiting for pods to become ready.",
            details,
        )
    }
}

fn server_resource_components(resources: &Value) -> Vec<RemoteServerComponent> {
    let mut items = resources["items"].as_array().cloned().unwrap_or_default();
    items.sort_by(|left, right| {
        left["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["metadata"]["name"].as_str().unwrap_or_default())
    });
    let mut output = Vec::new();
    for item in items {
        let kind = item["kind"].as_str().unwrap_or_default();
        let name = item["metadata"]["name"].as_str().unwrap_or_default();
        match kind {
            "ServerGroup" => output.push(server_group_component(&item)),
            "ServerGateway" => output.push(resource_phase_component("Gateway Resource", &item)),
            "ServerSet" => {
                if should_show_serverset(&item) {
                    output.push(serverset_component(name, &item));
                }
            }
            _ => {}
        }
    }
    output
}

fn server_group_component(item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        "Server Group",
        "server-group",
        "system",
        phase,
        format!("Server Group reports {phase}."),
        vec![],
    )
}

fn resource_phase_component(label: &str, item: &Value) -> RemoteServerComponent {
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    phase_component(
        label,
        "gateway-resource",
        "system",
        phase,
        format!("{label} reports {phase}."),
        vec![],
    )
}

fn serverset_component(name: &str, item: &Value) -> RemoteServerComponent {
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    let label = friendly_map_name(map, name);
    let phase = item["status"]["phase"].as_str().unwrap_or("Unknown");
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let ready = item["status"]["readyReplicas"].as_u64().unwrap_or_default();
    let completed = item["status"]["completedReplicas"]
        .as_u64()
        .unwrap_or_default();
    let pods = item["status"]["pods"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let game_ready = pods
        .iter()
        .filter(|pod| pod["ready"].as_bool().unwrap_or(false))
        .count();
    let details = compact_details(vec![
        format!("{ready}/{target} Kubernetes-ready replicas"),
        format!("{completed}/{target} completed game replicas"),
        format!("{game_ready}/{target} game-ready servers"),
    ]);
    let summary =
        if phase == "Initializing" && ready >= target && target > 0 && game_ready < target as usize
        {
            "Game process is running, but game readiness has not completed.".to_string()
        } else {
            format!("{label} reports {phase}.")
        };
    phase_component(
        &label,
        &serverset_log_key(name, map),
        "map",
        phase,
        summary,
        details,
    )
}

fn should_show_serverset(item: &Value) -> bool {
    let phase = item["status"]["phase"].as_str().unwrap_or_default();
    let target = item["status"]["targetReplicas"]
        .as_u64()
        .unwrap_or_default();
    let map = item["spec"]["map"].as_str().unwrap_or_default();
    phase != "Stopped" || target > 0 || matches!(map, "Survival_1" | "Overmap" | "DeepDesert_1")
}

fn phase_component(
    label: &str,
    log_key: &str,
    category: &str,
    phase: &str,
    summary: String,
    details: Vec<String>,
) -> RemoteServerComponent {
    let normalized = phase.to_ascii_lowercase();
    let (state, tone) = match normalized.as_str() {
        "healthy" | "running" | "ready" | "available" => ("Ready", "green"),
        "stopped" | "suspended" => ("Stopped", "gray"),
        "initializing" | "reconciling" | "pending" | "starting" => ("Starting", "amber"),
        "failed" | "error" | "degraded" => ("Problem", "red"),
        _ => ("Unknown", "amber"),
    };
    component(label, log_key, category, state, tone, summary, details)
}

fn component(
    name: &str,
    log_key: &str,
    category: &str,
    state: &str,
    tone: &str,
    summary: impl Into<String>,
    details: Vec<String>,
) -> RemoteServerComponent {
    RemoteServerComponent {
        name: name.to_string(),
        log_key: log_key.to_string(),
        category: category.to_string(),
        state: state.to_string(),
        tone: tone.to_string(),
        summary: summary.into(),
        details,
    }
}

fn compact_details(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn is_bad_reason(reason: &str) -> bool {
    matches!(
        reason,
        "CrashLoopBackOff"
            | "ImagePullBackOff"
            | "ErrImagePull"
            | "CreateContainerConfigError"
            | "CreateContainerError"
            | "RunContainerError"
            | "OOMKilled"
            | "Error"
    )
}

fn friendly_map_name(map: &str, fallback_name: &str) -> String {
    let normalized = map.to_ascii_lowercase();
    if normalized == "survival_1" || fallback_name.contains("survival-1") {
        return "Hagga Basin".to_string();
    }
    if normalized == "overmap" || fallback_name.contains("overmap") {
        return "Overmap".to_string();
    }
    if normalized.contains("deepdesert") || fallback_name.contains("deepdesert") {
        return "Deep Desert".to_string();
    }
    if fallback_name.contains("sh-arrakeen") {
        return "Social Hub: Arrakeen".to_string();
    }
    if fallback_name.contains("sh-harkovillage") {
        return "Social Hub: Harko Village".to_string();
    }
    if !map.is_empty() {
        return map.replace('_', " ");
    }
    "Game Server".to_string()
}

fn serverset_log_key(name: &str, map: &str) -> String {
    let combined = format!("{name} {map}").to_ascii_lowercase();
    if map.eq_ignore_ascii_case("Survival_1") || combined.contains("survival-1") {
        return "map-survival-1".to_string();
    }
    if map.eq_ignore_ascii_case("Overmap") || combined.contains("overmap") {
        return "map-overmap".to_string();
    }
    if combined.contains("deepdesert") || combined.contains("deep-desert") {
        return "map-deepdesert".to_string();
    }
    if combined.contains("sh-arrakeen") {
        return "map-social-arrakeen".to_string();
    }
    if combined.contains("sh-harkovillage") {
        return "map-social-harkovillage".to_string();
    }
    format!("map-{}", sanitize_component_key(map))
}

fn sanitize_component_key(value: &str) -> String {
    let key = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if key.is_empty() {
        "unknown".to_string()
    } else {
        key
    }
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(TunnelRegistry::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            remote_server_status,
            remote_server_components,
            start_server_tunnel,
            stop_server_tunnel,
            server_tunnel_status,
            stop_all_tunnels,
            remote_component_log_tail,
            restart_remote_component,
            start_remote_battlegroup,
            stop_remote_battlegroup,
            update_remote_battlegroup,
            detect_remote_ubuntu_servers,
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.state::<TunnelRegistry>().stop_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
