//! Public surface for vendor SSH key preparation.

use std::path::{Path, PathBuf};

use crate::{
    errors::failure,
    models::CommandResult,
    toolchain::{
        package_detection::detect_server_package_layout,
        ssh_key_paths::{prepare_restricted_ssh_key_copy, prepare_vendor_ssh_key_candidates_inner},
    },
};

/// Copies the packaged bootstrap SSH key to a temporary path and restricts its ACL for OpenSSH.
pub fn prepare_vendor_ssh_key(server_package_dir: impl AsRef<Path>) -> CommandResult<PathBuf> {
    let layout = detect_server_package_layout(server_package_dir)?;
    let ssh_key = layout.ssh_key.ok_or_else(|| {
        failure("The release server package does not include a bootstrap SSH key")
    })?;
    prepare_restricted_ssh_key_copy(&ssh_key)
}

/// Copies usable vendor SSH key candidates to temporary paths with OpenSSH-compatible ACLs.
///
/// Current `battlegroup-management` packages rotate the public bootstrap key into
/// `%LOCALAPPDATA%\DuneAwakeningServer\sshKey` during vendor setup. Existing VMs
/// may therefore authenticate with that active key, while fresh imported VMs
/// still authenticate with the packaged bootstrap key.
pub fn prepare_vendor_ssh_key_candidates(
    server_package_dir: impl AsRef<Path>,
) -> CommandResult<Vec<PathBuf>> {
    prepare_vendor_ssh_key_candidates_inner(server_package_dir.as_ref(), None)
}

/// Copies usable vendor SSH key candidates for a specific Hyper-V VM.
///
/// The manager stores generated keys per VM so one local server cannot overwrite
/// the active key needed by another local server.
pub fn prepare_vendor_ssh_key_candidates_for_vm(
    server_package_dir: impl AsRef<Path>,
    vm_name: &str,
) -> CommandResult<Vec<PathBuf>> {
    prepare_vendor_ssh_key_candidates_inner(server_package_dir.as_ref(), Some(vm_name))
}
