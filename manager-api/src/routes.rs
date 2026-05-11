use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        OriginalUri, Path, Query, State,
    },
    http::{HeaderMap, Method},
    response::Response,
    routing::{any, get, post},
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::LogParams, Api};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, sync::Arc, time::Duration};
use tokio::time;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::authorize,
    clock::now_unix_ms,
    director_domain::*,
    director_proxy::*,
    errors::*,
    kubernetes_domain::*,
    models::*,
    openapi,
    security::{redact_json, redact_text},
    state::AppState,
    validation::{validate_kube_name, validate_namespace},
};

const MANAGER_API_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/status", get(status))
        .route("/api/manager/self", get(manager_self))
        .route("/api/battlegroups", get(battlegroups))
        .route(
            "/api/battlegroups/:namespace/:name",
            get(battlegroup_detail),
        )
        .route(
            "/api/battlegroups/:namespace/:name/raw",
            get(battlegroup_raw),
        )
        .route(
            "/api/battlegroups/:namespace/:name/start",
            post(start_battlegroup),
        )
        .route(
            "/api/battlegroups/:namespace/:name/stop",
            post(stop_battlegroup),
        )
        .route(
            "/api/battlegroups/:namespace/:name/restart",
            post(restart_battlegroup),
        )
        .route("/api/pods", get(pods))
        .route("/api/services", get(services))
        .route("/api/workloads", get(workloads))
        .route("/api/logs", get(logs))
        .route("/api/director/battlegroup", get(director_battlegroup))
        .route("/api/director/capabilities", get(director_capabilities))
        .route(
            "/api/director/players/summary",
            get(director_players_summary),
        )
        .route("/api/director/players", get(director_players))
        .route("/api/director/maps", get(director_maps))
        .route(
            "/api/director/config/fls",
            get(director_fls_config)
                .post(director_update_fls_config)
                .delete(director_clear_fls_config),
        )
        .route(
            "/api/director/config/character-transfer",
            get(director_character_transfer_config)
                .post(director_update_character_transfer_config)
                .delete(director_clear_character_transfer_config),
        )
        .route(
            "/api/director/config/maps/:map_name/override",
            post(director_update_map_override).delete(director_clear_map_override),
        )
        .route("/api/director/v0/*path", any(director_api_proxy))
        .route("/v0/*path", any(director_root_api_proxy))
        .route("/director", any(director_ui_proxy_root))
        .route("/director/*path", any(director_ui_proxy))
        .route("/Script/*path", any(director_script_proxy))
        .route("/Stylesheet/*path", any(director_stylesheet_proxy))
        .route("/Icons/*path", any(director_icons_proxy))
        .route("/api/telemetry", get(telemetry))
        .with_state(state)
        .merge(
            SwaggerUi::new("/swagger-ui")
                .external_url_unchecked("/openapi.json", openapi::document()),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

fn audit_action(action: &str, target: Option<&str>) {
    match target {
        Some(target) => info!(target: "audit", action, target, "mutating manager action"),
        None => info!(target: "audit", action, "mutating manager action"),
    }
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        api_version: MANAGER_API_VERSION,
        namespace: state.namespace.clone(),
        auth_enabled: state.token.is_some(),
        director_configured: state.director_base_url.is_some(),
    })
}

async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<StatusResponse> {
    authorize(&state, &headers, None)?;
    let (battlegroups, pods, services, director_configured) = tokio::join!(
        list_battlegroups(&state),
        list_pods(&state),
        list_services(&state),
        resolve_director_base_url(&state),
    );
    let battlegroups = battlegroups?.len();
    let pods = pods?.len();
    let services = services?.len();
    let director_configured = director_configured.is_ok();

    Ok(Json(StatusResponse {
        api_version: MANAGER_API_VERSION,
        namespace: state.namespace.clone(),
        auth_enabled: state.token.is_some(),
        director_configured,
        battlegroups,
        pods,
        services,
    }))
}

