//! Detection of host-side Dune server package layout on disk.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{errors::failure, models::CommandResult};

/// Vendor package layout detected on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerPackageLayout {
    /// Original package layout using `internal-scripts`.
    LegacyInternalScripts,
    /// Current package layout using `battlegroup-management`.
    BattlegroupManagement,
}

/// Required paths discovered for a host-side Dune server package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackageLayoutInfo {
    /// Package root directory.
    pub package_dir: PathBuf,
    /// Detected vendor layout.
    pub layout: ServerPackageLayout,
    /// Host-side batch entrypoint.
    pub battlegroup_bat: PathBuf,
    /// Vendor SSH private key used for first guest contact, when the package ships one.
    pub ssh_key: Option<PathBuf>,
    /// Host-side bootstrap helper uploaded into the guest.
    pub bootstrap_setup: PathBuf,
    /// Packaged Hyper-V VM configuration.
    pub vmcx_path: PathBuf,
}

/// Detects a complete server package layout and returns its required paths.
pub fn detect_server_package_layout(
    install_dir: impl AsRef<Path>,
) -> CommandResult<ServerPackageLayoutInfo> {
    let install_dir = install_dir.as_ref();
    let battlegroup_bat = install_dir.join("battlegroup.bat");
    if !battlegroup_bat.is_file() {
        return Err(failure(format!(
            "Vendor battlegroup entrypoint was not found: {}",
            battlegroup_bat.display()
        )));
    }
    let vmcx_path = find_packaged_vmcx(install_dir).ok_or_else(|| {
        failure(format!(
            "Packaged VM configuration was not found under {}",
            install_dir.join("Virtual Machines").display()
        ))
    })?;
    let battlegroup_management_setup = install_dir
        .join("battlegroup-management")
        .join("bootstrap")
        .join("setup");
    if battlegroup_management_setup.is_file() {
        let ssh_key = install_dir
            .join("battlegroup-management")
            .join("ssh")
            .join("bundledSshKey");
        return Ok(ServerPackageLayoutInfo {
            package_dir: install_dir.to_path_buf(),
            layout: ServerPackageLayout::BattlegroupManagement,
            battlegroup_bat,
            ssh_key: ssh_key.is_file().then_some(ssh_key),
            bootstrap_setup: battlegroup_management_setup,
            vmcx_path,
        });
    }

    let legacy_setup = install_dir
        .join("internal-scripts")
        .join("bootstrap")
        .join("setup");
    let legacy_ssh_key = install_dir
        .join("internal-scripts")
        .join("ssh")
        .join("sshKey");
    if legacy_setup.is_file() && legacy_ssh_key.is_file() {
        return Ok(ServerPackageLayoutInfo {
            package_dir: install_dir.to_path_buf(),
            layout: ServerPackageLayout::LegacyInternalScripts,
            battlegroup_bat,
            ssh_key: Some(legacy_ssh_key),
            bootstrap_setup: legacy_setup,
            vmcx_path,
        });
    }
    Err(failure(format!(
        "Vendor bootstrap files were not found in supported layouts under {}",
        install_dir.display()
    )))
}

fn find_packaged_vmcx(install_dir: &Path) -> Option<PathBuf> {
    install_dir
        .join("Virtual Machines")
        .read_dir()
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("vmcx"))
        })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn detects_new_battlegroup_management_layout() {
        let root = temp_package_root("new-layout");
        fs::create_dir_all(root.join("Virtual Machines")).unwrap();
        fs::create_dir_all(root.join("battlegroup-management/bootstrap")).unwrap();
        fs::write(root.join("battlegroup.bat"), "").unwrap();
        fs::write(root.join("Virtual Machines/test.vmcx"), "").unwrap();
        fs::write(root.join("battlegroup-management/bootstrap/setup"), "setup").unwrap();

        let layout = detect_server_package_layout(&root).unwrap();

        assert_eq!(layout.layout, ServerPackageLayout::BattlegroupManagement);
        assert!(layout.ssh_key.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detects_legacy_internal_scripts_layout() {
        let root = temp_package_root("old-layout");
        fs::create_dir_all(root.join("Virtual Machines")).unwrap();
        fs::create_dir_all(root.join("internal-scripts/ssh")).unwrap();
        fs::create_dir_all(root.join("internal-scripts/bootstrap")).unwrap();
        fs::write(root.join("battlegroup.bat"), "").unwrap();
        fs::write(root.join("Virtual Machines/test.vmcx"), "").unwrap();
        fs::write(root.join("internal-scripts/ssh/sshKey"), "key").unwrap();
        fs::write(root.join("internal-scripts/bootstrap/setup"), "setup").unwrap();

        let layout = detect_server_package_layout(&root).unwrap();

        assert_eq!(layout.layout, ServerPackageLayout::LegacyInternalScripts);
        assert!(layout
            .ssh_key
            .as_ref()
            .is_some_and(|path| path.ends_with("sshKey")));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_package_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "dune-manager-toolchain-test-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
