use anyhow::{Context, Result};
use kube::Client;

use crate::{clock::now_unix_ms, config::ManagerConfig};

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub namespace: String,
    pub token: Option<String>,
    pub director_base_url: Option<String>,
    pub http: reqwest::Client,
    pub started_unix_ms: u128,
    pub port: u16,
}

impl AppState {
    pub async fn from_config(config: ManagerConfig) -> Result<Self> {
        Ok(Self {
            client: Client::try_default()
                .await
                .context("failed to create Kubernetes client")?,
            namespace: config.namespace,
            token: config.token,
            director_base_url: config.director_base_url,
            http: reqwest::Client::new(),
            started_unix_ms: now_unix_ms(),
            port: config.port,
        })
    }
}
