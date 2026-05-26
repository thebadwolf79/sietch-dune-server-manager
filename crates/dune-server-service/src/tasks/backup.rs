use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::Utc;

use crate::kubectl::battlegroup as bg;
use crate::kubectl::run_process;
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};

/// Replaces `scripts/cron-battlegroup-backup`. Runs the vendor backup helper,
/// emits a per-run log line referencing the dump path, and lets the operator
/// handle stale dump cleanup out-of-band (we do not invoke `sudo find -delete`
/// from the daemon — too easy to widen the blast radius).
pub struct BackupTask;

#[async_trait]
impl Task for BackupTask {
    fn id(&self) -> &'static str {
        "backup"
    }

    fn schedule(&self) -> Schedule {
        Schedule::interval_secs(2 * 60 * 60)
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
        run_backup_and_verify(ctx, &bg_name, &backup_name).await?;
        ctx.log_info(&format!(
            "backup complete path=/funcom/artifacts/database-dumps/{bg_name}/{backup_name}"
        ))?;
        Ok(TaskOutcome::Done)
    }
}

pub async fn run_backup_and_verify(ctx: &TaskCtx, bg_name: &str, backup_name: &str) -> Result<()> {
    ctx.env.bg_cli.backup(backup_name).await?;

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
