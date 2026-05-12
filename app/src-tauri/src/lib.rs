use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};

use dune_manager_core::environment::{detect_setup_environment, SetupEnvironment};
use dune_manager_core::models::CommandResult;
use dune_manager_core::orchestration::{
    is_started_state, BattlegroupManagementOrchestrator, BattlegroupRef, CreatedWorld,
    DuneVmCandidate, DuneVmDetector, ExperimentalSwapOrchestrator, ExperimentalSwapRequest,
    GuestBootstrapPlan, GuestBootstrapProvider, GuestNetworkConfig, GuestNetworkPlan,
    HyperVInitialSetupOrchestrator, HyperVInitialSetupRequest, HyperVVmSetupRequest, InstanceMap,
    KubernetesProvider, ManagerApiInstallRequest, ManagerApiInstaller, ManagerApiServiceManager,
    MapInstanceOrchestrator, MemoryProfile, OpenSshGuestProvider, OpenSshRunner, OpenSshTarget,
    OperationSink, OrchestrationEvent, RemoteCommandRunner, SetMapInstancesRequest,
    SshGuestBootstrapProvider, StrictPowerShellHyperV, StructuredKubectl, UbuntuSshPreflight,
    UbuntuSshPrepareRequest, UbuntuSshSetup, VmProvider, WorldManifestRequest,
};
use dune_manager_core::shell::{ps_single_quoted, run_powershell};
use dune_manager_core::toolchain::{
    default_server_package_dir, default_vm_destination, prepare_vendor_ssh_key, ManagedTool,
    Toolchain,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{path::BaseDirectory, Emitter, Manager};

#[tauri::command]
async fn detect_environment() -> Result<SetupEnvironment, String> {
    tauri::async_runtime::spawn_blocking(detect_setup_environment)
        .await
        .map_err(|err| format!("Environment detection worker failed: {err}"))?
        .map_err(|err| err.message)
}

#[tauri::command]
async fn default_vm_location() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        default_vm_destination()
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|err| err.message)
    })
    .await
    .map_err(|err| format!("Default VM location worker failed: {err}"))?
}

#[tauri::command]
async fn vm_destination_has_vm(path: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if path.trim().is_empty() {
            return Ok(false);
        }
        Ok(destination_has_vm_artifacts(std::path::Path::new(&path)))
    })
    .await
    .map_err(|err| format!("Path check worker failed: {err}"))?
}

#[tauri::command]
async fn detect_dune_vms() -> Result<Vec<DuneVmCandidate>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        DuneVmDetector::new(StrictPowerShellHyperV::new())
            .detect()
            .map_err(|err| err.message)
    })
    .await
    .map_err(|err| format!("Dune VM detection worker failed: {err}"))?
}

