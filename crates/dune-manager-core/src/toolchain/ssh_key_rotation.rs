//! Guest-side SSH key rotation flow for fresh Hyper-V VMs.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    errors::{command_failure, failure},
    models::CommandResult,
    shell::{ps_single_quoted, run_powershell},
    toolchain::{
        package_detection::{detect_server_package_layout, ServerPackageLayout},
        ssh_key::VendorSshKeyRotationResult,
        ssh_key_paths::{manager_vm_ssh_key_path, vendor_active_ssh_key_path},
    },
};

pub(super) fn rotate_vendor_guest_ssh_key_inner(
    server_package_dir: impl AsRef<Path>,
    ssh_path: impl AsRef<Path>,
    bootstrap_key_path: impl AsRef<Path>,
    host: &str,
    vm_name: Option<&str>,
) -> CommandResult<VendorSshKeyRotationResult> {
    let layout = detect_server_package_layout(server_package_dir)?;
    let bootstrap_key_path = bootstrap_key_path.as_ref();
    if layout.layout != ServerPackageLayout::BattlegroupManagement {
        return Ok(VendorSshKeyRotationResult {
            key_path: bootstrap_key_path.to_path_buf(),
            public_key_path: None,
            rotated: false,
            message: "Legacy server package layout keeps using the packaged SSH key.".to_string(),
        });
    }

    let ssh_path = ssh_path.as_ref();
    let keygen = ssh_path
        .parent()
        .map(|dir| dir.join("ssh-keygen.exe"))
        .ok_or_else(|| failure("Failed to resolve OpenSSH tool directory"))?;
    if !keygen.is_file() {
        return Ok(VendorSshKeyRotationResult {
            key_path: bootstrap_key_path.to_path_buf(),
            public_key_path: None,
            rotated: false,
            message: format!(
                "OpenSSH key generator was not found at {}; continuing with the bootstrap key.",
                keygen.display()
            ),
        });
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_stem = env::temp_dir().join(format!(
        "dune-manager-vm-generated-sshKey-{}-{unique}",
        std::process::id()
    ));
    let temp_public = PathBuf::from(format!("{}.pub", temp_stem.to_string_lossy()));
    let active_key = vendor_active_ssh_key_path()
        .ok_or_else(|| failure("LOCALAPPDATA is required to store the active VM SSH key"))?;
    let active_public = PathBuf::from(format!("{}.pub", active_key.to_string_lossy()));

    let generate_output = Command::new(&keygen)
        .args(["-t", "ed25519", "-f"])
        .arg(&temp_stem)
        .args(["-N", "", "-q", "-C", "dune-manager-hyperv"])
        .output()
        .map_err(|err| failure(format!("Failed to run ssh-keygen: {err}")))?;
    if !generate_output.status.success() || !temp_stem.is_file() || !temp_public.is_file() {
        let _ = fs::remove_file(&temp_stem);
        let _ = fs::remove_file(&temp_public);
        return Ok(VendorSshKeyRotationResult {
            key_path: bootstrap_key_path.to_path_buf(),
            public_key_path: None,
            rotated: false,
            message: command_failure(
                "ssh-keygen failed; continuing with the bootstrap key",
                generate_output,
            )
            .message,
        });
    }

    let public_key = fs::read_to_string(&temp_public)
        .map_err(|err| failure(format!("Failed to read generated public key: {err}")))?;
    if let Err(err) = install_guest_public_key(ssh_path, bootstrap_key_path, host, &public_key) {
        let _ = fs::remove_file(&temp_stem);
        let _ = fs::remove_file(&temp_public);
        return Ok(VendorSshKeyRotationResult {
            key_path: bootstrap_key_path.to_path_buf(),
            public_key_path: None,
            rotated: false,
            message: format!(
                "Failed to install the generated SSH key; continuing with the bootstrap key. {}",
                err.message
            ),
        });
    }

    if let Err(err) = verify_guest_key(ssh_path, &temp_stem, host) {
        let _ = fs::remove_file(&temp_stem);
        let _ = fs::remove_file(&temp_public);
        return Ok(VendorSshKeyRotationResult {
            key_path: bootstrap_key_path.to_path_buf(),
            public_key_path: None,
            rotated: false,
            message: format!(
                "The generated SSH key was installed but did not authenticate; continuing with the bootstrap key. {}",
                err.message
            ),
        });
    }

    store_active_vendor_ssh_key(&temp_stem, &temp_public, &active_key, &active_public)?;
    let (key_path, public_key_path) =
        if let Some(vm_private) = vm_name.and_then(manager_vm_ssh_key_path) {
            let vm_public = PathBuf::from(format!("{}.pub", vm_private.to_string_lossy()));
            copy_vendor_ssh_key_pair(&active_key, &active_public, &vm_private, &vm_public)?;
            (vm_private, vm_public)
        } else {
            (active_key, active_public)
        };
    Ok(VendorSshKeyRotationResult {
        key_path,
        public_key_path: Some(public_key_path),
        rotated: true,
        message: "Generated and installed a fresh VM SSH key.".to_string(),
    })
}

fn install_guest_public_key(
    ssh_path: &Path,
    bootstrap_key_path: &Path,
    host: &str,
    public_key: &str,
) -> CommandResult<()> {
    let public_key_b64 = base64_encode(format!("{}\n", public_key.trim()).as_bytes());
    let remote_script = format!(
        r#"
set -eu
mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
printf '%s' '{public_key_b64}' | base64 -d > "$HOME/.ssh/authorized_keys.new"
chmod 600 "$HOME/.ssh/authorized_keys.new"
mv "$HOME/.ssh/authorized_keys.new" "$HOME/.ssh/authorized_keys"
echo ROTATE_OK
"#
    );
    let remote_command = format!(
        "printf '%s' '{}' | base64 -d | sh",
        base64_encode(remote_script.as_bytes())
    );
    let output = Command::new(ssh_path)
        .args(openssh_key_rotation_args(bootstrap_key_path, host))
        .arg(remote_command)
        .output()
        .map_err(|err| failure(format!("Failed to run ssh for key rotation: {err}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !stdout.contains("ROTATE_OK") {
        return Err(command_failure(
            "Failed to install generated SSH public key in the guest",
            output,
        ));
    }
    Ok(())
}

fn verify_guest_key(ssh_path: &Path, key_path: &Path, host: &str) -> CommandResult<()> {
    let output = Command::new(ssh_path)
        .args(openssh_key_rotation_args(key_path, host))
        .arg("true")
        .output()
        .map_err(|err| failure(format!("Failed to verify generated SSH key: {err}")))?;
    if !output.status.success() {
        return Err(command_failure(
            "Generated SSH key did not authenticate to the guest",
            output,
        ));
    }
    Ok(())
}

fn openssh_key_rotation_args(key_path: &Path, host: &str) -> Vec<String> {
    vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        "IdentitiesOnly=yes".to_string(),
        "-o".to_string(),
        "PreferredAuthentications=publickey".to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(),
        "UserKnownHostsFile=NUL".to_string(),
        "-o".to_string(),
        "LogLevel=ERROR".to_string(),
        "-o".to_string(),
        "ConnectTimeout=8".to_string(),
        "-i".to_string(),
        key_path.to_string_lossy().to_string(),
        format!("dune@{host}"),
    ]
}

fn store_active_vendor_ssh_key(
    private_source: &Path,
    public_source: &Path,
    private_destination: &Path,
    public_destination: &Path,
) -> CommandResult<()> {
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$privateSource = {private_source}
$publicSource = {public_source}
$privateDestination = {private_destination}
$publicDestination = {public_destination}
$keyDir = Split-Path -Parent $privateDestination
New-Item -ItemType Directory -Force -Path $keyDir | Out-Null
foreach ($path in @($privateDestination, $publicDestination)) {{
  if (Test-Path -LiteralPath $path) {{
    takeown /f $path 2>&1 | Out-Null
    icacls $path /reset 2>&1 | Out-Null
    Remove-Item -LiteralPath $path -Force
  }}
}}
Move-Item -LiteralPath $privateSource -Destination $privateDestination -Force
Move-Item -LiteralPath $publicSource -Destination $publicDestination -Force
icacls $privateDestination /inheritance:r | Out-Null
icacls $privateDestination /grant:r "$($env:USERNAME):(R)" | Out-Null
"#,
        private_source = ps_single_quoted(&private_source.to_string_lossy()),
        public_source = ps_single_quoted(&public_source.to_string_lossy()),
        private_destination = ps_single_quoted(&private_destination.to_string_lossy()),
        public_destination = ps_single_quoted(&public_destination.to_string_lossy()),
    );
    run_powershell(&script).map(|_| ())
}

fn copy_vendor_ssh_key_pair(
    private_source: &Path,
    public_source: &Path,
    private_destination: &Path,
    public_destination: &Path,
) -> CommandResult<()> {
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$privateSource = {private_source}
$publicSource = {public_source}
$privateDestination = {private_destination}
$publicDestination = {public_destination}
$keyDir = Split-Path -Parent $privateDestination
New-Item -ItemType Directory -Force -Path $keyDir | Out-Null
Copy-Item -LiteralPath $privateSource -Destination $privateDestination -Force
Copy-Item -LiteralPath $publicSource -Destination $publicDestination -Force
icacls $privateDestination /inheritance:r | Out-Null
icacls $privateDestination /grant:r "$($env:USERNAME):(R)" | Out-Null
"#,
        private_source = ps_single_quoted(&private_source.to_string_lossy()),
        public_source = ps_single_quoted(&public_source.to_string_lossy()),
        private_destination = ps_single_quoted(&private_destination.to_string_lossy()),
        public_destination = ps_single_quoted(&public_destination.to_string_lossy()),
    );
    run_powershell(&script).map(|_| ())
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}
