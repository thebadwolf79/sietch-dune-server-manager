use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SetupSelections {
    pub steamcmd_path: String,
    pub steamcmd_install_dir: String,
    pub server_install_dir: String,
    pub vm_destination_path: String,
    pub vm_switch_name: String,
    pub physical_adapter_name: String,
    pub memory_gb: u32,
    pub vm_ip_mode: String,
    pub static_ip: String,
    pub static_cidr: String,
    pub static_gateway: String,
    pub static_dns: String,
    pub player_ip_mode: String,
    pub manual_player_ip: String,
    pub world_name: String,
    pub world_region: String,
    pub bootstrap_profile_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SetupPersistedState {
    pub current_stage: String,
    pub completed_stages: Vec<String>,
    pub last_error: String,
    pub log_path: String,
    pub selections: SetupSelections,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SteamCmdDetection {
    pub found: bool,
    pub path: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SetupState {
    pub persisted: SetupPersistedState,
    pub steamcmd: SteamCmdDetection,
    pub suggested_steamcmd_install_dir: String,
    pub suggested_server_install_dir: String,
    pub server_installed: bool,
    pub server_install_path: String,
    pub vm_exists: bool,
    pub vm_state: String,
    pub vm_ip: String,
    pub elevated: bool,
    pub hyperv_available: bool,
    pub vmms_running: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DriveOption {
    pub name: String,
    pub root: String,
    pub free_gb: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapterOption {
    pub name: String,
    pub interface_description: String,
    pub ipv4_address: String,
    pub prefix_length: u8,
    pub cidr: String,
    pub gateway: String,
    pub bound_switch_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VmSwitchOption {
    pub name: String,
    pub switch_type: String,
    pub net_adapter_interface_description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VmImportOptions {
    pub vmcx_path: String,
    pub existing_vm: bool,
    pub existing_vm_state: String,
    pub drives: Vec<DriveOption>,
    pub network_adapters: Vec<NetworkAdapterOption>,
    pub switches: Vec<VmSwitchOption>,
    pub suggested_destination: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VmDestinationStatus {
    pub exists: bool,
    pub is_empty: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetupCommandResult {
    pub ok: bool,
    pub stage: String,
    pub message: String,
    pub stdout: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GuestBootstrapRequest {
    pub install_path: String,
    pub ip: String,
    pub player_ip: String,
    pub static_ip: String,
    pub static_cidr: String,
    pub static_gateway: String,
    pub static_dns: String,
    pub world_name: String,
    pub region: String,
    pub self_host_token: String,
    pub profile_id: String,
}
