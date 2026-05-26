use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

pub mod battlegroup;
pub mod battlegroup_cli;
pub mod cluster;
pub mod steam;

pub use battlegroup_cli::{BattlegroupCli, ReadySummary};
pub use cluster::{Cluster, ClusterCache};
pub use steam::SteamCmd;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct KubectlClient {
    use_sudo: bool,
    namespace_override: Option<String>,
    mq_pod_override: Option<String>,
    db_pod_override: Option<String>,
}

#[derive(Debug)]
pub struct ProcessResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessResult {
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }

    pub fn require_ok(&self, ctx: &str) -> Result<()> {
        if self.ok() {
            return Ok(());
        }
        let stderr = self.stderr.trim();
        let detail = if stderr.is_empty() {
            "no detail".to_string()
        } else {
            stderr.to_string()
        };
        Err(anyhow!(
            "{ctx} failed (exit {}): {}",
            self.exit_code,
            detail
        ))
    }
}

impl KubectlClient {
    pub fn new(
        use_sudo: bool,
        namespace_override: Option<String>,
        mq_pod_override: Option<String>,
        db_pod_override: Option<String>,
    ) -> Self {
        Self {
            use_sudo,
            namespace_override,
            mq_pod_override,
            db_pod_override,
        }
    }

    pub fn namespace_override(&self) -> Option<&str> {
        self.namespace_override.as_deref()
    }

    pub fn mq_pod_override(&self) -> Option<&str> {
        self.mq_pod_override.as_deref()
    }

    pub fn db_pod_override(&self) -> Option<&str> {
        self.db_pod_override.as_deref()
    }

    pub async fn run(&self, args: &[&str]) -> Result<ProcessResult> {
        self.run_with(args, None, DEFAULT_TIMEOUT).await
    }

    pub async fn run_with_stdin(&self, args: &[&str], stdin: &str) -> Result<ProcessResult> {
        self.run_with(args, Some(stdin), DEFAULT_TIMEOUT).await
    }

    pub async fn run_timeout(
        &self,
        args: &[&str],
        stdin: Option<&str>,
        timeout_secs: u64,
    ) -> Result<ProcessResult> {
        self.run_with(args, stdin, Duration::from_secs(timeout_secs))
            .await
    }

    async fn run_with(
        &self,
        args: &[&str],
        stdin: Option<&str>,
        dur: Duration,
    ) -> Result<ProcessResult> {
        let (program, full_args): (&str, Vec<&str>) = if self.use_sudo {
            (
                "sudo",
                std::iter::once("kubectl")
                    .chain(args.iter().copied())
                    .collect(),
            )
        } else {
            ("kubectl", args.to_vec())
        };

        let mut cmd = Command::new(program);
        cmd.args(&full_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::debug!(
            program = program,
            args = %full_args.join(" "),
            path = %std::env::var("PATH").unwrap_or_default(),
            "spawning subprocess"
        );

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning {program} {}", full_args.join(" ")))?;

        if let Some(input) = stdin {
            if let Some(mut writer) = child.stdin.take() {
                writer
                    .write_all(input.as_bytes())
                    .await
                    .context("writing stdin to kubectl")?;
                writer.shutdown().await.ok();
            }
        } else {
            drop(child.stdin.take());
        }

        let wait_fut = child.wait_with_output();
        match timeout(dur, wait_fut).await {
            Ok(Ok(out)) => Ok(ProcessResult {
                exit_code: out.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            }),
            Ok(Err(err)) => Err(err).context("kubectl process error"),
            Err(_) => Err(anyhow!(
                "kubectl timed out after {}s: {}",
                dur.as_secs(),
                full_args.join(" ")
            )),
        }
    }
}

/// Spawn a non-kubectl process (for `battlegroup` wrapper, `steamcmd`, etc.).
/// Uses the same capture + timeout semantics.
pub async fn run_process(
    program: &str,
    args: &[&str],
    stdin: Option<&str>,
    timeout_secs: u64,
) -> Result<ProcessResult> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning {program} {}", args.join(" ")))?;

    if let Some(input) = stdin {
        if let Some(mut writer) = child.stdin.take() {
            writer
                .write_all(input.as_bytes())
                .await
                .with_context(|| format!("writing stdin to {program}"))?;
            writer.shutdown().await.ok();
        }
    } else {
        drop(child.stdin.take());
    }

    let dur = Duration::from_secs(timeout_secs);
    match timeout(dur, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(ProcessResult {
            exit_code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
        Ok(Err(err)) => Err(err).with_context(|| format!("{program} process error")),
        Err(_) => Err(anyhow!(
            "{program} timed out after {}s: {}",
            dur.as_secs(),
            args.join(" ")
        )),
    }
}
