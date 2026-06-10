use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use axum::Router;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

use crate::config::ServiceConfig;
use crate::scheduler::TaskRunner;
use crate::store::Store;
use crate::tasks::TaskEnv;

pub mod api_admin;
pub mod api_runs;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub env: Arc<TaskEnv>,
    pub runner: Arc<TaskRunner>,
}

impl AppState {
    pub fn new(store: Store, env: Arc<TaskEnv>, runner: Arc<TaskRunner>) -> Self {
        Self { store, env, runner }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", axum::routing::get(api_runs::root))
        .route("/api/health", axum::routing::get(api_runs::health))
        .route("/api/runs", axum::routing::get(api_runs::list_runs))
        .route("/api/logs", axum::routing::get(api_runs::list_logs))
        .route("/api/runs/trigger", axum::routing::post(api_runs::trigger))
        .route(
            "/api/config",
            axum::routing::get(api_runs::get_config).post(api_runs::set_config),
        )
        .route(
            "/api/timezones",
            axum::routing::get(api_runs::list_timezones),
        )
        .route(
            "/api/cron/preview",
            axum::routing::get(api_runs::cron_preview),
        )
        .route(
            "/api/maintenance/dump-prune",
            axum::routing::get(api_runs::dump_prune_preview).post(api_runs::dump_prune_execute),
        )
        .route(
            "/api/admin/commands",
            axum::routing::get(api_admin::list_commands),
        )
        .route(
            "/api/admin/items",
            axum::routing::get(api_admin::search_items),
        )
        .route(
            "/api/admin/vehicles",
            axum::routing::get(api_admin::search_vehicles),
        )
        .route(
            "/api/admin/skill-modules",
            axum::routing::get(api_admin::search_skill_modules),
        )
        .route(
            "/api/admin/journey-nodes",
            axum::routing::get(api_admin::search_journey_nodes),
        )
        .route(
            "/api/admin/xp-event-tags",
            axum::routing::get(api_admin::search_xp_event_tags),
        )
        .route(
            "/api/admin/players",
            axum::routing::get(api_admin::search_players),
        )
        .route(
            "/api/admin/player-location",
            axum::routing::get(api_admin::player_location),
        )
        .route("/api/admin/cluster", axum::routing::get(api_admin::cluster))
        .route("/api/admin/history", axum::routing::get(api_admin::history))
        .route(
            "/api/admin/welcome-grants",
            axum::routing::get(api_admin::welcome_grants),
        )
        .route(
            "/api/admin/welcome-grants/retry",
            axum::routing::post(api_admin::retry_welcome_grant),
        )
        .route(
            "/api/admin/welcome-whisper",
            axum::routing::post(api_admin::welcome_whisper),
        )
        .route(
            "/api/admin/publish",
            axum::routing::post(api_admin::publish),
        )
        .route(
            "/api/admin/grant-currency",
            axum::routing::post(api_admin::grant_currency),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

pub fn version() -> &'static str {
    VERSION
}

pub async fn serve(cfg: &ServiceConfig, state: AppState, cancel: CancellationToken) -> Result<()> {
    let addr = build_bind_address(&cfg.dashboard_host, cfg.dashboard_port)?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "http server listening");

    let app = router(state);

    let shutdown = async move {
        cancel.cancelled().await;
        tracing::info!("http server shutdown signal received");
    };

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown)
        .await
        .context("axum::serve")?;
    Ok(())
}

fn build_bind_address(host: &str, port: u16) -> Result<SocketAddr> {
    let candidate = if host == "localhost" {
        format!("127.0.0.1:{port}")
    } else {
        format!("{host}:{port}")
    };
    SocketAddr::from_str(&candidate)
        .map_err(|err| anyhow!("invalid bind address {candidate}: {err}"))
}
