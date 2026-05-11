use std::{env, net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::info;

mod auth;
mod clock;
mod config;
mod director_domain;
mod director_proxy;
mod errors;
mod kubernetes_domain;
mod models;
mod openapi;
mod routes;
mod security;
mod state;
mod validation;

use config::ManagerConfig;
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = ManagerConfig::from_env()?;
    let state = Arc::new(AppState::from_config(config).await?);
    let app = routes::router(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], state.port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;

    info!(
        namespace = %state.namespace,
        auth_enabled = state.token.is_some(),
        director_configured = state.director_base_url.is_some(),
        swagger_ui = "/swagger-ui",
        openapi = "/openapi.json",
        "manager API listening on {addr}"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")?;
    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG")
                .unwrap_or_else(|_| "dune_manager_api=info,tower_http=info".to_string()),
        )
        .init();
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
