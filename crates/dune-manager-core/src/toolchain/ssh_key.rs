//! Public surface for vendor SSH key preparation and rotation.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    errors::failure,
    models::CommandResult,
    toolchain::{
        package_detection::detect_server_package_layout,
        ssh_key_paths::{prepare_restricted_ssh_key_copy, prepare_vendor_ssh_key_candidates_inner},
        ssh_key_rotation::rotate_vendor_guest_ssh_key_inner,
    },
};

/// Result of rotating a fresh imported VM from the public bootstrap key to a host-local key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorSshKeyRotationResult {
    /// Private key path to use for the rest of the current setup flow.
    pub key_path: PathBuf,
    /// Public key path when a new key was generated and stored.
    pub public_key_path: Option<PathBuf>,
    /// Whether a new public key was installed into the guest.
    pub rotated: bool,
    /// Human-readable status or fallback reason.
    pub message: String,
}

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

/// Generates a fresh host-local SSH key and installs its public key into the guest.
///
/// Current vendor setup seeds first contact with the packaged `bundledSshKey`, then rotates
/// the VM to `%LOCALAPPDATA%\DuneAwakeningServer\sshKey`. This mirrors that behavior while
/// returning the currently usable key so native setup can continue even if rotation falls back.
pub fn rotate_vendor_guest_ssh_key(
    server_package_dir: impl AsRef<Path>,
    ssh_path: impl AsRef<Path>,
    bootstrap_key_path: impl AsRef<Path>,
    host: &str,
) -> CommandResult<VendorSshKeyRotationResult> {
    rotate_vendor_guest_ssh_key_inner(server_package_dir, ssh_path, bootstrap_key_path, host, None)
}

/// Generates a fresh host-local SSH key for one Hyper-V VM and installs it into the guest.
pub fn rotate_vendor_guest_ssh_key_for_vm(
    server_package_dir: impl AsRef<Path>,
    ssh_path: impl AsRef<Path>,
    bootstrap_key_path: impl AsRef<Path>,
    host: &str,
    vm_name: &str,
) -> CommandResult<VendorSshKeyRotationResult> {
    rotate_vendor_guest_ssh_key_inner(
        server_package_dir,
        ssh_path,
        bootstrap_key_path,
        host,
        Some(vm_name),
    )
}
