use std::process::Command;

use anyhow::{anyhow, Context, Result};

const SERVICE_NAME: &str = "dune-server-service.service";
const OVERRIDE_PATH: &str =
    "/etc/systemd/system/dune-server-service.service.d/zz-dune-steamcmd-compat.conf";

pub fn steamcmd_relocation_blocked(text: &str) -> bool {
    text.contains("steamclient.so")
        && text.contains("cannot make segment writable for relocation")
        && text.contains("Permission denied")
}

pub fn repair_on_startup_if_needed() -> Result<bool> {
    repair_if_needed(false, "startup")
}

pub fn repair_after_steamcmd_relocation_failure() -> Result<bool> {
    repair_if_needed(true, "steamcmd-relocation-failure")
}

fn repair_if_needed(force: bool, reason: &str) -> Result<bool> {
    if !cfg!(target_os = "linux") {
        return Ok(false);
    }

    let script = format!(
        r#"set -eu
if ! command -v systemctl >/dev/null 2>&1 || [ ! -d /run/systemd/system ]; then
  echo "not-systemd"
  exit 0
fi
state="$(systemctl show {service} -p MemoryDenyWriteExecute --value 2>/dev/null || true)"
if [ "{force}" != "1" ]; then
  case "$state" in
    yes|true|1) ;;
    *) echo "compatible:$state"; exit 0 ;;
  esac
fi
sudo install -d -m 0755 /etc/systemd/system/{service}.d
printf '%s\n' '[Service]' 'NoNewPrivileges=false' 'MemoryDenyWriteExecute=false' \
  | sudo install -m 0644 -o root -g root /dev/stdin {override_path}
sudo systemctl daemon-reload
sudo systemctl reset-failed {service} >/dev/null 2>&1 || true
sudo systemctl --no-block restart {service}
echo "repaired:$state"
"#,
        service = SERVICE_NAME,
        force = if force { "1" } else { "0" },
        override_path = OVERRIDE_PATH,
    );

    let output = Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .with_context(|| format!("checking systemd steamcmd compatibility for {reason}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(anyhow!(
            "failed to repair systemd steamcmd compatibility for {reason}: stdout={} stderr={}",
            stdout.trim(),
            stderr.trim()
        ));
    }
    let repaired = stdout.lines().any(|line| line.starts_with("repaired:"));
    if repaired {
        tracing::warn!(
            reason,
            override_path = OVERRIDE_PATH,
            "installed systemd steamcmd compatibility override and requested service restart"
        );
    } else {
        tracing::debug!(
            reason,
            detail = stdout.trim(),
            "systemd steamcmd compatibility check passed"
        );
    }
    Ok(repaired)
}
