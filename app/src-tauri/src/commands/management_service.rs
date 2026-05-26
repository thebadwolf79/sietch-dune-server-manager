use std::path::PathBuf;

use base64::Engine as _;
use dune_manager_core::orchestration::{RemoteCommandRunner, RusshRunner, RusshTarget};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::commands::shared::{command_error_message, sh_single_quoted};

const REMOTE_BINARY_PATH: &str = "/opt/dune-server-service/dune-server-service";
const REMOTE_SYSTEMD_UNIT_PATH: &str = "/etc/systemd/system/dune-server-service.service";
const REMOTE_OPENRC_PATH: &str = "/etc/init.d/dune-server-service";

const BUNDLED_VERSION: &str = env!("DUNE_SERVER_SERVICE_VERSION");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementInstallRequest {
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    /// Optional command-auth token. If None, install only refreshes the binary.
    pub command_auth_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementConnRequest {
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementInstallResult {
    pub installed: bool,
    pub started: bool,
    pub init_system: String,
    pub installed_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementServiceStatus {
    pub installed: bool,
    pub active: bool,
    pub init_system: String,
    pub installed_version: Option<String>,
    pub bundled_version: String,
    pub journal_tail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgressEvent {
    pub step: String,
    pub status: String,
    pub message: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone)]
struct ServiceAccount {
    user: String,
    group: String,
    home: String,
}

fn target_from_conn(req: &ManagementConnRequest) -> Result<RusshTarget, String> {
    let mut target = RusshTarget::new(
        PathBuf::from(
            req.key_path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string(),
        ),
        req.user.trim().to_string(),
        req.host.trim().to_string(),
    );
    if req.port != 0 {
        target.port = req.port;
    }
    target.validate().map_err(|err| err.message)?;
    Ok(target)
}

fn target_from_install(req: &ManagementInstallRequest) -> Result<RusshTarget, String> {
    let conn = ManagementConnRequest {
        host: req.host.clone(),
        user: req.user.clone(),
        key_path: req.key_path.clone(),
        port: req.port,
    };
    target_from_conn(&conn)
}

fn resolve_resource(app: &tauri::AppHandle, path: &str) -> Result<PathBuf, String> {
    let resource = app
        .path()
        .resolve(path, tauri::path::BaseDirectory::Resource)
        .map_err(|err| format!("resolving bundled {path}: {err}"))?;
    if !resource.exists() {
        return Err(format!("bundled {path} missing at {}", resource.display()));
    }
    Ok(resource)
}

#[tauri::command]
pub async fn install_management_service(
    app: tauri::AppHandle,
    request: ManagementInstallRequest,
) -> Result<ManagementInstallResult, String> {
    let binary_path = resolve_resource(&app, "binaries/dune-server-service")?;
    let unit_path = resolve_resource(&app, "binaries/dune-server-service.service")?;
    let openrc_path = resolve_resource(&app, "binaries/dune-server-service.openrc")?;
    let target = target_from_install(&request)?;
    let token = request.command_auth_token.clone();
    let app_handle = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        install_inner(
            &app_handle,
            &target,
            &binary_path,
            &unit_path,
            &openrc_path,
            token.as_deref(),
        )
    })
    .await
    .map_err(|err| format!("install worker failed: {err}"))?
}

#[tauri::command]
pub fn management_service_bundled_version() -> String {
    BUNDLED_VERSION.trim().to_string()
}

#[tauri::command]
pub async fn uninstall_management_service(request: ManagementConnRequest) -> Result<(), String> {
    let target = target_from_conn(&request)?;
    tauri::async_runtime::spawn_blocking(move || uninstall_inner(&target))
        .await
        .map_err(|err| format!("uninstall worker failed: {err}"))?
}

#[tauri::command]
pub async fn restart_management_service(request: ManagementConnRequest) -> Result<(), String> {
    let target = target_from_conn(&request)?;
    tauri::async_runtime::spawn_blocking(move || {
        let script = "set -eu\n\
             export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
             if command -v systemctl >/dev/null 2>&1; then\n  \
                 sudo systemctl restart dune-server-service.service\n\
             elif command -v rc-service >/dev/null 2>&1; then\n  \
                 sudo rc-service dune-server-service restart\n\
             else\n  \
                 echo \"no supported init system\" >&2\n  \
                 exit 1\n\
             fi\n\
             exit 0\n";
        let runner = RusshRunner::new(target.clone());
        runner
            .run_script(script)
            .map_err(command_error_message)
            .map(|_| ())
    })
    .await
    .map_err(|err| format!("restart worker failed: {err}"))?
}

#[tauri::command]
pub async fn management_service_status(
    request: ManagementConnRequest,
) -> Result<ManagementServiceStatus, String> {
    let target = target_from_conn(&request)?;
    tauri::async_runtime::spawn_blocking(move || status_inner(&target))
        .await
        .map_err(|err| format!("status worker failed: {err}"))?
}

fn install_inner(
    app: &tauri::AppHandle,
    target: &RusshTarget,
    binary_path: &std::path::Path,
    unit_path: &std::path::Path,
    openrc_path: &std::path::Path,
    token: Option<&str>,
) -> Result<ManagementInstallResult, String> {
    let runner = RusshRunner::new(target.clone());
    let account = discover_service_account(&runner, &target.user)?;

    emit_progress(app, "stop-old", "running", None);
    let stop_script = "set +e\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         sudo systemctl disable --now server-management-service.service >/dev/null 2>&1 || true\n\
         sudo systemctl stop dune-server-service.service >/dev/null 2>&1 || true\n\
         sudo rc-service dune-server-service stop >/dev/null 2>&1 || true\n\
         exit 0\n";
    runner
        .run_script(stop_script)
        .map_err(|err| step_err(app, "stop-old", err))?;
    emit_progress(app, "stop-old", "ok", None);

    let binary_bytes = std::fs::read(binary_path)
        .map_err(|err| format!("reading resource {}: {err}", binary_path.display()))?;
    let binary_size = std::fs::metadata(binary_path)
        .ok()
        .map(|m| m.len())
        .unwrap_or(0);
    let size_msg = if binary_size > 0 {
        format!("{:.1} MB", binary_size as f64 / 1024.0 / 1024.0)
    } else {
        "unknown size".to_string()
    };
    emit_progress(
        app,
        "upload-binary",
        "running",
        Some(format!(
            "streaming {size_msg} from {} to {REMOTE_BINARY_PATH}",
            binary_path.display()
        )),
    );
    let upload_script = format!(
        "set -eu\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         sudo install -d -m 0755 /opt/dune-server-service\n\
         tmp=$(mktemp /tmp/dune-server-service.XXXXXX)\n\
         trap 'rm -f \"$tmp\"' EXIT\n\
         cat > \"$tmp\"\n\
         actual=$(wc -c < \"$tmp\" | tr -d '[:space:]')\n\
         if [ \"$actual\" != {expected_bytes} ]; then\n  \
             echo \"upload byte-count mismatch: expected {expected_bytes}, got $actual\" >&2\n  \
             exit 42\n\
         fi\n\
         sudo install -m 0755 -o root -g root \"$tmp\" {dest}\n\
         installed=$(sudo stat -c '%s bytes mode=%a owner=%U:%G' {dest})\n\
         echo \"remote install: $installed\"\n",
        expected_bytes = binary_bytes.len(),
        dest = sh_single_quoted(REMOTE_BINARY_PATH),
    );
    let upload_stdout = runner
        .run_with_stdin(
            &format!("sh -c {}", sh_single_quoted(&upload_script)),
            &binary_bytes,
        )
        .map_err(|err| step_err(app, "upload-binary", err))?;
    let upload_msg = if upload_stdout.trim().is_empty() {
        size_msg
    } else {
        format!("{size_msg}; {}", upload_stdout.trim())
    };
    emit_progress(app, "upload-binary", "ok", Some(upload_msg));

    if let Some(t) = token {
        emit_progress(app, "write-token", "running", None);
        let token_b64 = base64::engine::general_purpose::STANDARD.encode(t.as_bytes());
        let token_path = format!("{}/.dune/state/command-auth-token", account.home);
        let token_script = format!(
            "set -eu\n\
             export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
             sudo install -d -m 0700 -o {user} -g {group} {state_dir}\n\
             echo {b64} | base64 -d | sudo install -m 0600 -o {user} -g {group} /dev/stdin {dest}\n",
            user = sh_single_quoted(&account.user),
            group = sh_single_quoted(&account.group),
            state_dir = sh_single_quoted(&format!("{}/.dune/state", account.home)),
            b64 = sh_single_quoted(&token_b64),
            dest = sh_single_quoted(&token_path),
        );
        runner
            .run_script(&token_script)
            .map_err(|err| step_err(app, "write-token", err))?;
        emit_progress(app, "write-token", "ok", None);
    } else {
        emit_progress(
            app,
            "write-token",
            "ok",
            Some("skipped (no token)".to_string()),
        );
    }

    emit_progress(app, "install-init", "running", None);
    let unit_b64 = base64::engine::general_purpose::STANDARD
        .encode(render_systemd_unit(unit_path, &account)?.as_bytes());
    let openrc_b64 = base64::engine::general_purpose::STANDARD
        .encode(render_openrc_unit(openrc_path, &account)?.as_bytes());
    let init_script = format!(
        "set -eu\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         if command -v systemctl >/dev/null 2>&1; then\n  \
             echo SYSTEMD\n  \
             echo {unit_b64} | base64 -d | sudo install -m 0644 -o root -g root /dev/stdin {unit_dest}\n  \
             sudo install -d -m 0755 /etc/systemd/system/dune-server-service.service.d\n  \
             printf '%s\\n' '[Service]' 'NoNewPrivileges=false' 'MemoryDenyWriteExecute=false' | sudo install -m 0644 -o root -g root /dev/stdin /etc/systemd/system/dune-server-service.service.d/zz-dune-steamcmd-compat.conf\n\
             sudo systemctl daemon-reload\n\
             sudo systemctl reset-failed dune-server-service.service >/dev/null 2>&1 || true\n\
         elif command -v rc-service >/dev/null 2>&1; then\n  \
             echo OPENRC\n  \
             echo {openrc_b64} | base64 -d | sudo install -m 0755 -o root -g root /dev/stdin {openrc_dest}\n  \
             sudo rc-update add dune-server-service default >/dev/null 2>&1 || true\n\
         else\n  \
             echo \"no supported init system found (need systemd or openrc)\" >&2\n  \
             exit 1\n\
         fi\n",
        unit_b64 = sh_single_quoted(&unit_b64),
        unit_dest = sh_single_quoted(REMOTE_SYSTEMD_UNIT_PATH),
        openrc_b64 = sh_single_quoted(&openrc_b64),
        openrc_dest = sh_single_quoted(REMOTE_OPENRC_PATH),
    );
    let init_stdout = runner
        .run_script(&init_script)
        .map_err(|err| step_err(app, "install-init", err))?;
    let mut init_system = String::from("unknown");
    for line in init_stdout.lines() {
        match line.trim() {
            "SYSTEMD" => init_system = "systemd".to_string(),
            "OPENRC" => init_system = "openrc".to_string(),
            _ => {}
        }
    }
    emit_progress(app, "install-init", "ok", Some(init_system.clone()));

    emit_progress(app, "start-service", "running", None);
    let start_script = "set -eu\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         if command -v systemctl >/dev/null 2>&1; then\n  \
             sudo systemctl enable --now dune-server-service.service\n\
         elif command -v rc-service >/dev/null 2>&1; then\n  \
             sudo rc-service dune-server-service restart >/dev/null 2>&1 || sudo rc-service dune-server-service start\n\
         fi\n";
    runner
        .run_script(start_script)
        .map_err(|err| step_err(app, "start-service", err))?;
    emit_progress(app, "start-service", "ok", None);

    emit_progress(app, "verify", "running", None);
    let verify_script = "set +e\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         if command -v systemctl >/dev/null 2>&1; then\n  \
             sleep 1\n  \
             sudo systemctl is-active dune-server-service.service\n\
         elif command -v rc-service >/dev/null 2>&1; then\n  \
             sleep 1\n  \
             sudo rc-service dune-server-service status >/dev/null 2>&1 && echo active || echo inactive\n\
         else\n  \
             echo inactive\n\
         fi\n\
         /opt/dune-server-service/dune-server-service --version 2>/dev/null || true\n\
         exit 0\n";
    let verify_stdout = runner
        .run_script(verify_script)
        .map_err(|err| step_err(app, "verify", err))?;
    let mut active_state = String::new();
    let mut installed_version: Option<String> = None;
    for line in verify_stdout.lines() {
        let trimmed = line.trim();
        match trimmed {
            "active" | "inactive" => active_state = trimmed.to_string(),
            other if other.starts_with("dune-server-service ") => {
                installed_version = other
                    .strip_prefix("dune-server-service ")
                    .map(|s| s.trim().to_string());
            }
            _ => {}
        }
    }
    let started = active_state == "active";
    let verify_msg = match (started, &installed_version) {
        (true, Some(v)) => Some(format!("active, version {v}")),
        (true, None) => Some("active".to_string()),
        (false, _) => Some(format!("not active ({active_state})")),
    };
    emit_progress(
        app,
        "verify",
        if started { "ok" } else { "error" },
        verify_msg.clone(),
    );

    Ok(ManagementInstallResult {
        installed: true,
        started,
        init_system: init_system.clone(),
        installed_version,
        message: format!("installed via {init_system}; active={active_state}"),
    })
}

fn discover_service_account(
    runner: &RusshRunner,
    registered_user: &str,
) -> Result<ServiceAccount, String> {
    let user = registered_user.trim();
    if user.is_empty() {
        return Err("registered SSH user is required".to_string());
    }
    let script = format!(
        "set -eu\n\
         user={user}\n\
         home=$(getent passwd \"$user\" | awk -F: '{{print $6}}')\n\
         group=$(id -gn \"$user\")\n\
         if [ -z \"$home\" ] || [ -z \"$group\" ]; then\n  \
             echo \"could not resolve service account for $user\" >&2\n  \
             exit 1\n\
         fi\n\
         printf 'USER=%s\\nGROUP=%s\\nHOME=%s\\n' \"$user\" \"$group\" \"$home\"\n",
        user = sh_single_quoted(user),
    );
    let stdout = runner.run_script(&script).map_err(command_error_message)?;
    let mut account = ServiceAccount {
        user: String::new(),
        group: String::new(),
        home: String::new(),
    };
    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("USER=") {
            account.user = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("GROUP=") {
            account.group = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("HOME=") {
            account.home = value.trim().trim_end_matches('/').to_string();
        }
    }
    if account.user.is_empty() || account.group.is_empty() || account.home.is_empty() {
        return Err(format!(
            "could not resolve service account from remote output: {stdout}"
        ));
    }
    Ok(account)
}

fn render_systemd_unit(path: &std::path::Path, account: &ServiceAccount) -> Result<String, String> {
    let unit = std::fs::read_to_string(path)
        .map_err(|err| format!("reading resource {}: {err}", path.display()))?;
    let home = account.home.as_str();
    Ok(unit
        .replace("User=dune", &format!("User={}", account.user))
        .replace("Group=dune", &format!("Group={}", account.group))
        .replace("/home/dune/.local/bin", &format!("{home}/.local/bin"))
        .replace("/home/dune/.dune", &format!("{home}/.dune"))
        .replace("/home/dune/.steam", &format!("{home}/.steam"))
        .replace("/home/dune/Steam", &format!("{home}/Steam"))
        .replace(
            "Environment=\"DUNE_SERVICE_HOME=/home/dune\"",
            &format!("Environment=\"DUNE_SERVICE_HOME={home}\""),
        ))
}

fn render_openrc_unit(path: &std::path::Path, account: &ServiceAccount) -> Result<String, String> {
    let unit = std::fs::read_to_string(path)
        .map_err(|err| format!("reading resource {}: {err}", path.display()))?;
    let home = account.home.as_str();
    Ok(unit
        .replace(
            "command_user=\"dune:dune\"",
            &format!("command_user=\"{}:{}\"", account.user, account.group),
        )
        .replace(
            "--owner dune:dune",
            &format!("--owner {}:{}", account.user, account.group),
        )
        .replace("/home/dune/.dune", &format!("{home}/.dune"))
        .replace(
            "DUNE_SERVICE_HOME=\"${DUNE_SERVICE_HOME:-/home/dune}\"",
            &format!("DUNE_SERVICE_HOME=\"${{DUNE_SERVICE_HOME:-{home}}}\""),
        ))
}

fn emit_progress(app: &tauri::AppHandle, step: &str, status: &str, message: Option<String>) {
    let payload = InstallProgressEvent {
        step: step.to_string(),
        status: status.to_string(),
        message,
    };
    let _ = app.emit("management-install-progress", payload);
}

fn step_err(
    app: &tauri::AppHandle,
    step: &str,
    err: dune_manager_core::models::CommandFailure,
) -> String {
    let msg = command_error_message(err);
    emit_progress(app, step, "error", Some(msg.clone()));
    msg
}

fn uninstall_inner(target: &RusshTarget) -> Result<(), String> {
    let script = "set -eu\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         if command -v systemctl >/dev/null 2>&1; then\n  \
             sudo systemctl disable --now dune-server-service.service >/dev/null 2>&1 || true\n  \
             sudo rm -f /etc/systemd/system/dune-server-service.service\n  \
             sudo systemctl daemon-reload\n\
         fi\n\
         if command -v rc-service >/dev/null 2>&1; then\n  \
             sudo rc-service dune-server-service stop >/dev/null 2>&1 || true\n  \
             sudo rc-update del dune-server-service default >/dev/null 2>&1 || true\n  \
             sudo rm -f /etc/init.d/dune-server-service\n\
         fi\n\
         sudo rm -rf /opt/dune-server-service\n\
         exit 0\n";
    let runner = RusshRunner::new(target.clone());
    runner
        .run_script(script)
        .map_err(command_error_message)
        .map(|_| ())
}

fn status_inner(target: &RusshTarget) -> Result<ManagementServiceStatus, String> {
    let script = "set +e\n\
         export PATH=/sbin:/usr/sbin:/usr/local/sbin:$PATH\n\
         if [ -x /opt/dune-server-service/dune-server-service ]; then\n  \
             echo INSTALLED=yes\n  \
             /opt/dune-server-service/dune-server-service --version 2>/dev/null | head -n 1\n\
         else\n  \
             echo INSTALLED=no\n\
         fi\n\
         if command -v systemctl >/dev/null 2>&1; then\n  \
             echo INIT=systemd\n  \
             sudo systemctl is-active dune-server-service.service\n\
         elif command -v rc-service >/dev/null 2>&1; then\n  \
             echo INIT=openrc\n  \
             sudo rc-service dune-server-service status >/dev/null 2>&1 && echo active || echo inactive\n\
         else\n  \
             echo INIT=none\n\
         fi\n\
         exit 0\n";
    let runner = RusshRunner::new(target.clone());
    let stdout = runner.run_script(script).map_err(command_error_message)?;
    let mut installed = false;
    let mut active = false;
    let mut init_system = String::from("unknown");
    let mut installed_version: Option<String> = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        match trimmed {
            "INSTALLED=yes" => installed = true,
            "INSTALLED=no" => installed = false,
            "INIT=systemd" => init_system = "systemd".to_string(),
            "INIT=openrc" => init_system = "openrc".to_string(),
            "INIT=none" => init_system = "none".to_string(),
            "active" => active = true,
            "inactive" => active = false,
            other if other.starts_with("dune-server-service ") => {
                installed_version = other
                    .strip_prefix("dune-server-service ")
                    .map(|s| s.trim().to_string());
            }
            _ => {}
        }
    }
    Ok(ManagementServiceStatus {
        installed,
        active,
        init_system,
        installed_version,
        bundled_version: BUNDLED_VERSION.trim().to_string(),
        journal_tail: String::new(),
    })
}