fn destination_has_vm_artifacts(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    if path.join("Virtual Machines").is_dir() || path.join("Virtual Hard Disks").is_dir() {
        return true;
    }
    path.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            let path = entry.path();
            path.extension().is_some_and(|extension| {
                ["vmcx", "vmrs", "vhd", "vhdx"]
                    .iter()
                    .any(|candidate| extension.to_string_lossy().eq_ignore_ascii_case(candidate))
            })
        })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupRequest {
    vm_destination: String,
    vm_name: String,
    disk_gb: u64,
    memory_gb: u64,
    processor_count: u32,
    enable_swap: bool,
    network_mode: String,
    switch_name: String,
    adapter_name: String,
    static_ip: String,
    gateway: String,
    dns: String,
    player_ip: String,
    world_name: String,
    region: String,
    self_host_token: String,
    survival_instances: usize,
    deep_desert_pve_instances: usize,
    deep_desert_pvp_instances: usize,
    deep_desert_warm_servers: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSetupRequest {
    host: String,
    user: String,
    key_path: String,
    player_ip: String,
    world_name: String,
    region: String,
    self_host_token: String,
    survival_instances: usize,
    deep_desert_pve_instances: usize,
    deep_desert_pvp_instances: usize,
    deep_desert_warm_servers: usize,
    enable_swap: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteConnectionRequest {
    host: String,
    key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerActionRequest {
    host: String,
    user: String,
    key_path: String,
    namespace: String,
    battlegroup_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteManagerApiActionRequest {
    host: String,
    user: String,
    key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagerApiProbeRequest {
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
struct RemoteManagerApiServiceStatus {
    installed: bool,
    running: bool,
    health_reachable: bool,
    service_manager: String,
    raw_state: String,
    port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerStatus {
    battlegroup: RemoteBattlegroupStatus,
    manager_api: RemoteManagerApiServiceStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagerApiProbeResult {
    url: String,
    reachable: bool,
    ok: bool,
    api_version: String,
    namespace: String,
    auth_enabled: bool,
    director_configured: bool,
    error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupLogPayload {
    level: &'static str,
    scope: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupRunResult {
    namespace: String,
    battlegroup_name: String,
    world_unique_name: String,
    director_node_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSetupRunResult {
    namespace: String,
    battlegroup_name: String,
    world_unique_name: String,
    manager_api_url: String,
    preflight: UbuntuSshPreflight,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteServerRecord {
    id: String,
    name: String,
    host: String,
    user: String,
    key_path: String,
    namespace: String,
    battlegroup_name: String,
    world_unique_name: String,
    manager_api_url: String,
    phase: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RollbackRequest {
    vm_name: String,
    vm_destination: String,
    switch_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RollbackResult {
    vm_removed: bool,
    files_removed: bool,
    switch_removed: bool,
}

struct TauriOperationSink {
    app: tauri::AppHandle,
}

impl TauriOperationSink {
    fn info(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "setup-log",
            SetupLogPayload {
                level: "info",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }

    fn error(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "setup-log",
            SetupLogPayload {
                level: "error",
                scope: scope.into(),
                message: message.into(),
            },
        );
    }

    fn warn(&self, scope: impl Into<String>, message: impl Into<String>) {
        let _ = self.app.emit(
            "setup-log",
            SetupLogPayload {
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
async fn start_full_setup(
    app: tauri::AppHandle,
    request: SetupRequest,
) -> Result<SetupRunResult, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.info("setup", "Starting full setup workflow.");
        let manager_api_binary = bundled_manager_api_binary(&sink.app);
        match run_full_setup(request, manager_api_binary, &mut sink) {
            Ok(result) => {
                sink.info("setup", "Full setup workflow completed.");
                Ok(result)
            }
            Err(err) => {
                sink.error("setup", err.message.clone());
                if !err.stderr.trim().is_empty() {
                    sink.error("stderr", err.stderr);
                }
                if !err.stdout.trim().is_empty() {
                    sink.error("stdout", err.stdout);
                }
                Err("Setup failed; see setup log for details.".to_string())
            }
        }
    })
    .await
    .map_err(|err| format!("Setup worker failed: {err}"))?
}

#[tauri::command]
async fn preflight_remote_ubuntu(
    request: RemoteSetupRequest,
) -> Result<UbuntuSshPreflight, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let toolchain = Toolchain::from_default_root().map_err(|err| err.message)?;
        toolchain
            .install(ManagedTool::OpenSsh, false, None)
            .map_err(|err| err.message)?;
        let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
        let runner = OpenSshRunner::new(OpenSshTarget::new(
            ssh_path,
            PathBuf::from(request.key_path),
            request.user,
            request.host,
        ));
        UbuntuSshSetup::new(runner)
            .preflight()
            .map_err(|err| err.message)
    })
    .await
    .map_err(|err| format!("Remote preflight worker failed: {err}"))?
}

#[tauri::command]
async fn detect_remote_ubuntu_servers(
    request: RemoteConnectionRequest,
) -> Result<Vec<RemoteServerRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let toolchain = Toolchain::from_default_root().map_err(|err| err.message)?;
        toolchain
            .install(ManagedTool::OpenSsh, false, None)
            .map_err(|err| err.message)?;
        let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
        let runner = OpenSshRunner::new(OpenSshTarget::new(
            ssh_path,
            PathBuf::from(&request.key_path),
            "root",
            request.host.clone(),
        ));
        let value = runner
            .run_json(
                "sudo kubectl get battlegroups -A -o json",
                "remote ubuntu battlegroups",
            )
            .map_err(|err| err.message)?;
        Ok(remote_records_from_battlegroups(&request, &value))
    })
    .await
    .map_err(|err| format!("Remote server detection worker failed: {err}"))?
}

#[tauri::command]
async fn check_manager_api(
    request: ManagerApiProbeRequest,
) -> Result<ManagerApiProbeResult, String> {
    tauri::async_runtime::spawn_blocking(move || probe_manager_api(&request.url))
        .await
        .map_err(|err| format!("Manager API probe worker failed: {err}"))
}

#[tauri::command]
async fn remote_server_status(
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runner = remote_runner(request.host, request.user, request.key_path)?;
        read_remote_server_status(&runner, &request.namespace, &request.battlegroup_name)
            .map_err(|err| err.message)
    })
    .await
    .map_err(|err| format!("Remote status worker failed: {err}"))?
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

async fn run_remote_battlegroup_action(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.info("bg.check", "Checking remote battlegroup state.");
        let runner = remote_runner(request.host, request.user, request.key_path)?;
        let kubernetes = StructuredKubectl::new(runner.clone());
        let before = kubernetes
            .battlegroup_state(&request.namespace, &request.battlegroup_name)
            .map_err(|err| err.message)?;
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
            namespace: request.namespace,
            name: request.battlegroup_name,
        };
        let manager = BattlegroupManagementOrchestrator::new(kubernetes);
        if stop {
            manager
                .stop(&battlegroup, &mut sink)
                .map_err(|err| err.message)?;
        } else {
            manager
                .start_and_wait_director(&battlegroup, 180, &mut sink)
                .map_err(|err| err.message)?;
        }
        sink.info("bg.check", "Refreshing remote battlegroup state.");
        read_remote_server_status(&runner, &battlegroup.namespace, &battlegroup.name)
            .map_err(|err| err.message)
    })
    .await
    .map_err(|err| format!("Remote battlegroup action worker failed: {err}"))?
}

#[tauri::command]
async fn start_remote_manager_api(
    app: tauri::AppHandle,
    request: RemoteManagerApiActionRequest,
) -> Result<RemoteManagerApiServiceStatus, String> {
    run_remote_manager_api_action(app, request, "start").await
}

#[tauri::command]
async fn stop_remote_manager_api(
    app: tauri::AppHandle,
    request: RemoteManagerApiActionRequest,
) -> Result<RemoteManagerApiServiceStatus, String> {
    run_remote_manager_api_action(app, request, "stop").await
}

async fn run_remote_manager_api_action(
    app: tauri::AppHandle,
    request: RemoteManagerApiActionRequest,
    action: &'static str,
) -> Result<RemoteManagerApiServiceStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let sink = TauriOperationSink { app: worker_app };
        sink.info(
            "manager-api.check",
            "Checking remote Manager API service state.",
        );
        let runner = remote_runner(request.host, request.user, request.key_path)?;
        let installer = ManagerApiInstaller::new(runner);
        let status = match action {
            "start" => installer.start_service("dune-manager-api", 8787),
            "stop" => installer.stop_service("dune-manager-api", 8787),
            _ => unreachable!("unsupported Manager API action"),
        }
        .map_err(|err| err.message)?;
        sink.info(
            "manager-api.check",
            "Remote Manager API service state refreshed.",
        );
        Ok(remote_manager_api_status(status))
    })
    .await
    .map_err(|err| format!("Remote Manager API action worker failed: {err}"))?
}

#[tauri::command]
async fn start_remote_ubuntu_setup(
    app: tauri::AppHandle,
    request: RemoteSetupRequest,
) -> Result<RemoteSetupRunResult, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.warn(
            "ubuntu",
            "Remote Ubuntu setup can modify packages, users, k3s, firewall state, and server files on the target host.",
        );
        let manager_api_binary = bundled_manager_api_binary(&sink.app);
        match run_remote_ubuntu_setup(request, manager_api_binary, &mut sink) {
            Ok(result) => {
                sink.info("ubuntu", "Remote Ubuntu setup completed.");
                Ok(result)
            }
            Err(err) => {
                sink.error("ubuntu", err.message.clone());
                if !err.stderr.trim().is_empty() {
                    sink.error("stderr", err.stderr);
                }
                if !err.stdout.trim().is_empty() {
                    sink.error("stdout", err.stdout);
                }
                Err("Remote Ubuntu setup failed; see setup log for details.".to_string())
            }
        }
    })
    .await
    .map_err(|err| format!("Remote Ubuntu setup worker failed: {err}"))?
}

