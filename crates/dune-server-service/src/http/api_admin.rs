use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::admin::{commands, data, players, MqPublisher};
use crate::store::AdminHistoryFilter;

use super::api_runs::ApiError;
use super::AppState;

pub async fn list_commands() -> impl IntoResponse {
    Json(commands::SPECS)
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<u32>,
}

pub async fn search_items(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_items(&query, limit))
}

pub async fn search_vehicles(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(20);
    Json(data::search_vehicles(&query, limit))
}

pub async fn search_skill_modules(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_skill_modules(&query, limit))
}

pub async fn search_journey_nodes(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(80);
    Json(data::search_journey_nodes(&query, limit))
}

pub async fn search_xp_event_tags(Query(q): Query<SearchQuery>) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    Json(data::search_xp_event_tags(&query, limit))
}

pub async fn search_players(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    let rows = players::search_players(&state.env.pg, &state.env.cluster, &query, limit).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct PlayerLocationQuery {
    #[serde(rename = "flsId")]
    pub fls_id: String,
}

pub async fn player_location(
    State(state): State<AppState>,
    Query(q): Query<PlayerLocationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    use crate::postgres::PositionProbe;
    let cluster = state.env.cluster.get().await?;
    let probe =
        crate::postgres::get_player_location(&state.env.pg, &cluster.namespace, &q.fls_id).await?;
    match probe {
        PositionProbe::Found(p) => Ok(Json(p).into_response()),
        PositionProbe::NoRow => Err(ApiError::not_found(format!(
            "no live pawn for fls_id {} — player may be offline",
            q.fls_id
        ))),
    }
}

pub async fn cluster(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let c = state.env.cluster.get().await?;
    Ok(Json(serde_json::json!({
        "namespace": c.namespace,
        "mqPod": c.mq_pod,
        "dbPod": c.db_pod,
        "serviceVersion": super::VERSION,
    })))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<u32>,
}

pub async fn history(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let list = state
        .store
        .list_admin_commands(AdminHistoryFilter { limit: q.limit })?;
    Ok(Json(list))
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub command: String,
    #[serde(default)]
    pub fields: Map<String, Value>,
}

pub async fn publish(
    State(state): State<AppState>,
    Json(req): Json<PublishRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let inner = commands::validate_and_build(&req.command, &req.fields)
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    let publisher: &MqPublisher = &state.env.mq;
    let result = publisher.publish_inner(&inner, &req.command).await;

    let (ok, output, error) = match result {
        Ok(pr) => (pr.ok, pr.output, None),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let _ = state.store.record_admin_command(
        &req.command,
        &inner,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": req.command,
        "output": output,
        "error": error,
        "inner": inner,
    })))
}
