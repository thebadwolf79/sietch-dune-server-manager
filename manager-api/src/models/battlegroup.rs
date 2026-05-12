use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleGroupSummary {
    pub namespace: String,
    pub name: String,
    pub title: String,
    pub phase: String,
    pub stop: bool,
    pub server_sets: usize,
    pub server_image: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSetSummary {
    pub map: String,
    pub replicas: u64,
    pub memory_limit: String,
    pub dedicated_scaling: bool,
    pub image: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleGroupDetail {
    pub namespace: String,
    pub name: String,
    pub title: String,
    pub phase: String,
    pub stop: bool,
    pub database_phase: String,
    pub server_group_phase: String,
    pub gateway_phase: String,
    pub director_phase: String,
    pub server_image: String,
    pub utility_images: Vec<String>,
    pub server_sets: Vec<ServerSetSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldLayout {
    pub hagga_basin_instances: usize,
    pub social_hubs_enabled: bool,
    pub deep_desert_pve_instances: usize,
    pub deep_desert_pvp_instances: usize,
    pub deep_desert_total_instances: usize,
    pub deep_desert_partition_ids: Vec<i64>,
    pub restart_required: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldLayoutUpdateRequest {
    pub hagga_basin_instances: Option<usize>,
    pub social_hubs_enabled: Option<bool>,
    pub deep_desert_pve_instances: Option<usize>,
    pub deep_desert_pvp_instances: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldLayoutUpdateResponse {
    pub layout: WorldLayout,
    pub battlegroup_patched: bool,
    pub pvp_config_updated: bool,
    pub restart_required: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleGroupSettingsRequest {
    pub title: Option<String>,
}