#[tauri::command]
async fn rollback_setup(
    app: tauri::AppHandle,
    request: RollbackRequest,
) -> Result<RollbackResult, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let sink = TauriOperationSink { app: worker_app };
        sink.warn("rollback", "Rolling back setup artifacts.");
        rollback_setup_inner(request, &sink).map_err(|err| {
            sink.error("rollback", err.message.clone());
            err.message
        })
    })
    .await
    .map_err(|err| format!("Rollback worker failed: {err}"))?
}

fn rollback_setup_inner(
    request: RollbackRequest,
    sink: &TauriOperationSink,
) -> CommandResult<RollbackResult> {
    let provider = StrictPowerShellHyperV::new();
    let mut vm_removed = false;
    if provider.get_vm(&request.vm_name)?.is_some() {
        sink.warn(
            "rollback",
            format!("Removing Hyper-V VM '{}'.", request.vm_name),
        );
        provider.remove_vm(&request.vm_name)?;
        vm_removed = true;
    }

    let destination = PathBuf::from(&request.vm_destination);
    let mut files_removed = false;
    if destination.exists()
        && (destination_has_vm_artifacts(&destination) || is_empty_dir(&destination))
    {
        sink.warn(
            "rollback",
            format!("Removing VM files at {}.", destination.display()),
        );
        std::fs::remove_dir_all(&destination).map_err(|err| {
            dune_manager_core::errors::failure(format!(
                "Failed to remove VM files at {}: {err}",
                destination.display()
            ))
        })?;
        files_removed = true;
    }

    let switch_removed = remove_switch_if_unused(&request.switch_name)?;
    if switch_removed {
        sink.warn(
            "rollback",
            format!("Removed unused Hyper-V switch '{}'.", request.switch_name),
        );
    } else {
        sink.info(
            "rollback",
            format!(
                "Hyper-V switch '{}' was left in place because it is missing or still used.",
                request.switch_name
            ),
        );
    }

    Ok(RollbackResult {
        vm_removed,
        files_removed,
        switch_removed,
    })
}

