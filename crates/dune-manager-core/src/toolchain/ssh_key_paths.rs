//! Path resolution and ACL-restricted copies for vendor SSH key material.

use std::{
    env,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    errors::failure,
    models::CommandResult,
    shell::{ps_single_quoted, run_powershell},
    toolchain::package_detection::{detect_server_package_layout, ServerPackageLayout},
};

pub(super) fn prepare_vendor_ssh_key_candidates_inner(
    server_package_dir: &Path,
    vm_name: Option<&str>,
) -> CommandResult<Vec<PathBuf>> {
    let layout = detect_server_package_layout(server_package_dir)?;
    let mut sources = Vec::new();
    if let Some(vm_key) = vm_name
        .and_then(manager_vm_ssh_key_path)
        .filter(|path| path.is_file())
    {
        sources.push(vm_key);
    }
    if layout.layout == ServerPackageLayout::BattlegroupManagement {
        if let Some(active_key) = vendor_active_ssh_key_path().filter(|path| path.is_file()) {
            sources.push(active_key);
        }
    }
    if let Some(ssh_key) = layout.ssh_key {
        sources.push(ssh_key);
    }
    sources.dedup();

    let mut candidates = Vec::with_capacity(sources.len());
    for source in sources {
        candidates.push(prepare_restricted_ssh_key_copy(&source)?);
    }
    Ok(candidates)
}

pub(super) fn vendor_active_ssh_key_path() -> Option<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local_app_data)
            .join("DuneAwakeningServer")
            .join("sshKey"),
    )
}

pub(super) fn manager_vm_ssh_key_path(vm_name: &str) -> Option<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local_app_data)
            .join("DuneDedicatedServerManager")
            .join("vm-keys")
            .join(sanitize_key_path_segment(vm_name))
            .join("sshKey"),
    )
}

fn sanitize_key_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "vm".to_string()
    } else {
        sanitized
    }
}

pub(super) fn prepare_restricted_ssh_key_copy(source: &Path) -> CommandResult<PathBuf> {
    if !source.is_file() {
        return Err(failure(format!(
            "Vendor SSH key was not found: {}",
            source.display()
        )));
    }
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let destination = env::temp_dir().join(format!(
        "dune-manager-vm-sshKey-{}-{unique}",
        std::process::id()
    ));
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$source = {source}
$destination = {destination}
Copy-Item -LiteralPath $source -Destination $destination -Force
icacls $destination /inheritance:r | Out-Null
icacls $destination /grant:r "$($env:USERNAME):(R)" | Out-Null
[Console]::Out.WriteLine($destination)
"#,
        source = ps_single_quoted(&source.to_string_lossy()),
        destination = ps_single_quoted(&destination.to_string_lossy()),
    );
    run_powershell(&script)?;
    Ok(destination)
}
