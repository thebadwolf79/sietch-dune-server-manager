use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub api_version: &'static str,
    pub namespace: String,
    pub auth_enabled: bool,
    pub director_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub api_version: &'static str,
    pub namespace: String,
    pub auth_enabled: bool,
    pub director_configured: bool,
    pub battlegroups: usize,
    pub pods: usize,
    pub services: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub authenticated: bool,
    pub api_version: &'static str,
    pub namespace: String,
    pub auth_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewResponse {
    pub status: StatusResponse,
    pub battlegroups: Vec<crate::models::BattleGroupSummary>,
    pub workloads: crate::models::WorkloadsResponse,
    pub director_available: bool,
    pub players: Option<crate::models::DirectorPlayerSummary>,
    pub maps: Vec<crate::models::DirectorMapSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerSelfResponse {
    pub api_version: &'static str,
    pub started_unix_ms: u128,
    pub uptime_seconds: u64,
    pub pid: u32,
    pub namespace: String,
    pub port: u16,
    pub auth_enabled: bool,
    pub director_configured: bool,
    pub current_exe: String,
    pub service_name: &'static str,
    pub binary_path: &'static str,
    pub env_path: &'static str,
    pub log_path: &'static str,
}