fn run_full_setup(
    request: SetupRequest,
    manager_api_binary: Option<PathBuf>,
    sink: &mut TauriOperationSink,
) -> CommandResult<SetupRunResult> {
    let toolchain = Toolchain::from_default_root()?;
    let server_package_dir = default_server_package_dir()?;
    let provider = StrictPowerShellHyperV::new();
    let vm_destination = PathBuf::from(&request.vm_destination);

    sink.info("setup", "Checking VM destination and existing VM state.");
    if destination_has_vm_artifacts(&vm_destination) {
        return Err(dune_manager_core::errors::failure(format!(
            "VM Location already contains VM files: {}. Choose another destination or remove the existing VM files first.",
            vm_destination.display()
        )));
    }
    if provider.get_vm(&request.vm_name)?.is_some() {
        return Err(dune_manager_core::errors::failure(format!(
            "A Hyper-V VM named '{}' already exists. Remove it or choose a different VM Name before setup.",
            request.vm_name
        )));
    }

    sink.info("tools", "Installing or validating SteamCMD.");
    toolchain.install(ManagedTool::SteamCmd, false, None)?;
    sink.info("tools", "Installing or validating OpenSSH.");
    toolchain.install(ManagedTool::OpenSsh, false, None)?;

    sink.info("steam", "Installing or validating the server package.");
    toolchain.install_server_package(&server_package_dir)?;

    sink.info("ssh", "Preparing the vendor VM SSH key.");
    let ssh_key = prepare_vendor_ssh_key(&server_package_dir)?;
    let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;

    if !request.network_mode.eq_ignore_ascii_case("static") {
        return Err(dune_manager_core::errors::failure(
            "Full setup currently requires Static internal IP mode so bootstrap can continue after the first boot",
        ));
    }

    let environment = detect_setup_environment()?;
    let adapter = environment
        .network_adapters
        .iter()
        .find(|adapter| adapter.name == request.adapter_name)
        .ok_or_else(|| {
            dune_manager_core::errors::failure(
                "Selected network adapter was not found in the current host environment",
            )
        })?;

    let address_cidr = format!("{}/{}", request.static_ip.trim(), adapter.prefix_length);
    let guest_network = GuestNetworkPlan::Static(GuestNetworkConfig {
        interface: "eth0".to_string(),
        address_cidr,
        gateway: request.gateway.clone(),
        dns: request.dns.clone(),
    });

    let guest_plan = GuestBootstrapPlan::from_self_host_token(
        request.player_ip.clone(),
        request.world_name.clone(),
        request.region.clone(),
        request.self_host_token.clone(),
    )?;
    let bootstrap_target = OpenSshTarget::new(
        ssh_path.clone(),
        ssh_key.clone(),
        "dune",
        request.static_ip.clone(),
    );
    let bootstrap_runner = OpenSshRunner::new(bootstrap_target.clone());
    let initial = HyperVInitialSetupRequest {
        vm: HyperVVmSetupRequest {
            install_path: server_package_dir.clone(),
            vm_name: request.vm_name.clone(),
            destination_path: vm_destination,
            switch_name: request.switch_name.clone(),
            adapter_name: request.adapter_name.clone(),
            memory: MemoryProfile::CustomBytes(
                request.memory_gb.saturating_mul(1024 * 1024 * 1024),
            ),
            processor_count: request.processor_count,
            replace_existing_vm: false,
            clear_destination: false,
            disk_size_bytes: request.disk_gb.saturating_mul(1024 * 1024 * 1024),
        },
        guest_network,
        guest_bootstrap: guest_plan,
        vm_ip_timeout_seconds: 180,
        ssh_timeout_seconds: 180,
    };

    let result = HyperVInitialSetupOrchestrator::new(
        &provider,
        &provider,
        OpenSshGuestProvider::new(ssh_path, ssh_key, "dune"),
        SshGuestBootstrapProvider::new(bootstrap_runner.clone()),
    )
    .run(&initial, sink)?;

    apply_instance_layout(
        &request,
        &result.bootstrap.namespace,
        &result.bootstrap.battlegroup_name,
        &bootstrap_runner,
        sink,
    )?;

    if request.enable_swap {
        sink.info("guest-swap", "Enabling experimental swap profile.");
        let mut swap = ExperimentalSwapRequest::new(
            result.bootstrap.namespace.clone(),
            result.bootstrap.battlegroup_name.clone(),
        );
        swap.restart_k3s = true;
        ExperimentalSwapOrchestrator::new(bootstrap_runner.clone()).enable(&swap, sink)?;
    }

    let battlegroup = BattlegroupRef {
        namespace: result.bootstrap.namespace.clone(),
        name: result.bootstrap.battlegroup_name.clone(),
    };
    sink.info("bg", "Starting battlegroup after setup.");
    let director_node_port =
        BattlegroupManagementOrchestrator::new(StructuredKubectl::new(bootstrap_runner.clone()))
            .start_and_wait_director(&battlegroup, 180, sink)?;
    if let Some(port) = director_node_port {
        sink.info(
            "director",
            format!("Director is available on NodePort {port}."),
        );
    }
    let Some(binary_path) = manager_api_binary else {
        return Err(dune_manager_core::errors::failure(
            "Bundled Manager API binary was not found; setup cannot finish without the Manager API.",
        ));
    };
    sink.info(
        "manager-api",
        "Installing Manager API with the Self-Host Service Token.",
    );
    let manager_request = ManagerApiInstallRequest::new(
        binary_path,
        request.self_host_token.trim().to_string(),
        result.bootstrap.namespace.clone(),
    );
    ManagerApiInstaller::new(bootstrap_runner).install(&manager_request, sink)?;
    sink.info("manager-api", "Manager API installed and healthy.");

    Ok(SetupRunResult {
        namespace: result.bootstrap.namespace,
        battlegroup_name: result.bootstrap.battlegroup_name,
        world_unique_name: result.bootstrap.world_unique_name,
        director_node_port,
    })
}

