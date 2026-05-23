//! Host-side server package install/status flow.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use crate::{
    errors::{command_failure, failure},
    models::CommandResult,
    shell::suppress_console_window,
    toolchain::{
        manager::Toolchain,
        package_detection::{
            detect_server_package_layout, server_package_exists, ServerPackageInstallResult,
            ServerPackageLayout,
        },
        tool_models::ManagedTool,
        vdf::{
            query_latest_server_build_id, read_installed_server_build_id, LEGACY_SERVER_APP_ID,
            LEGACY_SERVER_MANIFEST_PATH, SERVER_APP_ID, SERVER_MANIFEST_PATH,
        },
    },
};

/// Version and completeness status for the host-side Dune server package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackageStatus {
    /// Package root directory.
    pub package_dir: PathBuf,
    /// Steam app id used for the package.
    pub app_id: String,
    /// Installed Steam build id from the local manifest.
    pub installed_build_id: Option<String>,
    /// Latest Steam public branch build id when SteamCMD could report it.
    pub latest_build_id: Option<String>,
    /// Whether the local build is older than the latest known build.
    pub update_available: bool,
    /// Whether the app recognized all required package assets.
    pub complete: bool,
    /// Detected vendor layout, when complete enough to identify.
    pub layout: Option<ServerPackageLayout>,
    /// Human-readable status details or recovery hint.
    pub message: String,
}

impl Toolchain {
    /// Installs or validates the host-side Dune server package with SteamCMD.
    pub fn install_server_package(
        &self,
        install_dir: impl AsRef<Path>,
    ) -> CommandResult<ServerPackageInstallResult> {
        let steamcmd = self.status(ManagedTool::SteamCmd);
        if !steamcmd.installed {
            return Err(failure("SteamCMD is not installed"));
        }
        let install_dir = install_dir.as_ref();
        remove_legacy_server_package_if_needed(install_dir)?;
        let mut command = Command::new(&steamcmd.executable);
        suppress_console_window(&mut command);
        let output = command
            .args([
                "+@ShutdownOnFailedCommand",
                "1",
                "+@NoPromptForPassword",
                "1",
                "+force_install_dir",
            ])
            .arg(install_dir)
            .args([
                "+login",
                "anonymous",
                "+app_update",
                SERVER_APP_ID,
                "validate",
                "+quit",
            ])
            .output()
            .map_err(|err| failure(format!("Failed to run SteamCMD: {err}")))?;
        let combined_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let installed = combined_output
            .contains(&format!("Success! App '{SERVER_APP_ID}' fully installed."))
            || server_package_exists(install_dir);
        if !output.status.success() && !installed {
            return Err(command_failure(
                "SteamCMD server package install failed",
                output,
            ));
        }
        Ok(ServerPackageInstallResult {
            install_dir: install_dir.to_path_buf(),
            app_id: SERVER_APP_ID.to_string(),
            installed,
        })
    }

    /// Reads host-side server package status and optionally the latest Steam build id.
    pub fn server_package_status(
        &self,
        install_dir: impl AsRef<Path>,
    ) -> CommandResult<ServerPackageStatus> {
        let install_dir = install_dir.as_ref();
        let legacy_installed = legacy_server_package_installed(install_dir);
        let layout = detect_server_package_layout(install_dir).ok();
        let installed_build_id = read_installed_server_build_id(install_dir);
        let latest_build_id = if self.status(ManagedTool::SteamCmd).installed {
            query_latest_server_build_id(&self.status(ManagedTool::SteamCmd).executable).ok()
        } else {
            None
        };
        let update_available = legacy_installed
            || installed_build_id
                .as_deref()
                .zip(latest_build_id.as_deref())
                .is_some_and(|(installed, latest)| installed != latest);
        let complete = layout.is_some() && !legacy_installed;
        let message = match (&layout, &installed_build_id, &latest_build_id) {
            _ if legacy_installed => format!(
                "Old playtest server package app {LEGACY_SERVER_APP_ID} is installed; update the package to install release app {SERVER_APP_ID}."
            ),
            (Some(info), Some(installed), Some(latest)) if installed == latest => {
                format!("{:?} package is current at build {installed}.", info.layout)
            }
            (Some(info), Some(installed), Some(latest)) => {
                format!(
                    "{:?} package build {installed} is older than latest build {latest}.",
                    info.layout
                )
            }
            (Some(info), Some(installed), None) => {
                format!(
                    "{:?} package build {installed} is installed; latest build is unknown.",
                    info.layout
                )
            }
            (Some(info), None, _) => format!(
                "{:?} package assets are present but the Steam manifest build id was not found.",
                info.layout
            ),
            (None, _, _) => {
                "Server package is missing required VM or bootstrap assets.".to_string()
            }
        };
        Ok(ServerPackageStatus {
            package_dir: install_dir.to_path_buf(),
            app_id: SERVER_APP_ID.to_string(),
            installed_build_id,
            latest_build_id,
            update_available,
            complete,
            layout: layout.map(|info| info.layout),
            message,
        })
    }
}

fn legacy_server_package_installed(install_dir: &Path) -> bool {
    install_dir.join(LEGACY_SERVER_MANIFEST_PATH).is_file()
        && !install_dir.join(SERVER_MANIFEST_PATH).is_file()
}

fn remove_legacy_server_package_if_needed(install_dir: &Path) -> CommandResult<()> {
    let legacy_manifest = install_dir.join(LEGACY_SERVER_MANIFEST_PATH);
    if !legacy_manifest.is_file() {
        return Ok(());
    }
    if install_dir.join(SERVER_MANIFEST_PATH).is_file() {
        fs::remove_file(&legacy_manifest).map_err(|err| {
            failure(format!(
                "Failed to remove old server package manifest {}: {err}",
                legacy_manifest.display()
            ))
        })?;
        return Ok(());
    }
    if !install_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(install_dir).map_err(|err| {
        failure(format!(
            "Failed to inspect old server package at {}: {err}",
            install_dir.display()
        ))
    })? {
        let path = entry
            .map_err(|err| {
                failure(format!(
                    "Failed to inspect old server package at {}: {err}",
                    install_dir.display()
                ))
            })?
            .path();
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|err| {
                failure(format!(
                    "Failed to remove old server package directory {}: {err}",
                    path.display()
                ))
            })?;
        } else {
            fs::remove_file(&path).map_err(|err| {
                failure(format!(
                    "Failed to remove old server package file {}: {err}",
                    path.display()
                ))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn removes_legacy_package_before_release_install() {
        let root = temp_package_root("legacy-cleanup");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::create_dir_all(root.join("internal-scripts")).unwrap();
        fs::write(root.join("steamapps/appmanifest_3104830.acf"), "old").unwrap();
        fs::write(root.join("internal-scripts/old.txt"), "old").unwrap();
        fs::write(root.join("old-root.txt"), "old").unwrap();

        remove_legacy_server_package_if_needed(&root).unwrap();

        assert!(root.exists());
        assert!(!root.join("steamapps").exists());
        assert!(!root.join("internal-scripts").exists());
        assert!(!root.join("old-root.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn removes_stale_legacy_manifest_when_release_manifest_exists() {
        let root = temp_package_root("stale-legacy-manifest");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        fs::write(root.join("steamapps/appmanifest_3104830.acf"), "old").unwrap();
        fs::write(root.join("steamapps/appmanifest_4754530.acf"), "new").unwrap();

        remove_legacy_server_package_if_needed(&root).unwrap();

        assert!(!root.join("steamapps/appmanifest_3104830.acf").exists());
        assert!(root.join("steamapps/appmanifest_4754530.acf").exists());
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