async fn manager_self(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<ManagerSelfResponse> {
    authorize(&state, &headers, None)?;
    Ok(Json(ManagerSelfResponse {
        api_version: MANAGER_API_VERSION,
        started_unix_ms: state.started_unix_ms,
        uptime_seconds: ((now_unix_ms()).saturating_sub(state.started_unix_ms) / 1000) as u64,
        pid: std::process::id(),
        namespace: state.namespace.clone(),
        port: state.port,
        auth_enabled: state.token.is_some(),
        director_configured: resolve_director_base_url(&state).await.is_ok(),
        current_exe: env::current_exe()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        service_name: "dune-manager-api",
        binary_path: "/opt/dune-manager/dune-manager-api",
        env_path: "/etc/dune-manager-api.env",
        log_path: "/var/log/dune-manager-api.log",
    }))
}

async fn battlegroups(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Vec<BattleGroupSummary>> {
    authorize(&state, &headers, None)?;
    Ok(Json(list_battlegroups(&state).await?))
}

async fn battlegroup_detail(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResponse<BattleGroupDetail> {
    authorize(&state, &headers, None)?;
    validate_namespace(&state, &namespace)?;
    validate_kube_name(&name)?;
    let item = get_battlegroup_object(&state, &name).await?;
    Ok(Json(battlegroup_detail_from_object(&state.namespace, item)))
}

async fn battlegroup_raw(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    validate_namespace(&state, &namespace)?;
    validate_kube_name(&name)?;
    let item = get_battlegroup_object(&state, &name).await?;
    let value = serde_json::to_value(item).context("failed to serialize battlegroup")?;
    Ok(Json(redact_json(value)))
}

async fn start_battlegroup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResponse<BattleGroupDetail> {
    authorize(&state, &headers, None)?;
    audit_action("battlegroup.start", Some(&format!("{namespace}/{name}")));
    patch_battlegroup_stop(&state, &namespace, &name, false).await?;
    let item = get_battlegroup_object(&state, &name).await?;
    Ok(Json(battlegroup_detail_from_object(&state.namespace, item)))
}

async fn stop_battlegroup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResponse<BattleGroupDetail> {
    authorize(&state, &headers, None)?;
    audit_action("battlegroup.stop", Some(&format!("{namespace}/{name}")));
    patch_battlegroup_stop(&state, &namespace, &name, true).await?;
    let item = get_battlegroup_object(&state, &name).await?;
    Ok(Json(battlegroup_detail_from_object(&state.namespace, item)))
}

async fn restart_battlegroup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResponse<BattleGroupDetail> {
    authorize(&state, &headers, None)?;
    audit_action("battlegroup.restart", Some(&format!("{namespace}/{name}")));
    patch_battlegroup_stop(&state, &namespace, &name, true).await?;
    time::sleep(Duration::from_secs(5)).await;
    patch_battlegroup_stop(&state, &namespace, &name, false).await?;
    let item = get_battlegroup_object(&state, &name).await?;
    Ok(Json(battlegroup_detail_from_object(&state.namespace, item)))
}

async fn pods(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Vec<PodSummary>> {
    authorize(&state, &headers, None)?;
    Ok(Json(list_pods(&state).await?))
}

async fn services(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Vec<ServiceSummary>> {
    authorize(&state, &headers, None)?;
    Ok(Json(list_services(&state).await?))
}

async fn workloads(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<WorkloadsResponse> {
    authorize(&state, &headers, None)?;
    let (pods, services) = tokio::try_join!(list_pods(&state), list_services(&state))?;
    Ok(Json(WorkloadsResponse { pods, services }))
}

async fn logs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<LogQuery>,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    validate_kube_name(&query.pod)?;
    if let Some(container) = &query.container {
        validate_kube_name(container)?;
    }

    let pods: Api<Pod> = Api::namespaced(state.client.clone(), &state.namespace);
    let mut params = LogParams {
        tail_lines: Some(query.tail.unwrap_or(200).clamp(1, 2000)),
        ..Default::default()
    };
    params.container = query.container.clone();
    let text = pods
        .logs(&query.pod, &params)
        .await
        .with_context(|| format!("failed to read logs for pod {}", query.pod))?;

    Ok(Json(json!({
        "pod": query.pod,
        "container": query.container,
        "lines": redact_text(&text).lines().collect::<Vec<_>>()
    })))
}

async fn director_battlegroup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    let value = director_get_json(&state, "/v0/battlegroup").await?;

    Ok(Json(redact_json(value)))
}

async fn director_capabilities(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<DirectorCapabilities> {
    authorize(&state, &headers, None)?;
    Ok(Json(DirectorCapabilities {
        configured: resolve_director_base_url(&state).await.is_ok(),
        api_paths: director_capabilities_list(),
        ui_proxy_path: "/director",
    }))
}

async fn director_players_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<DirectorPlayerSummary> {
    authorize(&state, &headers, None)?;
    let value = director_get_json(&state, "/v0/battlegroup").await?;
    Ok(Json(director_player_summary(&value)))
}

async fn director_players(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<DirectorPlayersQuery>,
) -> ApiResponse<DirectorPlayerLists> {
    authorize(&state, &headers, None)?;
    let base_url = ensure_director_available(&state).await?;
    let all = director_get_json_with_base(&state, &base_url, "/v0/players").await?;
    let online = director_get_json_with_base(&state, &base_url, "/v0/players/online").await?;

    let (in_transit, grace_period, completion, queued) = if query.full.unwrap_or(false) {
        (
            director_get_json_with_base(&state, &base_url, "/v0/players/intransit").await?,
            director_get_json_with_base(&state, &base_url, "/v0/players/graceperiod").await?,
            director_get_json_with_base(&state, &base_url, "/v0/players/completion").await?,
            director_get_json_with_base(&state, &base_url, "/v0/players/queued").await?,
        )
    } else {
        let empty = json!([]);
        (empty.clone(), empty.clone(), empty.clone(), empty)
    };
    Ok(Json(director_player_lists(
        &all,
        &online,
        &in_transit,
        &grace_period,
        &completion,
        &queued,
    )))
}

#[derive(Debug, Deserialize)]
struct DirectorPlayersQuery {
    full: Option<bool>,
}

async fn director_maps(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Vec<DirectorMapSummary>> {
    authorize(&state, &headers, None)?;
    let value = director_get_json(&state, "/v0/battlegroup").await?;
    Ok(Json(director_map_summaries(&value)))
}

async fn director_fls_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    let value = director_get_json(&state, "/v0/BattlegroupFetchFlsReportSettings").await?;
    Ok(Json(redact_json(value)))
}

async fn director_update_fls_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    audit_action("director.fls.update", None);
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupUpdateFlsReportSettings",
        None,
        body,
    )
    .await
}

async fn director_clear_fls_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    audit_action("director.fls.clear", None);
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupClearFlsReportOverrides",
        None,
        Bytes::new(),
    )
    .await
}

async fn director_character_transfer_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    let value = director_get_json(&state, "/v0/BattlegroupFetchCharacterTransferRules").await?;
    Ok(Json(redact_json(value)))
}

