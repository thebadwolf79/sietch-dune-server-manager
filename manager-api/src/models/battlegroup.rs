use serde::Serialize;

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
