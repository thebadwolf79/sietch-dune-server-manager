use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::kubectl::battlegroup as bg;
use crate::kubectl::steam;
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::PendingUpdateRecord;

/// Replaces `scripts/cron-battlegroup-update-check`. Polls Steam for the
/// public-branch buildid, compares it to the locally downloaded build, and on
/// a real delta writes a `pending_update` row. `UpdateApplyTask` later invokes
/// the vendor `battlegroup update` flow, which owns download/apply/restart.
pub struct UpdateCheckTask;

#[async_trait]
impl Task for UpdateCheckTask {
    fn id(&self) -> &'static str {
        "update-check"
    }

    fn schedule(&self) -> Schedule {
        Schedule::interval_secs(15 * 60)
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        // If a pending update already exists, nothing to do.
        if ctx.store.load_pending_update()?.is_some() {
            ctx.log_info("pending update already scheduled; skipping check")?;
            return Ok(TaskOutcome::Noop);
        }

        let cluster = ctx.env.cluster.get().await?;
        let bg_name = bg::bg_name(&ctx.env.kubectl, &cluster.namespace).await?;
        let bg_doc = bg::bg_json(&ctx.env.kubectl, &cluster.namespace, &bg_name).await?;
        let live_version = steam::extract_live_version(&bg_doc);

        let latest = ctx.env.steamcmd.latest_public_build().await?;
        let local = ctx.env.steamcmd.local_build().await?;

        ctx.log_info(&format!(
            "update check latest_build={} local_build={} live_version={}",
            latest.buildid,
            local.as_deref().unwrap_or("unknown"),
            live_version.as_deref().unwrap_or("unknown"),
        ))?;

        if let Some(local_build) = local.as_deref() {
            if local_build == latest.buildid {
                ctx.log_info("no Steam update available")?;
                return Ok(TaskOutcome::Noop);
            }
        }

        if ctx.dry_run {
            ctx.log_info("[dry-run] would schedule vendor battlegroup update")?;
            return Ok(TaskOutcome::Done);
        }

        let lead_secs = ctx.env.update_lead_secs.max(0);
        let due_ts = Utc::now().timestamp() + lead_secs;
        ctx.store.upsert_pending_update(&PendingUpdateRecord {
            battlegroup: bg_name.clone(),
            namespace: cluster.namespace.clone(),
            latest_steam_build: Some(latest.buildid.clone()),
            local_steam_build: local,
            live_version,
            downloaded_version: latest.buildid.clone(),
            due_ts,
            created_ts: 0,
        })?;
        ctx.log_info(&format!(
            "scheduled update bg={bg_name} latest_steam_build={} due_ts={due_ts}",
            latest.buildid
        ))?;

        if let Err(err) = ctx
            .env
            .mq
            .publish_service_broadcast(
                "Server update",
                &format!(
                    "A server update is ready and will be applied in {}. The server will restart.",
                    human_duration(lead_secs)
                ),
                60,
            )
            .await
        {
            ctx.log_warn(&format!("warning broadcast failed: {err:#}"))?;
        }
        Ok(TaskOutcome::Done)
    }
}

fn human_duration(seconds: i64) -> String {
    if seconds == 0 {
        return "less than a minute".to_string();
    }
    if seconds % 3600 == 0 {
        let hours = seconds / 3600;
        return format!("{hours} {}", if hours == 1 { "hour" } else { "hours" });
    }
    if seconds % 60 == 0 {
        let minutes = seconds / 60;
        return format!(
            "{minutes} {}",
            if minutes == 1 { "minute" } else { "minutes" }
        );
    }
    format!("{seconds} seconds")
}
