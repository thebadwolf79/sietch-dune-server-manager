use tauri::AppHandle;

mod battlegroups;
mod config_store;
mod errors;
mod models;
mod security;
mod shell;
mod ssh;
mod validation;

use config_store::*;
use errors::*;
use models::*;
use shell::*;
use ssh::*;
use validation::*;

fn configured_vm_name(app: &AppHandle, vm_name: Option<String>) -> CommandResult<String> {
    let config = read_app_config(app)?;
    required_config_value(vm_name, &config.vm_name, "VM name")
}

pub(crate) fn resolve_connection(
    app: &AppHandle,
    install_path: Option<String>,
    ip: Option<String>,
    ssh_user: Option<String>,
) -> CommandResult<(String, String, String)> {
    let config = read_app_config(app)?;
    let install_path = required_config_value(install_path, &config.install_path, "Install path")?;
    let ip = ip
        .or_else(|| discover_ip_from_logs(&install_path))
        .unwrap_or_else(|| config.vm_ip.clone())
        .trim()
        .to_string();
    if ip.is_empty() {
        return Err(failure("VM IP is not configured"));
    }
    let ssh_user = required_config_value(ssh_user, &config.ssh_user, "SSH user")?;
    Ok((install_path, ip, ssh_user))
}

#[tauri::command]
fn get_app_config(app: AppHandle) -> CommandResult<AppConfig> {
    read_app_config(&app)
}

#[tauri::command]
fn save_app_config(app: AppHandle, config: AppConfig) -> CommandResult<AppConfig> {
    write_app_config(&app, config)
}

#[tauri::command]
fn detect_app_config(app: AppHandle) -> CommandResult<AppConfig> {
    let mut config = read_app_config(&app)?;
    let detected = detect_host_config();
    config.install_path = first_non_empty(config.install_path, detected.install_path);
    config.vm_name = first_non_empty(config.vm_name, detected.vm_name);
    config.vm_ip = first_non_empty(config.vm_ip, detected.vm_ip);
    config.ssh_path = first_non_empty(config.ssh_path, detected.ssh_path);
    config.ssh_user = first_non_empty(config.ssh_user, Some("dune".to_string()));
    config.manager_api_binary_path =
        first_non_empty(config.manager_api_binary_path, detect_manager_binary_path());
    if config.manager_api_url.is_empty() && !config.vm_ip.is_empty() {
        config.manager_api_url = format!("http://{}:8787", config.vm_ip);
    }
    write_app_config(&app, config)
}

#[tauri::command]
fn get_host_status(app: AppHandle) -> CommandResult<HostStatus> {
    let config = read_app_config(&app).unwrap_or_default();
    let script = format!(
        r#"
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$vmms = Get-Service -Name vmms -ErrorAction SilentlyContinue
[pscustomobject]@{{
  user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
  isElevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  hypervAvailable = [bool](Get-Command Get-VM -ErrorAction SilentlyContinue)
  vmmsStatus = if ($vmms) {{ $vmms.Status.ToString() }} else {{ $null }}
  sshAvailable = Test-Path {ssh}
  defaultInstallPathExists = Test-Path {install}
  defaultInstallPath = {install}
}} | ConvertTo-Json -Compress
"#,
        ssh = ps_single_quoted(&config.ssh_path),
        install = ps_single_quoted(&config.install_path)
    );
    parse_json(&run_powershell(&script)?, "host status")
}

#[tauri::command]
fn get_vm_status(app: AppHandle, vm_name: Option<String>) -> CommandResult<VmStatus> {
    let vm_name = configured_vm_name(&app, vm_name)?;
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$vmName = {vm_name}
$vm = Get-VM -Name $vmName
$ips = @((Get-VMNetworkAdapter -VMName $vmName).IPAddresses | Where-Object {{ $_ -match '^\d+\.\d+\.\d+\.\d+$' }})
[pscustomobject]@{{
  name = $vm.Name
  state = $vm.State.ToString()
  status = $vm.Status
  memoryAssignedBytes = [uint64]$vm.MemoryAssigned
  uptime = $vm.Uptime.ToString()
  path = $vm.Path
  configurationLocation = $vm.ConfigurationLocation
  ipAddresses = $ips
}} | ConvertTo-Json -Compress
"#,
        vm_name = ps_single_quoted(&vm_name)
    );
    parse_json(&run_powershell(&script)?, "VM status")
}

#[tauri::command]
fn start_vm(app: AppHandle, vm_name: Option<String>) -> CommandResult<VmStatus> {
    let vm_name = configured_vm_name(&app, vm_name)?;
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$vmName = {vm_name}
Start-VM -Name $vmName | Out-Null
"#,
        vm_name = ps_single_quoted(&vm_name)
    );
    run_powershell(&script)?;
    get_vm_status(app, Some(vm_name))
}

#[tauri::command]
fn stop_vm(app: AppHandle, vm_name: Option<String>) -> CommandResult<VmStatus> {
    let vm_name = configured_vm_name(&app, vm_name)?;
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$vmName = {vm_name}
Stop-VM -Name $vmName -Force | Out-Null
"#,
        vm_name = ps_single_quoted(&vm_name)
    );
    run_powershell(&script)?;
    get_vm_status(app, Some(vm_name))
}

