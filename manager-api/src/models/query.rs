use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub pod: String,
    pub container: Option<String>,
    pub tail: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStreamQuery {
    pub pod: String,
    pub container: Option<String>,
    pub tail: Option<i64>,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}