fn run_remote_ubuntu_setup(
    request: RemoteSetupRequest,
    manager_api_binary: Option<PathBuf>,
    sink: &mut TauriOperationSink,
) -> CommandResult<RemoteSetupRunResult> {
    let toolchain = Toolchain::from_default_root()?;
    toolchain.install(ManagedTool::OpenSsh, false, None)?;
    let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
    let target = OpenSshTarget::new(
        ssh_path,
        PathBuf::from(&request.key_path),
        request.user.clone(),
        request.host.clone(),
    );
    let runner = OpenSshRunner::new(target);
    let ubuntu = UbuntuSshSetup::new(runner.clone());
    let prepare = UbuntuSshPrepareRequest::default();

    sink.info("ubuntu.preflight", "Checking remote Ubuntu host resources.");
    let preflight = ubuntu.preflight()?;
    if !preflight.passwordless_sudo && preflight.uid != 0 {
        return Err(dune_manager_core::errors::failure(
            "Remote setup requires root login or passwordless sudo.",
        ));
    }

    ubuntu.prepare_host(&prepare, sink)?;
    ubuntu.install_k3s(&prepare, sink)?;
    let payload = ubuntu.install_server_payload(&prepare, sink)?;
    if !payload.setup_script_present || !payload.battlegroup_script_present {
        return Err(dune_manager_core::errors::failure(format!(
            "Dune server payload is incomplete at {} (setup script present: {}, battlegroup script present: {}).",
            payload.download_path, payload.setup_script_present, payload.battlegroup_script_present
        )));
    }
    ubuntu.bootstrap_kubernetes(&prepare, sink)?;

    let plan = GuestBootstrapPlan::from_self_host_token(
        request.player_ip.clone(),
        request.world_name.clone(),
        request.region.clone(),
        request.self_host_token.clone(),
    )?;
    plan.validate()?;
    ubuntu.write_player_settings(&plan.player_ip, sink)?;

    let provider = SshGuestBootstrapProvider::new(runner.clone());
    let world_request = WorldManifestRequest {
        world_name: plan.world_name.clone(),
        world_region: plan.world_region.clone(),
        world_unique_name: plan.world_unique_name(),
        self_host_token: plan.self_host_token.clone(),
    };
    let created = ensure_remote_world(&provider, &runner, &world_request, sink)?;
    sink.info("ubuntu.helper.install", "Installing battlegroup helper.");
    provider.install_battlegroup_helper()?;
    sink.info(
        "ubuntu.images.import",
        "Importing BattleGroup container images.",
    );
    provider.import_battlegroup_images()?;
    sink.info("ubuntu.images.patch", "Patching BattleGroup image tags.");
    provider.patch_battlegroup_images(&created.namespace, &created.battlegroup_name)?;
    ubuntu.use_default_scheduler(&created.namespace, &created.battlegroup_name, sink)?;
    sink.info("ubuntu.defaults.apply", "Applying default user settings.");
    provider.apply_default_user_settings(&created.namespace, &created.battlegroup_name)?;

    let layout_request = SetupRequest {
        vm_destination: String::new(),
        vm_name: String::new(),
        disk_gb: 0,
        memory_gb: 0,
        processor_count: 0,
        enable_swap: request.enable_swap,
        network_mode: "static".to_string(),
        switch_name: String::new(),
        adapter_name: String::new(),
        static_ip: String::new(),
        gateway: String::new(),
        dns: String::new(),
        player_ip: request.player_ip.clone(),
        world_name: request.world_name.clone(),
        region: request.region.clone(),
        self_host_token: request.self_host_token.clone(),
        survival_instances: request.survival_instances,
        deep_desert_pve_instances: request.deep_desert_pve_instances,
        deep_desert_pvp_instances: request.deep_desert_pvp_instances,
        deep_desert_warm_servers: request.deep_desert_warm_servers,
    };
    apply_instance_layout(
        &layout_request,
        &created.namespace,
        &created.battlegroup_name,
        &runner,
        sink,
    )?;

    if request.enable_swap {
        sink.info("ubuntu.swap", "Enabling experimental swap profile.");
        let mut swap = ExperimentalSwapRequest::new(
            created.namespace.clone(),
            created.battlegroup_name.clone(),
        );
        swap.restart_k3s = true;
        ExperimentalSwapOrchestrator::new(runner.clone()).enable(&swap, sink)?;
    }

    let Some(binary_path) = manager_api_binary else {
        return Err(dune_manager_core::errors::failure(
            "Bundled Manager API binary was not found; setup cannot finish without the Manager API.",
        ));
    };
    sink.info(
        "manager-api",
        "Installing Manager API with the Self-Host Service Token.",
    );
    let mut manager_request = ManagerApiInstallRequest::new(
        binary_path,
        request.self_host_token.trim().to_string(),
        created.namespace.clone(),
    );
    manager_request.service_manager = ManagerApiServiceManager::Systemd;
    ManagerApiInstaller::new(runner.clone()).install(&manager_request, sink)?;
    sink.info("manager-api", "Manager API installed and healthy.");

    let battlegroup = BattlegroupRef {
        namespace: created.namespace.clone(),
        name: created.battlegroup_name.clone(),
    };
    sink.info("bg", "Starting battlegroup after remote setup.");
    let _director_node_port =
        BattlegroupManagementOrchestrator::new(StructuredKubectl::new(runner.clone()))
            .start_and_wait_director(&battlegroup, 180, sink)?;

    Ok(RemoteSetupRunResult {
        namespace: created.namespace,
        battlegroup_name: created.battlegroup_name,
        world_unique_name: plan.world_unique_name(),
        manager_api_url: format!("http://{}:8787", request.host),
        preflight,
    })
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
    Some(RemoteServerRecord {
        id: format!("ubuntu:{}:{}:{}", request.host, namespace, battlegroup_name),
        name: title,
        host: request.host.clone(),
        user: "root".to_string(),
        key_path: request.key_path.clone(),
        namespace,
        battlegroup_name: battlegroup_name.clone(),
        world_unique_name: battlegroup_name,
        manager_api_url: format!("http://{}:8787", request.host),
        phase,
    })
}

