use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryEnvelope {
    pub event_type: String,
    pub time_unix_ms: u128,
    pub payload: Value,
}
