use serde::{Deserialize, Serialize};

fn default_ssh_port() -> u16 {
    22
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionRequest {
    pub host: String,
    pub key_path: Option<String>,
    pub server_type: Option<String>,
    pub user: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerActionRequest {
    pub server_type: Option<String>,
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub namespace: String,
    pub battlegroup_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTunnelStartRequest {
    pub tunnel_id: String,
    pub server_kind: String,
    pub service: String,
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub namespace: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTunnelStopRequest {
    pub tunnel_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTunnelStartRequest {
    pub tunnel_id: String,
    pub server_kind: String,
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub protocol: String,
    pub remote_port: u16,
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTunnelStatus {
    pub tunnel_id: String,
    pub service: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBattlegroupStatus {
    pub stop: bool,
    pub phase: String,
    #[serde(default)]
    pub database_phase: String,
    /// Wrapper's `Gateway` column. Kept under the old name for UI compatibility.
    pub server_group_phase: String,
    pub director_phase: String,
    #[serde(default)]
    pub uptime: String,
    #[serde(default)]
    pub server_stats: Vec<RemoteBattlegroupServerStat>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBattlegroupServerStat {
    pub map: String,
    pub phase: String,
    pub ready: String,
    pub players: String,
    pub age: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerStatus {
    pub battlegroup: RemoteBattlegroupStatus,
    pub package: RemoteServerPackageStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerPackageStatus {
    pub installed_build_id: Option<String>,
    pub battlegroup_version: Option<String>,
    pub live_battlegroup_version: Option<String>,
    pub operator_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerComponent {
    pub name: String,
    pub log_key: String,
    pub category: String,
    pub state: String,
    pub tone: String,
    pub summary: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteComponentLogRequest {
    pub server_type: Option<String>,
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub namespace: String,
    pub component: String,
    pub tail: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteComponentLogResult {
    pub component: String,
    pub output: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteComponentRestartRequest {
    pub server_type: Option<String>,
    pub host: String,
    pub user: String,
    pub key_path: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub namespace: String,
    pub component: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteComponentRestartResult {
    pub component: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServerRecord {
    #[serde(rename = "type")]
    pub server_type: String,
    pub id: String,
    pub name: String,
    pub host: String,
    pub user: String,
    pub key_path: String,
    pub port: u16,
    pub namespace: String,
    pub battlegroup_name: String,
    pub world_unique_name: String,
    pub phase: String,
}

/// Unified lifecycle state for a host-managed (Hyper-V) self-hosted server.
///
/// Authority for this value lives in Rust; the React UI renders it and gates
/// actions on it. Serialized as `{ "state": "<variant>", "data": <payload?> }`
/// so the frontend can switch on `state` and read step/reason/message detail.
///
/// Only meaningful when the manager runs *on* the Hyper-V host. In remote
/// (connect-only) mode the VM-level variants are not produced — see
/// `HostPermissionUnavailable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "data", rename_all = "camelCase")]
pub enum SystemState {
    /// State not yet determined.
    Unknown,
    /// Hyper-V is unreachable from this process: not on the host, Hyper-V not
    /// enabled, or the process lacks rights (non-elevated `Get-VM` is denied).
    /// The UI shows connect-only mode and disables VM power controls.
    HostPermissionUnavailable { reason: String },
    /// A terminal error with a human-readable message.
    Error { message: String },

    // VM-level states (Hyper-V VMState).
    VmOff,
    VmSaved,
    VmPaused,
    VmStarting { step: String },
    VmRunning,

    // Battlegroup-level states (once the VM is running + reachable).
    BattlegroupStopped,
    BattlegroupStarting { step: String },
    BattlegroupHealthy,
    BattlegroupDegraded { reason: String },
    BattlegroupStopping { step: String },
}

impl SystemState {
    /// Maps a Hyper-V `VMState` string (from `(Get-VM).State.ToString()`) to the
    /// corresponding VM-level `SystemState`. Unrecognized states map to `Unknown`
    /// rather than guessing, so the UI never enables an action on a state we don't
    /// understand.
    pub fn from_vm_state(vm_state: &str) -> Self {
        match vm_state.trim() {
            "Off" => SystemState::VmOff,
            "Saved" | "FastSaved" => SystemState::VmSaved,
            "Paused" => SystemState::VmPaused,
            "Running" => SystemState::VmRunning,
            "Starting" => SystemState::VmStarting {
                step: "Powering on".to_string(),
            },
            _ => SystemState::Unknown,
        }
    }

    /// Whether the VM can be powered on from this state (Off/Saved/Paused).
    pub fn can_start_vm(&self) -> bool {
        matches!(
            self,
            SystemState::VmOff | SystemState::VmSaved | SystemState::VmPaused
        )
    }

    /// Whether battlegroup actions should be enabled (VM is running).
    pub fn battlegroup_actions_enabled(&self) -> bool {
        matches!(
            self,
            SystemState::VmRunning
                | SystemState::BattlegroupStopped
                | SystemState::BattlegroupStarting { .. }
                | SystemState::BattlegroupHealthy
                | SystemState::BattlegroupDegraded { .. }
                | SystemState::BattlegroupStopping { .. }
        )
    }
}

#[cfg(test)]
mod system_state_tests {
    use super::SystemState;

    #[test]
    fn maps_known_vm_states() {
        assert_eq!(SystemState::from_vm_state("Off"), SystemState::VmOff);
        assert_eq!(SystemState::from_vm_state("Saved"), SystemState::VmSaved);
        assert_eq!(SystemState::from_vm_state("FastSaved"), SystemState::VmSaved);
        assert_eq!(SystemState::from_vm_state("Paused"), SystemState::VmPaused);
        assert_eq!(SystemState::from_vm_state("Running"), SystemState::VmRunning);
        assert!(matches!(
            SystemState::from_vm_state("Starting"),
            SystemState::VmStarting { .. }
        ));
    }

    #[test]
    fn unknown_vm_state_is_not_actionable() {
        let s = SystemState::from_vm_state("RunningCritical");
        assert_eq!(s, SystemState::Unknown);
        assert!(!s.can_start_vm());
        assert!(!s.battlegroup_actions_enabled());
    }

    #[test]
    fn only_off_saved_paused_can_start() {
        assert!(SystemState::VmOff.can_start_vm());
        assert!(SystemState::VmSaved.can_start_vm());
        assert!(SystemState::VmPaused.can_start_vm());
        assert!(!SystemState::VmRunning.can_start_vm());
    }

    #[test]
    fn battlegroup_actions_gated_on_vm_running() {
        assert!(SystemState::VmRunning.battlegroup_actions_enabled());
        assert!(SystemState::BattlegroupHealthy.battlegroup_actions_enabled());
        assert!(!SystemState::VmOff.battlegroup_actions_enabled());
        assert!(!SystemState::Unknown.battlegroup_actions_enabled());
    }

    #[test]
    fn serializes_with_state_tag() {
        let json = serde_json::to_string(&SystemState::VmOff).unwrap();
        assert_eq!(json, r#"{"state":"vmOff"}"#);
        let json = serde_json::to_string(&SystemState::VmStarting {
            step: "Powering on".to_string(),
        })
        .unwrap();
        assert_eq!(json, r#"{"state":"vmStarting","data":{"step":"Powering on"}}"#);
    }
}
