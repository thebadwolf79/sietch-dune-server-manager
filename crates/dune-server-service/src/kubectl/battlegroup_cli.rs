use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::time::{sleep, Instant};

use super::{battlegroup, run_process, KubectlClient};

/// Wraps the vendor `battlegroup` helper at `${DUNE_BIN_DIR}/battlegroup` plus
/// the readiness-polling utilities both the daily-restart and update-apply
/// shell scripts hand-rolled.
#[derive(Clone)]
pub struct BattlegroupCli {
    bin: PathBuf,
}

impl BattlegroupCli {
    pub fn new(bin_dir: &std::path::Path) -> Self {
        Self {
            bin: bin_dir.join("battlegroup"),
        }
    }

    fn bin_str(&self) -> String {
        self.bin.to_string_lossy().into_owned()
    }

    pub async fn stop(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["stop"], None, 120).await?;
        result.require_ok("battlegroup stop")
    }

    pub async fn start(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["start"], None, 120).await?;
        result.require_ok("battlegroup start")
    }

    pub async fn restart(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["restart"], None, 1200).await?;
        result.require_ok("battlegroup restart")
    }

    pub async fn update(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["update"], None, 3600).await?;
        result.require_ok("battlegroup update")
    }

    pub async fn backup(&self, backup_name: &str) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["backup", backup_name], None, 600).await?;
        result.require_ok(&format!("battlegroup backup {backup_name}"))
    }

    pub async fn update_from_downloads(&self) -> Result<()> {
        let bin = self.bin_str();
        let result = run_process(&bin, &["update-from-downloads"], None, 600).await?;
        result.require_ok("battlegroup update-from-downloads")
    }
}

/// Wait for the battlegroup to reach a fully stopped state: `spec.stop=true`
/// AND no server pods (matching `-sg-...-pod-`) present.
pub async fn wait_until_stopped(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
    timeout: Duration,
) -> Result<()> {
    let start = Instant::now();
    let interval = Duration::from_secs(10);
    while start.elapsed() < timeout {
        let stop_value = battlegroup::bg_field(kubectl, namespace, bg_name, "{.spec.stop}")
            .await
            .unwrap_or_default();
        let pod_count = count_server_pods(kubectl, namespace).await.unwrap_or(0);
        tracing::info!(
            stop = %stop_value,
            pods = pod_count,
            elapsed_s = start.elapsed().as_secs(),
            "waiting for battlegroup stop"
        );
        if stop_value == "true" && pod_count == 0 {
            return Ok(());
        }
        sleep(interval).await;
    }
    Err(anyhow!(
        "timeout waiting for battlegroup {bg_name} to stop after {}s",
        timeout.as_secs()
    ))
}

/// Wait for the battlegroup to reach a fully-running state: serverGroupPhase
/// is "Running" AND all servers report ready=true with phase=Running.
pub async fn wait_until_running(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
    timeout: Duration,
) -> Result<ReadySummary> {
    let start = Instant::now();
    let interval = Duration::from_secs(10);
    while start.elapsed() < timeout {
        if let Ok(summary) = ready_summary(kubectl, namespace, bg_name).await {
            tracing::info!(
                phase = %summary.phase,
                server_group_phase = %summary.server_group_phase,
                ready = %format!("{}/{}", summary.ready, summary.size),
                elapsed_s = start.elapsed().as_secs(),
                "waiting for battlegroup run"
            );
            if summary.is_running() {
                return Ok(summary);
            }
        }
        sleep(interval).await;
    }
    Err(anyhow!(
        "timeout waiting for battlegroup {bg_name} to become ready after {}s",
        timeout.as_secs()
    ))
}

pub async fn count_server_pods(kubectl: &KubectlClient, namespace: &str) -> Result<usize> {
    let result = kubectl
        .run(&[
            "get",
            "pods",
            "-n",
            namespace,
            "--no-headers",
            "-o",
            "custom-columns=NAME:.metadata.name,DEL:.metadata.deletionTimestamp",
        ])
        .await?;
    result.require_ok(&format!("kubectl get pods -n {namespace}"))?;
    let mut count = 0;
    for line in result.stdout.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let name = parts.next().unwrap_or("");
        let deletion = parts.next().unwrap_or("");
        if name.contains("-sg-")
            && name.contains("-pod-")
            && (deletion.is_empty() || deletion == "<none>")
        {
            count += 1;
        }
    }
    Ok(count)
}

#[derive(Debug, Clone)]
pub struct ReadySummary {
    pub phase: String,
    pub server_group_phase: String,
    pub ready: u32,
    pub size: u32,
}

impl ReadySummary {
    pub fn is_running(&self) -> bool {
        self.server_group_phase == "Running" && self.size > 0 && self.ready == self.size
    }
}

pub async fn ready_summary(
    kubectl: &KubectlClient,
    namespace: &str,
    bg_name: &str,
) -> Result<ReadySummary> {
    let bg = battlegroup::bg_json(kubectl, namespace, bg_name).await?;
    let status = bg
        .get("status")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let phase = status
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let server_group_phase = status
        .get("serverGroupPhase")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let servers = status
        .get("servers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let ready = servers
        .iter()
        .filter(|s| {
            s.get("ready").and_then(|v| v.as_bool()).unwrap_or(false)
                && s.get("phase").and_then(|v| v.as_str()) == Some("Running")
        })
        .count() as u32;
    let size = status
        .get("size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or_else(|| servers.len() as u32);
    Ok(ReadySummary {
        phase,
        server_group_phase,
        ready,
        size,
    })
}
