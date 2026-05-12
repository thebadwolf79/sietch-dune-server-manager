use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct DatabaseWorldPartition {
    pub partition_id: i64,
    pub server_id: Option<String>,
    pub map: String,
    pub partition_definition: String,
    pub dimension_index: i32,
    pub blocked: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseWorldPartitionsResponse {
    pub namespace: String,
    pub rows: Vec<DatabaseWorldPartition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseWorldPartitionUpdateRequest {
    pub blocked: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseWorldPartitionUpdateResponse {
    pub namespace: String,
    pub row: DatabaseWorldPartition,
}