async fn director_update_character_transfer_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    audit_action("director.character_transfer.update", None);
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupUpdateCharacterTransferSettings",
        None,
        body,
    )
    .await
}

async fn director_clear_character_transfer_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    audit_action("director.character_transfer.clear", None);
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupClearCharacterTransferOverrides",
        None,
        Bytes::new(),
    )
    .await
}

async fn director_update_map_override(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(map_name): Path<String>,
    body: Bytes,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    validate_director_map_name(&map_name)?;
    audit_action("director.map_override.update", Some(&map_name));
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupUpdateServerGroupConfig",
        None,
        body,
    )
    .await
}

async fn director_clear_map_override(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(map_name): Path<String>,
) -> ApiResponse<Value> {
    authorize(&state, &headers, None)?;
    validate_director_map_name(&map_name)?;
    audit_action("director.map_override.clear", Some(&map_name));
    proxy_director_json(
        &state,
        Method::POST,
        "/v0/BattlegroupClearMapConfigOverrides",
        None,
        Bytes::from(map_name),
    )
    .await
}

async fn director_api_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query_token(uri.query()))?;
    let director_path = format!("/v0/{path}");
    if !is_allowed_director_api(method.as_str(), &director_path) {
        return Err(ApiError::bad_request(
            "Director API path is not allowlisted",
        ));
    }
    proxy_director_response(
        &state,
        method,
        &director_path,
        director_query(uri.query()),
        body,
        None,
    )
    .await
}

