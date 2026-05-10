use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use k8s_openapi::{
    api::core::v1::{Pod, Service},
    apimachinery::pkg::util::intstr::IntOrString,
};
use kube::{
    api::{ApiResource, DynamicObject, ListParams, LogParams, Patch, PatchParams},
    Api, Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    env,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, time};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

const DEFAULT_PORT: u16 = 8787;

#[derive(Clone)]
struct AppState {
    client: Client,
    namespace: String,
    token: Option<String>,
    director_base_url: Option<String>,
    http: reqwest::Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogQuery {
    pod: String,
    container: Option<String>,
    tail: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    ok: bool,
    namespace: String,
    auth_enabled: bool,
    director_configured: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    namespace: String,
    auth_enabled: bool,
    director_configured: bool,
    battlegroups: usize,
    pods: usize,
    services: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PodSummary {
    name: String,
    phase: String,
    ready: bool,
    restarts: i32,
    node_name: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServicePortSummary {
    name: Option<String>,
    port: i32,
    target_port: Option<String>,
    node_port: Option<i32>,
    protocol: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceSummary {
    name: String,
    service_type: Option<String>,
    cluster_ip: Option<String>,
    external_ips: Vec<String>,
    ports: Vec<ServicePortSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BattleGroupSummary {
    namespace: String,
    name: String,
    title: String,
    phase: String,
    stop: bool,
    server_sets: usize,
    server_image: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerSetSummary {
    map: String,
    replicas: u64,
    memory_limit: String,
    dedicated_scaling: bool,
    image: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BattleGroupDetail {
    namespace: String,
    name: String,
    title: String,
    phase: String,
    stop: bool,
    database_phase: String,
    server_group_phase: String,
    gateway_phase: String,
    director_phase: String,
    server_image: String,
    utility_images: Vec<String>,
    server_sets: Vec<ServerSetSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkloadsResponse {
    pods: Vec<PodSummary>,
    services: Vec<ServiceSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryEnvelope {
    event_type: String,
    time_unix_ms: u128,
    payload: Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG")
                .unwrap_or_else(|_| "dune_manager_api=info,tower_http=info".to_string()),
        )
        .init();

    let namespace = env::var("DUNE_NAMESPACE")
        .or_else(|_| env::var("POD_NAMESPACE"))
        .unwrap_or_else(|_| "default".to_string());
    let token = env::var("MANAGER_API_TOKEN")
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty());
    let allow_no_auth = env::var("MANAGER_API_ALLOW_NO_AUTH")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if token.is_none() && !allow_no_auth {
        return Err(anyhow!(
            "MANAGER_API_TOKEN is required unless MANAGER_API_ALLOW_NO_AUTH=true"
        ));
    }
    let director_base_url = env::var("DIRECTOR_BASE_URL")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty());
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let state = Arc::new(AppState {
        client: Client::try_default()
            .await
            .context("failed to create Kubernetes client")?,
        namespace,
        token,
        director_base_url,
        http: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/status", get(status))
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
        .route("/api/telemetry", get(telemetry))
        .with_state(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    info!(
        namespace = %state.namespace,
        auth_enabled = state.token.is_some(),
        director_configured = state.director_base_url.is_some(),
        "manager API listening on {addr}"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
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
    let battlegroups = list_battlegroups(&state).await?.len();
    let pods = list_pods(&state).await?.len();
    let services = list_services(&state).await?.len();

    Ok(Json(StatusResponse {
        namespace: state.namespace.clone(),
        auth_enabled: state.token.is_some(),
        director_configured: state.director_base_url.is_some(),
        battlegroups,
        pods,
        services,
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
    Ok(Json(WorkloadsResponse {
        pods: list_pods(&state).await?,
        services: list_services(&state).await?,
    }))
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
    let base_url = state
        .director_base_url
        .as_ref()
        .ok_or_else(|| ApiError::bad_gateway("DIRECTOR_BASE_URL is not configured"))?;
    let value = state
        .http
        .get(format!("{base_url}/v0/battlegroup"))
        .send()
        .await
        .context("failed to call Director")?
        .error_for_status()
        .context("Director returned an error")?
        .json::<Value>()
        .await
        .context("failed to parse Director response")?;

    Ok(Json(redact_json(value)))
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
    let battlegroups = list_battlegroups(state).await?;
    let pods = list_pods(state).await?;
    let services = list_services(state).await?;

    Ok(json!({
        "namespace": state.namespace,
        "battlegroups": battlegroups,
        "pods": pods,
        "services": services
    }))
}

async fn list_pods(state: &AppState) -> Result<Vec<PodSummary>> {
    let pods: Api<Pod> = Api::namespaced(state.client.clone(), &state.namespace);
    let list = pods
        .list(&ListParams::default())
        .await
        .context("failed to list pods")?;

    Ok(list
        .items
        .into_iter()
        .map(|pod| {
            let status = pod.status.unwrap_or_default();
            let container_statuses = status.container_statuses.unwrap_or_default();
            PodSummary {
                name: pod.metadata.name.unwrap_or_default(),
                phase: status.phase.unwrap_or_default(),
                ready: !container_statuses.is_empty()
                    && container_statuses.iter().all(|container| container.ready),
                restarts: container_statuses
                    .iter()
                    .map(|container| container.restart_count)
                    .sum(),
                node_name: pod.spec.and_then(|spec| spec.node_name),
                created_at: pod
                    .metadata
                    .creation_timestamp
                    .map(|time| time.0.to_rfc3339()),
            }
        })
        .collect())
}

async fn list_services(state: &AppState) -> Result<Vec<ServiceSummary>> {
    let services: Api<Service> = Api::namespaced(state.client.clone(), &state.namespace);
    let list = services
        .list(&ListParams::default())
        .await
        .context("failed to list services")?;

    Ok(list
        .items
        .into_iter()
        .map(|service| {
            let spec = service.spec.unwrap_or_default();
            ServiceSummary {
                name: service.metadata.name.unwrap_or_default(),
                service_type: spec.type_,
                cluster_ip: spec.cluster_ip,
                external_ips: spec.external_ips.unwrap_or_default(),
                ports: spec
                    .ports
                    .unwrap_or_default()
                    .into_iter()
                    .map(|port| ServicePortSummary {
                        name: port.name,
                        port: port.port,
                        target_port: port.target_port.map(int_or_string_to_string),
                        node_port: port.node_port,
                        protocol: port.protocol,
                    })
                    .collect(),
            }
        })
        .collect())
}

async fn list_battlegroups(state: &AppState) -> Result<Vec<BattleGroupSummary>> {
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    let list = api
        .list(&ListParams::default())
        .await
        .context("failed to list battlegroups")?;

    Ok(list
        .items
        .into_iter()
        .map(|item| battlegroup_summary(&state.namespace, item))
        .collect())
}

async fn get_battlegroup_object(state: &AppState, name: &str) -> Result<DynamicObject> {
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    api.get(name)
        .await
        .with_context(|| format!("failed to get battlegroup {name}"))
}

async fn patch_battlegroup_stop(
    state: &AppState,
    namespace: &str,
    name: &str,
    stop: bool,
) -> Result<(), ApiError> {
    validate_namespace(state, namespace)?;
    validate_kube_name(name)?;
    let api: Api<DynamicObject> = Api::namespaced_with(
        state.client.clone(),
        &state.namespace,
        &battlegroup_resource(),
    );
    api.patch(
        name,
        &PatchParams::default(),
        &Patch::Merge(json!({ "spec": { "stop": stop } })),
    )
    .await
    .with_context(|| format!("failed to patch battlegroup {name}"))?;
    Ok(())
}

fn battlegroup_resource() -> ApiResource {
    ApiResource {
        group: "igw.funcom.com".to_string(),
        version: "v1".to_string(),
        api_version: "igw.funcom.com/v1".to_string(),
        kind: "BattleGroup".to_string(),
        plural: "battlegroups".to_string(),
    }
}

fn int_or_string_to_string(value: IntOrString) -> String {
    match value {
        IntOrString::Int(value) => value.to_string(),
        IntOrString::String(value) => value,
    }
}

fn battlegroup_summary(default_namespace: &str, item: DynamicObject) -> BattleGroupSummary {
    let namespace = item
        .metadata
        .namespace
        .unwrap_or_else(|| default_namespace.to_string());
    let name = item.metadata.name.unwrap_or_default();
    let data = serde_json::to_value(item.data).unwrap_or_else(|_| json!({}));
    let sets = data["spec"]["serverGroup"]["template"]["spec"]["sets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let server_image = sets
        .first()
        .and_then(|set| set["image"].as_str())
        .unwrap_or_default()
        .to_string();

    BattleGroupSummary {
        namespace,
        name,
        title: data["spec"]["title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        phase: data["status"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        stop: data["spec"]["stop"].as_bool().unwrap_or(false),
        server_sets: sets.len(),
        server_image,
    }
}

fn battlegroup_detail_from_object(
    default_namespace: &str,
    item: DynamicObject,
) -> BattleGroupDetail {
    let namespace = item
        .metadata
        .namespace
        .unwrap_or_else(|| default_namespace.to_string());
    let name = item.metadata.name.unwrap_or_default();
    let data = serde_json::to_value(item.data).unwrap_or_else(|_| json!({}));
    let server_sets = summarize_server_sets(&data);
    let server_image = server_sets
        .first()
        .map(|set| set.image.clone())
        .unwrap_or_default();
    let mut utility_images = Vec::new();

    for path in [
        &data["spec"]["utilities"]["director"]["spec"]["image"],
        &data["spec"]["utilities"]["serverGateway"]["spec"]["image"],
        &data["spec"]["utilities"]["textRouter"]["spec"]["image"],
        &data["spec"]["utilities"]["fileBrowser"]["spec"]["image"],
    ] {
        if let Some(image) = path.as_str() {
            utility_images.push(image.to_string());
        }
    }

    for template in data["spec"]["utilities"]["messageQueues"]["templates"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        if let Some(image) = template["spec"]["image"].as_str() {
            utility_images.push(image.to_string());
        }
    }

    BattleGroupDetail {
        namespace,
        name,
        title: data["spec"]["title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        phase: data["status"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        stop: data["spec"]["stop"].as_bool().unwrap_or(false),
        database_phase: data["status"]["database"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        server_group_phase: data["status"]["serverGroup"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        gateway_phase: data["status"]["serverGateway"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        director_phase: data["status"]["director"]["phase"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        server_image,
        utility_images: unique_strings(utility_images.into_iter()),
        server_sets,
    }
}

fn summarize_server_sets(data: &Value) -> Vec<ServerSetSummary> {
    data["spec"]["serverGroup"]["template"]["spec"]["sets"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|set| ServerSetSummary {
            map: set["map"].as_str().unwrap_or_default().to_string(),
            replicas: set["replicas"].as_u64().unwrap_or_default(),
            memory_limit: set["resources"]["limits"]["memory"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            dedicated_scaling: set["dedicatedScaling"].as_bool().unwrap_or(false),
            image: set["image"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

fn unique_strings(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        if !value.is_empty() && !output.contains(&value) {
            output.push(value);
        }
    }
    output
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> Result<(), ApiError> {
    let Some(expected) = state.token.as_deref() else {
        return Ok(());
    };

    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or(query_token);

    match bearer {
        Some(actual) if constant_time_eq(actual.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => Err(ApiError::unauthorized()),
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn validate_kube_name(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err(ApiError::bad_request("invalid Kubernetes resource name"));
    }
    Ok(())
}

fn validate_namespace(state: &AppState, namespace: &str) -> Result<(), ApiError> {
    validate_kube_name(namespace)?;
    if namespace != state.namespace {
        return Err(ApiError::bad_request(format!(
            "manager API is scoped to namespace {}",
            state.namespace
        )));
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn redact_json(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, child)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("token")
                        || lower.contains("secret")
                        || lower.contains("password")
                        || lower.contains("apikey")
                        || lower.contains("auth")
                    {
                        (key, Value::String("<redacted>".to_string()))
                    } else {
                        (key, redact_json(child))
                    }
                })
                .collect::<serde_json::Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_json).collect()),
        Value::String(text) if looks_like_jwt(&text) => Value::String("<redacted>".to_string()),
        other => other,
    }
}

fn redact_text(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("apikey")
                || lower.contains("auth")
            {
                "<redacted>".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn looks_like_jwt(value: &str) -> bool {
    let mut parts = value.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), Some(c), None) if a.len() > 8 && b.len() > 8 && c.len() > 8
    )
}

type ApiResponse<T> = Result<Json<T>, ApiError>;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "unauthorized".to_string(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}
