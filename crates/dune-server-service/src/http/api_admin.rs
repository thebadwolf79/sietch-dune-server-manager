use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json};
use futures::FutureExt;
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
    let result = std::panic::AssertUnwindSafe(players::search_players(
        &state.env.pg,
        &state.env.kubectl,
        &state.env.cluster,
        &query,
        limit,
    ))
    .catch_unwind()
    .await;
    let rows = match result {
        Ok(Ok(rows)) => rows,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            tracing::error!("admin players route panicked");
            return Err(ApiError::internal("admin players route panicked"));
        }
    };
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

pub async fn welcome_grants(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let list = state.store.list_welcome_grants(q.limit.unwrap_or(100))?;
    Ok(Json(list))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryWelcomeGrantRequest {
    pub player_id: String,
    pub package_version: String,
    pub account_id: i64,
}

/// Clears a failed welcome-grant ledger row so the next scan re-attempts it.
pub async fn retry_welcome_grant(
    State(state): State<AppState>,
    Json(req): Json<RetryWelcomeGrantRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let player_id = req.player_id.trim();
    if player_id.is_empty() {
        return Err(ApiError::bad_request("playerId must not be empty"));
    }
    let package_version = req.package_version.trim();
    if package_version.is_empty() {
        return Err(ApiError::bad_request("packageVersion must not be empty"));
    }
    let removed =
        state
            .store
            .delete_welcome_grant(player_id, package_version, req.account_id)?;
    Ok(Json(serde_json::json!({ "ok": removed > 0, "removed": removed })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeWhisperRequest {
    pub recipient_player_id: String,
    #[serde(default)]
    pub source_player_id: String,
    pub message: String,
}

pub async fn welcome_whisper(
    State(state): State<AppState>,
    Json(req): Json<WelcomeWhisperRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let recipient = req.recipient_player_id.trim();
    if recipient.is_empty() {
        return Err(ApiError::bad_request(
            "recipient_player_id must not be empty",
        ));
    }
    let message = req.message.trim();
    if message.is_empty() {
        return Err(ApiError::bad_request("message must not be empty"));
    }
    if message.len() > 1000 {
        return Err(ApiError::bad_request("message must be <= 1000 characters"));
    }

    let cluster = state.env.cluster.get().await?;
    let result = crate::tasks::welcome_package::send_welcome_whisper_now(
        &state.env,
        &cluster.namespace,
        req.source_player_id.trim(),
        recipient,
        message,
    )
    .await;

    let (ok, output, error) = match result {
        Ok(pr) => (pr.ok, pr.output, None),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let payload = serde_json::json!({
        "sourcePlayerId": req.source_player_id.trim(),
        "recipientPlayerId": recipient,
        "message": message,
    });
    let _ = state.store.record_admin_command(
        "WelcomePackage.SendWelcomeWhisper",
        &payload,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": "WelcomePackage.SendWelcomeWhisper",
        "output": output,
        "error": error,
        "inner": payload,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantCurrencyRequest {
    pub fls_id: String,
    pub currency_id: i16,
    pub amount: i64,
}

/// Only House Scrip is grantable here. Refusing every other id keeps a fat-finger
/// (or a future caller passing the wrong constant) from ever touching Bank Solari
/// (currency_id 0) or any other balance.
const HOUSE_SCRIP_CURRENCY_ID: i16 = 1;
/// Upper bound on a single grant. Balances live in the 1e6–1e8 range; 1e9 is
/// generous headroom yet far below bigint, so the ADD can't overflow.
const MAX_CURRENCY_GRANT: i64 = 1_000_000_000;

/// Grant House Scrip via a guarded offline DB write (no engine command exists).
/// See `postgres::grant_currency` for the transaction + offline guarantees.
pub async fn grant_currency(
    State(state): State<AppState>,
    Json(req): Json<GrantCurrencyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let fls_id = req.fls_id.trim().to_string();
    if fls_id.is_empty() {
        return Err(ApiError::bad_request("flsId must not be empty"));
    }
    if req.currency_id != HOUSE_SCRIP_CURRENCY_ID {
        return Err(ApiError::bad_request(format!(
            "unsupported currencyId {}; this endpoint only grants House Scrip (currency_id {})",
            req.currency_id, HOUSE_SCRIP_CURRENCY_ID
        )));
    }
    if req.amount < 1 || req.amount > MAX_CURRENCY_GRANT {
        return Err(ApiError::bad_request(format!(
            "amount must be between 1 and {MAX_CURRENCY_GRANT}"
        )));
    }

    let cluster = state.env.cluster.get().await?;
    let result = crate::postgres::grant_currency(
        &state.env.pg,
        &cluster.namespace,
        &fls_id,
        req.currency_id,
        req.amount,
    )
    .await;

    use crate::postgres::CurrencyGrantResult as R;
    let (ok, output, error) = match result {
        Ok(R::Granted(o)) => (
            true,
            format!(
                "Granted {} House Scrip to controller {} (new balance {})",
                req.amount, o.player_controller_id, o.new_balance
            ),
            None,
        ),
        Ok(R::PlayerNotFound) => (
            false,
            String::new(),
            Some(format!("no player found for flsId {fls_id}")),
        ),
        Ok(R::Ambiguous) => (
            false,
            String::new(),
            Some(format!(
                "flsId {fls_id} resolves to multiple players; refusing to guess"
            )),
        ),
        Ok(R::PlayerOnline(status)) => (
            false,
            String::new(),
            Some(format!(
                "player must be offline to receive currency (status: {}); the server overwrites DB currency edits on logout",
                if status.is_empty() { "unknown" } else { status.as_str() }
            )),
        ),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let payload = serde_json::json!({
        "flsId": fls_id,
        "currencyId": req.currency_id,
        "amount": req.amount,
    });
    let _ = state.store.record_admin_command(
        "GrantHouseScrip",
        &payload,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": "GrantHouseScrip",
        "output": output,
        "error": error,
        "inner": payload,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwardIntelRequest {
    pub fls_id: String,
    pub amount: i64,
}

/// Tech Knowledge points are small (a few per level); 1e6 is far more than anyone
/// needs yet well under int32, which the in-engine field is assumed to be.
const MAX_INTEL_GRANT: i64 = 1_000_000;

/// Award Intel (Tech Knowledge points) via a guarded offline single-leaf jsonb_set.
/// See `postgres::grant_intel` for the blob-safety + offline guarantees.
pub async fn award_intel(
    State(state): State<AppState>,
    Json(req): Json<AwardIntelRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let fls_id = req.fls_id.trim().to_string();
    if fls_id.is_empty() {
        return Err(ApiError::bad_request("flsId must not be empty"));
    }
    if req.amount < 1 || req.amount > MAX_INTEL_GRANT {
        return Err(ApiError::bad_request(format!(
            "amount must be between 1 and {MAX_INTEL_GRANT}"
        )));
    }

    let cluster = state.env.cluster.get().await?;
    let result = crate::postgres::grant_intel(
        &state.env.pg,
        &cluster.namespace,
        &fls_id,
        req.amount,
    )
    .await;

    use crate::postgres::IntelGrantResult as R;
    let (ok, output, error) = match result {
        Ok(R::Granted { new_points }) => (
            true,
            format!("Awarded {} Intel to {fls_id} (new total {new_points})", req.amount),
            None,
        ),
        Ok(R::PlayerNotFound) => (
            false,
            String::new(),
            Some(format!("no player found for flsId {fls_id}")),
        ),
        Ok(R::Ambiguous) => (
            false,
            String::new(),
            Some(format!(
                "flsId {fls_id} resolves to multiple players; refusing to guess"
            )),
        ),
        Ok(R::PlayerOnline(status)) => (
            false,
            String::new(),
            Some(format!(
                "player must be offline to receive Intel (status: {}); the server overwrites DB edits on logout",
                if status.is_empty() { "unknown" } else { status.as_str() }
            )),
        ),
        Ok(R::CharacterActorMissing) => (
            false,
            String::new(),
            Some(format!(
                "could not find a character actor with a Tech Knowledge component for flsId {fls_id} (character never created, or unexpected blob shape)"
            )),
        ),
        Err(err) => {
            let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
            (false, String::new(), Some(scrubbed))
        }
    };

    let payload = serde_json::json!({ "flsId": fls_id, "amount": req.amount });
    let _ = state.store.record_admin_command(
        "AwardIntel",
        &payload,
        ok,
        error
            .as_deref()
            .or(if ok { None } else { Some(output.as_str()) }),
    );

    Ok(Json(serde_json::json!({
        "ok": ok,
        "command": "AwardIntel",
        "output": output,
        "error": error,
        "inner": payload,
    })))
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