async fn director_root_api_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query_token(uri.query()))?;
    let director_path = format!("/v0/{path}");
    if !is_allowed_director_api(method.as_str(), &director_path) {
        return Err(ApiError::bad_request(
            "Director API path is not allowlisted",
        ));
    }
    proxy_director_response(
        &state,
        method,
        &director_path,
        director_query(uri.query()),
        body,
        None,
    )
    .await
}

async fn director_ui_proxy_root(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query_token(uri.query()))?;
    proxy_director_response(
        &state,
        method,
        "/",
        director_query(uri.query()),
        body,
        query_token(uri.query()),
    )
    .await
}

async fn director_script_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    director_static_proxy(state, headers, path, uri, method, body, "Script").await
}

async fn director_stylesheet_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    director_static_proxy(state, headers, path, uri, method, body, "Stylesheet").await
}

async fn director_icons_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    director_static_proxy(state, headers, path, uri, method, body, "Icons").await
}

async fn director_static_proxy(
    state: Arc<AppState>,
    headers: HeaderMap,
    path: String,
    uri: axum::http::Uri,
    method: Method,
    body: Bytes,
    prefix: &str,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query_token(uri.query()))?;
    if !is_safe_static_path(&path) {
        return Err(ApiError::bad_request("invalid Director static path"));
    }
    let director_path = format!("/{prefix}/{path}");
    proxy_director_response(
        &state,
        method,
        &director_path,
        director_query(uri.query()),
        body,
        None,
    )
    .await
}

async fn director_ui_proxy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: Bytes,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query_token(uri.query()))?;
    let director_path = format!("/{path}");
    proxy_director_response(
        &state,
        method,
        &director_path,
        director_query(uri.query()),
        body,
        query_token(uri.query()),
    )
    .await
}

async fn telemetry(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
) -> Result<Response, ApiError> {
    authorize(&state, &headers, query.token.as_deref())?;
    Ok(ws.on_upgrade(move |socket| telemetry_socket(socket, state)))
}

async fn telemetry_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut interval = time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                match telemetry_snapshot(&state).await {
                    Ok(payload) => {
                        let envelope = TelemetryEnvelope {
                            event_type: "snapshot".to_string(),
                            time_unix_ms: now_unix_ms(),
                            payload,
                        };
                        match serde_json::to_string(&envelope) {
                            Ok(text) => {
                                if sender.send(Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => error!(?err, "failed to serialize telemetry event"),
                        }
                    }
                    Err(err) => {
                        let envelope = TelemetryEnvelope {
                            event_type: "error".to_string(),
                            time_unix_ms: now_unix_ms(),
                            payload: json!({ "message": err.to_string() }),
                        };
                        if let Ok(text) = serde_json::to_string(&envelope) {
                            let _ = sender.send(Message::Text(text)).await;
                        }
                    }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn telemetry_snapshot(state: &AppState) -> Result<Value> {
    let (battlegroups, pods, services) = tokio::try_join!(
        list_battlegroups(state),
        list_pods(state),
        list_services(state)
    )?;

    Ok(json!({
        "namespace": state.namespace,
        "battlegroups": battlegroups,
        "pods": pods,
        "services": services
    }))
}