fn ensure_remote_world<R>(
    provider: &SshGuestBootstrapProvider<R>,
    runner: &R,
    request: &WorldManifestRequest,
    sink: &TauriOperationSink,
) -> CommandResult<CreatedWorld>
where
    R: RemoteCommandRunner + Clone,
{
    let namespace = format!("funcom-seabass-{}", request.world_unique_name);
    let battlegroup_name = request.world_unique_name.clone();
    let existing = StructuredKubectl::new(runner.clone())
        .battlegroup_state(&namespace, &battlegroup_name)
        .is_ok();
    if existing {
        sink.warn(
            "ubuntu.world.create",
            format!("Existing BattleGroup {namespace}/{battlegroup_name} found; resuming setup."),
        );
        return Ok(CreatedWorld {
            namespace,
            battlegroup_name,
        });
    }

    sink.info(
        "ubuntu.world.create",
        "Creating BattleGroup world resources.",
    );
    provider.create_world(request)
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

fn read_remote_server_status(
    runner: &OpenSshRunner,
    namespace: &str,
    battlegroup_name: &str,
) -> CommandResult<RemoteServerStatus> {
    let kubernetes = StructuredKubectl::new(runner.clone());
    let battlegroup = kubernetes.battlegroup_state(namespace, battlegroup_name)?;
    let manager_api =
        ManagerApiInstaller::new(runner.clone()).service_status("dune-manager-api", 8787)?;
    Ok(RemoteServerStatus {
        battlegroup: RemoteBattlegroupStatus {
            stop: battlegroup.stop,
            phase: battlegroup.phase,
            server_group_phase: battlegroup.server_group_phase,
            director_phase: battlegroup.director_phase,
        },
        manager_api: remote_manager_api_status(manager_api),
    })
}

fn remote_manager_api_status(
    status: dune_manager_core::orchestration::ManagerApiServiceStatus,
) -> RemoteManagerApiServiceStatus {
    RemoteManagerApiServiceStatus {
        installed: status.installed,
        running: status.running,
        health_reachable: status.health_reachable,
        service_manager: status.service_manager,
        raw_state: status.raw_state,
        port: status.port,
    }
}

fn probe_manager_api(url: &str) -> ManagerApiProbeResult {
    let normalized = normalize_manager_api_url(url);
    match probe_manager_api_health(&normalized) {
        Ok(value) => ManagerApiProbeResult {
            url: normalized,
            reachable: true,
            ok: value.get("ok").and_then(Value::as_bool).unwrap_or(false),
            api_version: value
                .get("apiVersion")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            namespace: value
                .get("namespace")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            auth_enabled: value
                .get("authEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            director_configured: value
                .get("directorConfigured")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            error: String::new(),
        },
        Err(error) => ManagerApiProbeResult {
            url: normalized,
            reachable: false,
            ok: false,
            api_version: String::new(),
            namespace: String::new(),
            auth_enabled: false,
            director_configured: false,
            error,
        },
    }
}

fn probe_manager_api_health(url: &str) -> Result<Value, String> {
    let target = parse_http_url(url)?;
    let mut stream = TcpStream::connect_timeout(&target.address, Duration::from_secs(2))
        .map_err(|err| format!("Manager API is not reachable: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|err| format!("Failed to configure Manager API read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|err| format!("Failed to configure Manager API write timeout: {err}"))?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n\r\n",
        target.path, target.host_header
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("Failed to request Manager API health: {err}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("Failed to read Manager API health: {err}"))?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "Manager API returned an invalid HTTP response.".to_string())?;
    let status_line = headers.lines().next().unwrap_or_default();
    if !status_line.contains(" 200 ") {
        return Err(format!("Manager API health returned {status_line}."));
    }
    serde_json::from_str(body.trim())
        .map_err(|err| format!("Manager API health was not JSON: {err}"))
}

struct HttpTarget {
    address: std::net::SocketAddr,
    host_header: String,
    path: String,
}

fn parse_http_url(url: &str) -> Result<HttpTarget, String> {
    let stripped = url
        .trim()
        .strip_prefix("http://")
        .ok_or_else(|| "Only http:// Manager API URLs are supported right now.".to_string())?;
    let (authority, path) = stripped.split_once('/').unwrap_or((stripped, "health"));
    let path = format!("/{}", path.trim_start_matches('/'));
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => {
            let port = port
                .parse::<u16>()
                .map_err(|_| "Manager API URL port is invalid.".to_string())?;
            (host, port)
        }
        None => (authority, 80),
    };
    if host.trim().is_empty() {
        return Err("Manager API URL host is empty.".to_string());
    }
    let mut addresses = (host, port)
        .to_socket_addrs()
        .map_err(|err| format!("Failed to resolve Manager API host: {err}"))?;
    let address = addresses
        .next()
        .ok_or_else(|| "Manager API host did not resolve.".to_string())?;
    Ok(HttpTarget {
        address,
        host_header: authority.to_string(),
        path,
    })
}

fn normalize_manager_api_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.ends_with("/health") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/health")
    }
}

fn bundled_manager_api_binary(app: &tauri::AppHandle) -> Option<PathBuf> {
    let candidates = [
        app.path()
            .resolve("manager-api/dune-manager-api", BaseDirectory::Resource)
            .ok(),
        app.path()
            .resolve(
                "manager-api/dune-manager-api-x86_64-unknown-linux-musl",
                BaseDirectory::Resource,
            )
            .ok(),
    ];

    candidates.into_iter().flatten().find(|path| path.is_file())
}

fn apply_instance_layout<R>(
    request: &SetupRequest,
    namespace: &str,
    battlegroup_name: &str,
    runner: &R,
    sink: &mut TauriOperationSink,
) -> CommandResult<()>
where
    R: RemoteCommandRunner + Clone,
{
    let battlegroup = dune_manager_core::orchestration::BattlegroupRef {
        namespace: namespace.to_string(),
        name: battlegroup_name.to_string(),
    };
    let orchestrator = MapInstanceOrchestrator::new(runner.clone());

    sink.info("layout", "Applying Hagga Basin instance count.");
    orchestrator.set_instances(&SetMapInstancesRequest::new(
        battlegroup.clone(),
        InstanceMap::Survival1,
        request.survival_instances,
    ))?;

    let deep_desert_total = request.deep_desert_pve_instances + request.deep_desert_pvp_instances;
    if deep_desert_total > 1 {
        return Err(dune_manager_core::errors::failure(
            "Only one Deep Desert instance is supported in this build.",
        ));
    }
    if deep_desert_total > 0 {
        sink.info("layout", "Applying Deep Desert instance count.");
        let mut deep_desert =
            SetMapInstancesRequest::new(battlegroup, InstanceMap::DeepDesert, deep_desert_total);
        deep_desert.pvp_instance_count = Some(request.deep_desert_pvp_instances);
        orchestrator.set_instances(&deep_desert)?;
    } else {
        sink.info(
            "layout",
            "Deep Desert is disabled; skipping Deep Desert instance patch.",
        );
    }

    if request.deep_desert_warm_servers > 0 {
        return Err(dune_manager_core::errors::failure(
            "Warm Deep Desert instances are not wired to Kubernetes yet; set Warm Deep Desert Instances to 0 for this build.",
        ));
    }

    Ok(())
}

fn is_empty_dir(path: &std::path::Path) -> bool {
    path.is_dir()
        && path
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
}

fn remove_switch_if_unused(switch_name: &str) -> CommandResult<bool> {
    if switch_name.trim().is_empty() {
        return Ok(false);
    }
    let output = run_powershell(&format!(
        r#"
$ErrorActionPreference = 'Stop'
$switchName = {switch_name}
$switch = Get-VMSwitch -Name $switchName -ErrorAction SilentlyContinue
if (-not $switch) {{
  [Console]::Out.WriteLine('false')
  exit 0
}}
$usedByVms = @(Get-VMNetworkAdapter -All -ErrorAction SilentlyContinue | Where-Object {{ $_.SwitchName -eq $switchName }})
if ($usedByVms.Count -eq 0) {{
  Remove-VMSwitch -Name $switchName -Force -ErrorAction Stop
  [Console]::Out.WriteLine('true')
}} else {{
  [Console]::Out.WriteLine('false')
}}
"#,
        switch_name = ps_single_quoted(switch_name),
    ))?;
    Ok(output.trim().eq_ignore_ascii_case("true"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            detect_environment,
            default_vm_location,
            vm_destination_has_vm,
            check_manager_api,
            remote_server_status,
            start_remote_battlegroup,
            stop_remote_battlegroup,
            start_remote_manager_api,
            stop_remote_manager_api,
            detect_dune_vms,
            detect_remote_ubuntu_servers,
            preflight_remote_ubuntu,
            start_full_setup,
            start_remote_ubuntu_setup,
            rollback_setup
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
