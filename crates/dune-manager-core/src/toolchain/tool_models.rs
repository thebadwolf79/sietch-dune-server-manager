//! Managed tool enum and per-tool installation status models.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{errors::failure, models::CommandResult};

pub(super) const STEAMCMD_URL: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";
pub(super) const OPENSSH_URL: &str =
    "https://github.com/PowerShell/Win32-OpenSSH/releases/latest/download/OpenSSH-Win64.zip";

/// External command-line tool managed under the app-owned tools directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagedTool {
    /// Valve SteamCMD client.
    #[serde(rename = "steamcmd")]
    SteamCmd,
    /// Windows OpenSSH client distribution.
    #[serde(rename = "openssh")]
    OpenSsh,
}

impl ManagedTool {
    /// Parses a CLI/user-facing tool name.
    pub fn parse(value: &str) -> CommandResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "steamcmd" | "steam-cmd" => Ok(Self::SteamCmd),
            "openssh" | "open-ssh" | "ssh" => Ok(Self::OpenSsh),
            _ => Err(failure(format!(
                "Unknown managed tool {value}; expected steamcmd or openssh"
            ))),
        }
    }

    /// Stable identifier used in tool paths and JSON output.
    pub fn id(self) -> &'static str {
        match self {
            Self::SteamCmd => "steamcmd",
            Self::OpenSsh => "openssh",
        }
    }

    /// Executable filename expected after installation.
    pub fn executable_name(self) -> &'static str {
        match self {
            Self::SteamCmd => "steamcmd.exe",
            Self::OpenSsh => "ssh.exe",
        }
    }

    /// Default archive URL used for installation.
    pub fn default_url(self) -> &'static str {
        match self {
            Self::SteamCmd => STEAMCMD_URL,
            Self::OpenSsh => OPENSSH_URL,
        }
    }
}

/// Installation status for one managed tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    /// Managed tool.
    pub tool: ManagedTool,
    /// Whether the expected executable exists.
    pub installed: bool,
    /// Root directory for manager-owned data.
    pub tools_root: PathBuf,
    /// Directory where the tool is installed.
    pub install_dir: PathBuf,
    /// Expected executable path.
    pub executable: PathBuf,
}

/// Result of installing or reusing a managed tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInstallResult {
    /// Status after the install attempt.
    pub status: ToolStatus,
    /// Source archive URL used or selected.
    pub source_url: String,
    /// Whether the installer performed work in this call.
    pub installed_now: bool,
}
