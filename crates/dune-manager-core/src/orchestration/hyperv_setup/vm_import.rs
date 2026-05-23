use std::path::Path;

use crate::{errors::failure, models::CommandResult, orchestration::packaged_vmcx_candidates};

pub(super) fn single_vmcx(install_path: &Path) -> CommandResult<String> {
    let candidates = packaged_vmcx_candidates(install_path)?;
    match candidates.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(failure(format!(
            "No .vmcx file found under {}",
            install_path.join("Virtual Machines").display()
        ))),
        _ => Err(failure(format!(
            "Multiple .vmcx files found under {}",
            install_path.join("Virtual Machines").display()
        ))),
    }
}

pub(super) fn clear_destination_dir(path: &Path) -> CommandResult<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.parent().is_none() {
        return Err(failure(
            "Refusing to clear destination without a parent directory",
        ));
    }
    std::fs::remove_dir_all(path)
        .map_err(|err| failure(format!("Failed to clear {}: {err}", path.display())))
}

pub(super) fn destination_has_vm_artifacts(path: &Path) -> bool {
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
            entry.path().extension().is_some_and(|extension| {
                ["vmcx", "vmrs", "vhd", "vhdx"]
                    .iter()
                    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
            })
        })
}
