use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use tokio::sync::Mutex;

use super::KubectlClient;

const CACHE_TTL: Duration = Duration::from_secs(30);

static MQ_POD_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"-mq-game-sts-0$").unwrap());
static DB_POD_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"-db-dbdepl-sts-0$").unwrap());

#[derive(Debug, Clone, Serialize)]
pub struct Cluster {
    pub namespace: String,
    #[serde(rename = "mqPod")]
    pub mq_pod: String,
    #[serde(rename = "dbPod")]
    pub db_pod: Option<String>,
}

#[derive(Clone)]
pub struct ClusterCache {
    inner: Arc<Mutex<Option<CachedCluster>>>,
    kubectl: KubectlClient,
}

struct CachedCluster {
    value: Cluster,
    at: Instant,
}

impl ClusterCache {
    pub fn new(kubectl: KubectlClient) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            kubectl,
        }
    }

    pub async fn get(&self) -> Result<Cluster> {
        self.get_with(false).await
    }

    pub async fn refresh(&self) -> Result<Cluster> {
        self.get_with(true).await
    }

    async fn get_with(&self, force: bool) -> Result<Cluster> {
        {
            let guard = self.inner.lock().await;
            if !force {
                if let Some(cached) = guard.as_ref() {
                    if cached.at.elapsed() < CACHE_TTL {
                        return Ok(cached.value.clone());
                    }
                }
            }
        }

        let value = self.detect().await?;
        let mut guard = self.inner.lock().await;
        *guard = Some(CachedCluster {
            value: value.clone(),
            at: Instant::now(),
        });
        Ok(value)
    }

    async fn detect(&self) -> Result<Cluster> {
        let namespace = match self.kubectl.namespace_override() {
            Some(ns) => ns.to_string(),
            None => detect_namespace(&self.kubectl).await?,
        };

        let mq_pod = match self.kubectl.mq_pod_override() {
            Some(p) => p.to_string(),
            None => detect_pod(&self.kubectl, &namespace, &MQ_POD_PATTERN).await?,
        };

        let db_pod = match self.kubectl.db_pod_override() {
            Some(p) => Some(p.to_string()),
            None => detect_pod(&self.kubectl, &namespace, &DB_POD_PATTERN)
                .await
                .ok(),
        };

        Ok(Cluster {
            namespace,
            mq_pod,
            db_pod,
        })
    }
}

async fn detect_namespace(kubectl: &KubectlClient) -> Result<String> {
    let result = kubectl
        .run(&[
            "get",
            "pods",
            "-A",
            "--no-headers",
            "-o",
            "custom-columns=NS:.metadata.namespace,NAME:.metadata.name",
        ])
        .await?;
    result.require_ok("kubectl get pods -A")?;

    let mut candidates = std::collections::BTreeSet::<String>::new();
    for line in result.stdout.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let ns = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        if ns.starts_with("funcom-seabass-") && MQ_POD_PATTERN.is_match(name) {
            candidates.insert(ns.to_string());
        }
    }

    match candidates.len() {
        0 => Err(anyhow!(
            "no funcom-seabass-* namespace with a Game RMQ pod found"
        )),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => Err(anyhow!(
            "multiple candidate namespaces: {}; set DUNE_NAMESPACE",
            candidates.into_iter().collect::<Vec<_>>().join(", ")
        )),
    }
}

async fn detect_pod(kubectl: &KubectlClient, namespace: &str, pattern: &Regex) -> Result<String> {
    let result = kubectl
        .run(&[
            "get",
            "pods",
            "-n",
            namespace,
            "--no-headers",
            "-o",
            "custom-columns=NAME:.metadata.name",
        ])
        .await?;
    result.require_ok(&format!("kubectl get pods -n {namespace}"))?;

    for line in result.stdout.split('\n') {
        let name = line.split_whitespace().next().unwrap_or("");
        if !name.is_empty() && pattern.is_match(name) {
            return Ok(name.to_string());
        }
    }
    Err(anyhow!(
        "no pod matching {} in namespace {namespace}",
        pattern.as_str()
    ))
}
