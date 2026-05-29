use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tauri::Manager;

use crate::state::TunnelRegistry;

pub fn ensure_client(app: &tauri::AppHandle) -> Client {
    if let Some(client) = app.try_state::<Client>() {
        return client.inner().clone();
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .expect("reqwest client builds");
    app.manage(client.clone());
    client
}

fn tunnel_local_port(registry: &TunnelRegistry, tunnel_id: &str) -> Result<u16, String> {
    let tunnels = registry
        .tunnels
        .lock()
        .map_err(|_| "tunnel registry unavailable".to_string())?;
    let tunnel = tunnels
        .get(tunnel_id.trim())
        .ok_or_else(|| format!("no active tunnel id={tunnel_id}"))?;
    Ok(tunnel.status.local_port)
}

async fn get_json(client: &Client, port: u16, path: &str) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("GET {path}: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("GET {path} -> {status}: {body_text}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|err| format!("decoding {path}: {err}"))
}

async fn post_json(client: &Client, port: u16, path: &str, body: &Value) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = client
        .post(&url)
        .json(body)
        .send()
        .await
        .map_err(|err| format!("POST {path}: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("POST {path} -> {status}: {body_text}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|err| format!("decoding {path}: {err}"))
}

#[tauri::command]
pub async fn ms_health(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/health").await
}

#[tauri::command]
pub async fn ms_list_runs(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    limit: Option<u32>,
    task: Option<String>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let mut path = String::from("/api/runs");
    let mut sep = '?';
    if let Some(l) = limit {
        path.push(sep);
        path.push_str(&format!("limit={l}"));
        sep = '&';
    }
    if let Some(t) = task {
        path.push(sep);
        path.push_str(&format!("task={t}"));
    }
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_list_logs(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    limit: Option<u32>,
    run_id: Option<i64>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let mut path = String::from("/api/logs");
    let mut sep = '?';
    if let Some(l) = limit {
        path.push(sep);
        path.push_str(&format!("limit={l}"));
        sep = '&';
    }
    if let Some(r) = run_id {
        path.push(sep);
        path.push_str(&format!("runId={r}"));
    }
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_trigger_run(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    task: String,
    options: Option<Value>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let mut body = serde_json::Map::new();
    body.insert("task".to_string(), Value::String(task));
    if let Some(opts) = options {
        body.insert("options".to_string(), opts);
    }
    post_json(&client, port, "/api/runs/trigger", &Value::Object(body)).await
}

#[tauri::command]
pub async fn ms_list_commands(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/admin/commands").await
}

#[tauri::command]
pub async fn ms_search_items(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/items", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_search_vehicles(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/vehicles", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_search_skill_modules(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/skill-modules", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_search_journey_nodes(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/journey-nodes", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_search_xp_event_tags(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/xp-event-tags", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_search_players(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    q: Option<String>,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(
        &client,
        port,
        &search_path("/api/admin/players", q.as_deref(), limit),
    )
    .await
}

#[tauri::command]
pub async fn ms_cluster(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/admin/cluster").await
}

#[tauri::command]
pub async fn ms_player_location(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    fls_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let path = format!("/api/admin/player-location?flsId={}", urlencoding(&fls_id));
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_get_config(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/config").await
}

#[tauri::command]
pub async fn ms_set_config(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    config: Value,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    post_json(&client, port, "/api/config", &config).await
}

#[tauri::command]
pub async fn ms_list_timezones(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/timezones").await
}

#[tauri::command]
pub async fn ms_cron_preview(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    expr: String,
    count: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let mut path = format!("/api/cron/preview?expr={}", urlencoding(&expr));
    if let Some(c) = count {
        path.push_str(&format!("&count={c}"));
    }
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_dump_prune_preview(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    get_json(&client, port, "/api/maintenance/dump-prune").await
}

#[tauri::command]
pub async fn ms_dump_prune_execute(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    items: Value,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let body = serde_json::json!({ "items": items });
    post_json(&client, port, "/api/maintenance/dump-prune", &body).await
}

#[tauri::command]
pub async fn ms_history(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let path = match limit {
        Some(l) => format!("/api/admin/history?limit={l}"),
        None => String::from("/api/admin/history"),
    };
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_welcome_grants(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    limit: Option<u32>,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    let path = match limit {
        Some(l) => format!("/api/admin/welcome-grants?limit={l}"),
        None => String::from("/api/admin/welcome-grants"),
    };
    get_json(&client, port, &path).await
}

#[tauri::command]
pub async fn ms_welcome_grant_retry(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    player_id: String,
    package_version: String,
    account_id: i64,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    post_json(
        &client,
        port,
        "/api/admin/welcome-grants/retry",
        &serde_json::json!({
            "playerId": player_id,
            "packageVersion": package_version,
            "accountId": account_id,
        }),
    )
    .await
}

#[tauri::command]
pub async fn ms_publish(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    command: String,
    fields: Value,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    post_json(
        &client,
        port,
        "/api/admin/publish",
        &serde_json::json!({ "command": command, "fields": fields }),
    )
    .await
}

#[tauri::command]
pub async fn ms_welcome_whisper(
    app: tauri::AppHandle,
    registry: tauri::State<'_, TunnelRegistry>,
    tunnel_id: String,
    recipient_player_id: String,
    source_player_id: String,
    message: String,
) -> Result<Value, String> {
    let port = tunnel_local_port(&registry, &tunnel_id)?;
    let client = ensure_client(&app);
    post_json(
        &client,
        port,
        "/api/admin/welcome-whisper",
        &serde_json::json!({
            "recipientPlayerId": recipient_player_id,
            "sourcePlayerId": source_player_id,
            "message": message,
        }),
    )
    .await
}

fn search_path(base: &str, q: Option<&str>, limit: Option<u32>) -> String {
    let mut out = base.to_string();
    let mut sep = '?';
    if let Some(qq) = q {
        out.push(sep);
        out.push_str(&format!("q={}", urlencoding(qq)));
        sep = '&';
    }
    if let Some(l) = limit {
        out.push(sep);
        out.push_str(&format!("limit={l}"));
    }
    out
}

fn urlencoding(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    out
}
