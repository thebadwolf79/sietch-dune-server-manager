use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::Utc;

use crate::kubectl::battlegroup as bg;
use crate::kubectl::{run_process, KubectlClient};
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::tasks::TaskEnv;

/// Polls the dump pod every this often while the vendor backup runs.
const DUMP_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// RAII guard for the background dump-pod monitor. Dropping it stops the poll
/// loop (`active = false`) and aborts the task, so the monitor can never outlive
/// the backup it watches — on success, error, or panic alike.
struct MonitorGuard {
    active: Arc<AtomicBool>,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Relaxed);
        self.handle.abort();
    }
}

/// Replaces `scripts/cron-battlegroup-backup`. Runs the vendor backup helper,
/// emits a per-run log line referencing the dump path, and lets the operator
/// handle stale dump cleanup out-of-band (we do not invoke `sudo find -delete`
/// from the daemon — too easy to widen the blast radius).
pub struct BackupTask {
    env: Arc<TaskEnv>,
}

impl BackupTask {
    pub fn new(env: Arc<TaskEnv>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl Task for BackupTask {
    fn id(&self) -> &'static str {
        "backup"
    }

    fn schedule(&self) -> Schedule {
        // Two gates: the `backup_enabled` master switch and a parsed cron.
        // Vendor backups block server I/O for the whole dump, so a cron must be
        // set explicitly (see seb851's report of in-play perf hits with the old
        // 2h default). The switch lets an operator pause the cadence without
        // discarding their cron. Either gate off -> Disabled (manual still runs).
        match (self.env.backup_enabled, self.env.backup_cron.as_ref()) {
            (true, Some(schedule)) => Schedule::Cron(Box::new(schedule.clone())),
            _ => Schedule::Disabled,
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        let cluster = ctx.env.cluster.get().await?;
        let bg_name = bg::bg_name(&ctx.env.kubectl, &cluster.namespace).await?;
        let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let backup_name = format!("{}-{}.backup", bg_name, stamp);

        if ctx.dry_run {
            ctx.log_info(&format!(
                "[dry-run] would invoke battlegroup backup name={backup_name}"
            ))?;
            return Ok(TaskOutcome::Done);
        }

        ctx.log_info(&format!(
            "starting backup bg={bg_name} ns={} name={backup_name}",
            cluster.namespace
        ))?;
        // The stray-table precheck + dump-pod monitor live inside
        // run_backup_and_verify so every backup path (scheduled, manual,
        // pre-update) is guarded uniformly.
        run_backup_and_verify(ctx, &bg_name, &backup_name).await?;
        ctx.log_info(&format!(
            "backup complete path=/funcom/artifacts/database-dumps/{bg_name}/{backup_name}"
        ))?;
        Ok(TaskOutcome::Done)
    }
}

pub async fn run_backup_and_verify(ctx: &TaskCtx, bg_name: &str, backup_name: &str) -> Result<()> {
    let cluster = ctx.env.cluster.get().await?;
    let namespace = cluster.namespace.clone();

    // Precheck (#7): abort before the vendor spins up a dump pod if any public
    // table is owned by a different role — pg_dump would fail mid-run inside
    // that pod and leave only an opaque "backup failed". Guards every caller
    // (scheduled / manual / pre-update).
    check_table_ownership(ctx, &namespace).await?;

    // Real-time dump-pod monitor (#7): the vendor `battlegroup backup` runs the
    // dump in a separate pod and deletes it on exit, so a post-hoc diagnostics
    // query usually races the cleanup. Poll while the backup runs and keep the
    // latest snapshot (termination reason/exitCode + log tail) buffered.
    let active = Arc::new(AtomicBool::new(true));
    let diag_buffer: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let handle = {
        let kubectl = ctx.env.kubectl.clone();
        let ns = namespace.clone();
        let active = active.clone();
        let buffer = diag_buffer.clone();
        tokio::spawn(async move {
            while active.load(Ordering::Relaxed) {
                if let Some(diag) = dump_pod_diagnostics(&kubectl, &ns).await {
                    if let Ok(mut guard) = buffer.lock() {
                        *guard = Some(diag);
                    }
                }
                tokio::time::sleep(DUMP_POLL_INTERVAL).await;
            }
        })
    };
    let monitor_guard = MonitorGuard {
        active: active.clone(),
        handle,
    };

    let backup_result = ctx.env.bg_cli.backup(backup_name).await;
    // Stop the loop and tear the monitor down before we touch the buffer.
    active.store(false, Ordering::Relaxed);
    drop(monitor_guard);

    if let Err(err) = backup_result {
        // Prefer diagnostics captured live during the run; fall back to a
        // post-hoc query for the (rare) case where nothing was buffered before
        // the pod vanished.
        let buffered = diag_buffer.lock().ok().and_then(|guard| guard.clone());
        let diag = match buffered {
            Some(detail) => Some(detail),
            None => dump_pod_diagnostics(&ctx.env.kubectl, &namespace).await,
        };
        return match diag {
            Some(detail) => Err(err.context(detail)),
            None => Err(err),
        };
    }

    let backup_path = format!("/funcom/artifacts/database-dumps/{bg_name}/{backup_name}");
    let stat = run_process("sudo", &["-n", "stat", "-c", "%s", &backup_path], None, 30)
        .await
        .with_context(|| format!("checking backup output {backup_path}"))?;
    stat.require_ok(&format!("stat backup {backup_path}"))?;

    let size = stat.stdout.trim().parse::<u64>().unwrap_or(0);
    if size == 0 {
        return Err(anyhow!("backup output is empty: {backup_path}"));
    }
    ctx.log_info(&format!("backup verified path={backup_path} bytes={size}"))?;

    let spec_path = format!("{backup_path}.yaml");
    let spec = run_process("sudo", &["-n", "test", "-f", &spec_path], None, 30)
        .await
        .with_context(|| format!("checking backup companion spec {spec_path}"))?;
    if spec.ok() {
        ctx.log_info(&format!("backup companion spec present path={spec_path}"))?;
    } else {
        ctx.log_warn(&format!("backup companion spec missing path={spec_path}"))?;
    }

    Ok(())
}

/// Tables in the `public` schema not owned by the connection role. The vendor
/// dump pod runs `pg_dump` as this role, so a stray-owned table aborts the dump
/// partway with a permission error that never reaches the wrapper output.
const STRAY_TABLE_SQL: &str = "SELECT tablename, tableowner FROM pg_tables \
     WHERE schemaname = 'public' AND tableowner <> current_user \
     ORDER BY tablename";

/// Abort the backup if any `public` table is owned by a role other than the
/// connection user. Connecting/querying failures propagate (a backup that can't
/// be prechecked is not safe to start): the vendor dump targets the same DB and
/// would fail opaquely anyway, so surfacing the cause here is strictly clearer.
async fn check_table_ownership(ctx: &TaskCtx, namespace: &str) -> Result<()> {
    let state = ctx
        .env
        .pg
        .client(namespace)
        .await
        .context("connecting to postgres for backup table-ownership precheck")?;
    let rows = state
        .client()
        .query(STRAY_TABLE_SQL, &[])
        .await
        .context("querying public table ownership")?;
    if rows.is_empty() {
        return Ok(());
    }
    let stray: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
        .collect();
    let listed = format_stray_tables(&stray);
    ctx.log_error(&format!(
        "aborting backup: {} public table(s) not owned by the backup role; pg_dump would fail in the vendor dump pod: {listed}",
        stray.len()
    ))?;
    Err(anyhow!(
        "backup precheck failed: {} stray table(s) not owned by current_user: {listed}",
        stray.len()
    ))
}

/// Render `(table, owner)` pairs as a stable, comma-separated list for logs and
/// the error message. Split out so it can be unit-tested without a live DB.
fn format_stray_tables(tables: &[(String, String)]) -> String {
    tables
        .iter()
        .map(|(table, owner)| format!("{table} (owner={owner})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Best-effort diagnostics for a failed vendor backup (#7): summarize the most
/// recently-created dump pod's phase + container termination (reason/exitCode)
/// and append a short log tail. Returns the formatted detail, or None if no
/// dump pod or kubectl output is available. Never errors — diagnostics must not
/// mask the original backup failure.
async fn dump_pod_diagnostics(kubectl: &KubectlClient, namespace: &str) -> Option<String> {
    let pods = match kubectl
        .run(&["get", "pods", "-n", namespace, "-o", "json"])
        .await
    {
        Ok(out) if out.ok() => out,
        _ => return None,
    };
    let json: serde_json::Value = serde_json::from_str(&pods.stdout).ok()?;
    let (name, mut detail) = summarize_latest_dump_pod(&json)?;

    if let Ok(logs) = kubectl
        .run(&["logs", "-n", namespace, &name, "--tail=30"])
        .await
    {
        let tail = logs.stdout.trim();
        if !tail.is_empty() {
            detail.push_str("\n--- dump pod log tail ---\n");
            detail.push_str(tail);
        }
    }
    Some(detail)
}

/// Pure parse of `kubectl get pods -o json`: pick the most recently-created pod
/// whose name marks it as a dump pod (`-dump-`) and summarize its phase +
/// container termination. Returns `(pod_name, summary)`. Split out for testing.
fn summarize_latest_dump_pod(json: &serde_json::Value) -> Option<(String, String)> {
    let items = json.get("items")?.as_array()?;
    let mut latest_ts = "";
    let mut chosen: Option<&serde_json::Value> = None;
    let mut chosen_name = "";
    for pod in items {
        let name = pod
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !name.contains("-dump-") {
            continue;
        }
        let ts = pod
            .pointer("/metadata/creationTimestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if chosen.is_none() || ts > latest_ts {
            latest_ts = ts;
            chosen = Some(pod);
            chosen_name = name;
        }
    }
    let pod = chosen?;
    let phase = pod
        .pointer("/status/phase")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let mut detail = format!("latest dump pod {chosen_name} (phase={phase})");
    if let Some(statuses) = pod
        .pointer("/status/containerStatuses")
        .and_then(|v| v.as_array())
    {
        for cs in statuses {
            if let Some(term) = cs
                .pointer("/state/terminated")
                .or_else(|| cs.pointer("/lastState/terminated"))
            {
                let reason = term.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                let code = term.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                detail.push_str(&format!(" terminated(reason={reason}, exitCode={code})"));
            }
        }
    }
    Some((chosen_name.to_string(), detail))
}

#[cfg(test)]
mod tests {
    use super::{format_stray_tables, summarize_latest_dump_pod};

    #[test]
    fn formats_stray_tables_for_error() {
        let tables = vec![
            ("legacy_audit".to_string(), "postgres".to_string()),
            ("imported_blob".to_string(), "admin".to_string()),
        ];
        assert_eq!(
            format_stray_tables(&tables),
            "legacy_audit (owner=postgres), imported_blob (owner=admin)"
        );
    }

    #[test]
    fn formats_empty_stray_tables_as_blank() {
        assert_eq!(format_stray_tables(&[]), "");
    }

    #[test]
    fn picks_most_recent_dump_pod_and_surfaces_termination() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"items":[
              {"metadata":{"name":"bg-dump-20260609-pod","creationTimestamp":"2026-06-09T19:00:00Z"},
               "status":{"phase":"Failed","containerStatuses":[{"lastState":{"terminated":{"reason":"OOMKilled","exitCode":137}}}]}},
              {"metadata":{"name":"bg-dump-20260610-pod","creationTimestamp":"2026-06-10T03:00:00Z"},
               "status":{"phase":"Failed","containerStatuses":[{"state":{"terminated":{"reason":"Error","exitCode":1}}}]}},
              {"metadata":{"name":"bg-mq-game-sts-0","creationTimestamp":"2026-06-10T05:00:00Z"},
               "status":{"phase":"Running"}}
            ]}"#,
        )
        .unwrap();
        let (name, summary) = summarize_latest_dump_pod(&json).expect("a dump pod summary");
        // Most recent DUMP pod (03:00), not the newer non-dump mq pod.
        assert_eq!(name, "bg-dump-20260610-pod");
        assert!(summary.contains("exitCode=1"), "{summary}");
        assert!(summary.contains("Error"), "{summary}");
    }

    #[test]
    fn none_when_no_dump_pod_present() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"items":[{"metadata":{"name":"bg-mq-game-sts-0"},"status":{"phase":"Running"}}]}"#,
        )
        .unwrap();
        assert!(summarize_latest_dump_pod(&json).is_none());
    }
}
