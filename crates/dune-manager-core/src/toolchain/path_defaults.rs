//! Default filesystem locations for the app-owned toolchain and packages.

use std::{
    env,
    path::{Path, PathBuf},
};

use crate::{errors::failure, models::CommandResult};

/// Resolves the default manager data root for owned tools and downloads.
pub fn default_tools_root() -> CommandResult<PathBuf> {
    if let Ok(value) = env::var("DUNE_MANAGER_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(value) = env::var("LOCALAPPDATA") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("DuneDedicatedServerManager"));
        }
    }
    Ok(env::current_dir()
        .map_err(|err| failure(format!("Failed to determine current directory: {err}")))?
        .join(".dune-manager"))
}

/// Resolves the default directory for Hyper-V VM files managed by the app.
pub fn default_vm_destination() -> CommandResult<PathBuf> {
    Ok(default_runtime_root()?.join("vm"))
}

/// Resolves the default host-side server package directory.
pub fn default_server_package_dir() -> CommandResult<PathBuf> {
    Ok(default_runtime_root()?.join("dune-server"))
}

fn default_runtime_root() -> CommandResult<PathBuf> {
    let current = env::current_dir()
        .map_err(|err| failure(format!("Failed to determine current directory: {err}")))?;
    if current
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("src-tauri"))
    {
        return current
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| failure("Failed to resolve app runtime root"));
    }
    Ok(current)
}
