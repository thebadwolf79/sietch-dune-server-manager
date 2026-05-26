//! Pre-attach connectivity + sudo checks executed against a candidate host.

use std::path::PathBuf;

use dune_manager_core::orchestration::{RemoteCommandRunner, RusshRunner, RusshTarget};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightCheck {
    /// SSH connection + key authentication succeeded.
    pub ssh_ok: bool,
    /// The SSH user can `sudo -n -u dune` without a password.
    pub sudo_to_dune_ok: bool,
    /// The `dune` user itself has passwordless sudo for arbitrary commands.
    pub dune_nopasswd_ok: bool,
    /// Whether the SSH login user IS `dune` (no impersonation needed).
    pub is_dune_login: bool,
    /// Raw stdout/stderr collected from the probe script — surfaced in the
    /// UI when something fails so the operator can see exactly what
    /// happened on the host.
    pub raw_output: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightRequest {
    pub host: String,
    pub user: String,
    pub key_path: String,
    #[serde(default)]
    pub port: Option<u16>,
}

/// Probes connectivity, SSH auth, and the various sudo capabilities we
/// rely on. The result is used to gate the attach flow with a clear error
/// banner when something is missing.
#[tauri::command]
pub async fn check_remote_sudo(request: PreflightRequest) -> Result<PreflightCheck, String> {
    let host = request.host.trim().to_string();
    let user = request.user.trim().to_string();
    let key_path = request.key_path.trim().to_string();
    let port = request.port;
    if host.is_empty() || user.is_empty() || key_path.is_empty() {
        return Err("Host, user, and SSH key path are required.".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || run_preflight(host, user, key_path, port))
        .await
        .map_err(|err| format!("Preflight worker failed: {err}"))?
}

fn run_preflight(
    host: String,
    user: String,
    key_path: String,
    port: Option<u16>,
) -> Result<PreflightCheck, String> {
    let mut target = RusshTarget::new(PathBuf::from(&key_path), user.clone(), host.clone());
    if let Some(p) = port {
        target.port = p;
    }
    target.validate().map_err(|err| err.message)?;
    let runner = RusshRunner::new(target);
    let probe = r#"set +e
echo SSH_OK
if sudo -n -u dune true >/dev/null 2>&1; then echo SUDO_TO_DUNE_OK; else echo SUDO_TO_DUNE_FAILED; fi
if sudo -n -u dune sudo -n true >/dev/null 2>&1; then echo DUNE_NOPASSWD_OK; else echo DUNE_NOPASSWD_FAILED; fi
echo PREFLIGHT_DONE
"#;
    let stdout = runner.run_script(probe).map_err(|err| {
        // Connection / auth failures land here. Surface them to the UI so
        // the operator can fix host/key before retrying.
        if !err.stderr.trim().is_empty() {
            format!("{}: {}", err.message, err.stderr.trim())
        } else {
            err.message
        }
    })?;
    let ssh_ok = stdout.contains("SSH_OK");
    let is_dune_login = user == "dune";
    // When the SSH login is already dune, we do not need a sudo-to-dune
    // hop; treat it as ok regardless of the probe outcome.
    let sudo_to_dune_ok = is_dune_login || stdout.contains("SUDO_TO_DUNE_OK");
    let dune_nopasswd_ok = if is_dune_login {
        // `sudo -n -u dune sudo -n true` may be rejected when the outer
        // sudo refuses self-targeting. Fall back to a direct `sudo -n true`
        // check when the operator is already logged in as dune. Re-run a
        // quick second probe.
        let direct = r#"if sudo -n true >/dev/null 2>&1; then echo DUNE_NOPASSWD_OK; else echo DUNE_NOPASSWD_FAILED; fi"#;
        runner
            .run_script(direct)
            .map(|out| out.contains("DUNE_NOPASSWD_OK"))
            .unwrap_or(false)
    } else {
        stdout.contains("DUNE_NOPASSWD_OK")
    };
    Ok(PreflightCheck {
        ssh_ok,
        sudo_to_dune_ok,
        dune_nopasswd_ok,
        is_dune_login,
        raw_output: stdout,
    })
}