#[tauri::command]
fn connect_guest(
    app: AppHandle,
    install_path: Option<String>,
    ip: Option<String>,
    ssh_user: Option<String>,
) -> CommandResult<GuestConnection> {
    let (install_path, ip, ssh_user) = resolve_connection(&app, install_path, ip, ssh_user)?;
    let key = prepare_key(&app, &install_path)?;

    let identity = run_ssh(
        &app,
        &install_path,
        &ip,
        &ssh_user,
        "hostname; uname -r; sudo -n true && echo SUDO_OK; sudo kubectl version --client=true >/dev/null 2>&1 && echo KUBECTL_OK",
    )?;

    let mut lines = identity.lines();
    let hostname = lines.next().unwrap_or_default().to_string();
    let kernel = lines.next().unwrap_or_default().to_string();
    let sudo = identity.contains("SUDO_OK");
    let kubectl = identity.contains("KUBECTL_OK");

    Ok(GuestConnection {
        ip,
        ssh_user,
        key_path: key.to_string_lossy().to_string(),
        connected: true,
        sudo,
        hostname,
        kernel,
        kubectl,
    })
}


#[tauri::command]
fn install_manager_api(
    app: AppHandle,
    namespace: String,
    binary_path: String,
    token: String,
    director_base_url: String,
    install_path: Option<String>,
    ip: Option<String>,
    ssh_user: Option<String>,
) -> CommandResult<ManagerApiInstallResult> {
    let namespace = namespace.trim().to_string();
    let binary_path = binary_path.trim().to_string();
    let token = token.trim().to_string();
    let director_base_url = director_base_url.trim().trim_end_matches('/').to_string();
    validate_kube_arg(&namespace, "namespace")?;
    validate_plain_value(&binary_path, "Manager API binary")?;
    validate_plain_value(&token, "Manager API token")?;
    if !director_base_url.is_empty() {
        validate_plain_value(&director_base_url, "Director base URL")?;
    }

    let (install_path, ip, ssh_user) = resolve_connection(&app, install_path, ip, ssh_user)?;
    let upload_path = format!("/home/{ssh_user}/dune-manager-api");
    copy_to_guest(
        &app,
        &install_path,
        &ip,
        &ssh_user,
        &binary_path,
        &upload_path,
    )?;

    let install_script = format!(
        r#"set -eu
install -d -m 0755 /opt/dune-manager
install -m 0755 {upload_path} /opt/dune-manager/dune-manager-api
rm -f {upload_path}
cat > /etc/dune-manager-api.env <<'EOF'
MANAGER_API_TOKEN={token}
DUNE_NAMESPACE={namespace}
KUBECONFIG=/etc/rancher/k3s/k3s.yaml
DIRECTOR_BASE_URL={director_base_url}
PORT=8787
RUST_LOG=dune_manager_api=info,tower_http=info
EOF
chmod 0600 /etc/dune-manager-api.env
cat > /opt/dune-manager/run-manager-api <<'EOF'
#!/bin/sh
set -a
. /etc/dune-manager-api.env
set +a
exec /opt/dune-manager/dune-manager-api
EOF
chmod 0755 /opt/dune-manager/run-manager-api
cat > /etc/init.d/dune-manager-api <<'EOF'
#!/sbin/openrc-run
name="Dune Manager API"
description="Dune dedicated server manager guest service"
command="/opt/dune-manager/run-manager-api"
command_background="yes"
pidfile="/run/dune-manager-api.pid"
output_log="/var/log/dune-manager-api.log"
error_log="/var/log/dune-manager-api.log"
depend() {{
  need net
  after k3s
}}
EOF
chmod 0755 /etc/init.d/dune-manager-api
rc-update add dune-manager-api default >/dev/null 2>&1 || true
rc-service dune-manager-api restart
"#,
        upload_path = upload_path,
        token = token,
        namespace = namespace,
        director_base_url = director_base_url,
    );

    run_ssh_with_stdin(
        &app,
        &install_path,
        &ip,
        &ssh_user,
        "sudo sh -s",
        &install_script,
    )?;
    run_ssh(
        &app,
        &install_path,
        &ip,
        &ssh_user,
        "sudo rc-service dune-manager-api status",
    )?;

    Ok(ManagerApiInstallResult {
        namespace,
        deployment: "dune-manager-api".to_string(),
        service: "openrc".to_string(),
        binary_path,
        url: format!("http://{ip}:8787"),
    })
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            detect_app_config,
            get_host_status,
            get_vm_status,
            start_vm,
            stop_vm,
            connect_guest,
            battlegroups::get_battlegroups,
            battlegroups::get_battlegroup_detail,
            battlegroups::get_workloads,
            battlegroups::start_battlegroup,
            battlegroups::stop_battlegroup,
            battlegroups::restart_battlegroup,
            battlegroups::export_live_config,
            install_manager_api
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
