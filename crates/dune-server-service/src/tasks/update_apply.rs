use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::kubectl::{battlegroup as bg, steam};
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::PendingUpdateRecord;

const UPDATE_RETRY_DELAY_SECS: i64 = 15 * 60;

/// Replaces `scripts/apply-pending-battlegroup-update`. Every minute, checks
/// whether the `pending_update` row's `due_ts` has arrived; when it has,
/// broadcasts, runs a pre-update backup, then delegates the full
/// check/download/apply/restart flow to the vendor `battlegroup update`.
pub struct UpdateApplyTask;

#[async_trait]
impl Task for UpdateApplyTask {
    fn id(&self) -> &'static str {
        "update-apply"
    }

    fn schedule(&self) -> Schedule {
        Schedule::interval_secs(60)
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        let Some(pending) = ctx.store.load_pending_update()? else {
            return Ok(TaskOutcome::Noop);
        };
        if Utc::now().timestamp() < pending.due_ts {
            return Ok(TaskOutcome::Noop);
        }

        let result = apply_due_update(ctx, pending).await;
        if let Err(err) = &result {
            if !ctx.dry_run {
                let _ = ctx.store.defer_pending_update(UPDATE_RETRY_DELAY_SECS);
                let _ = ctx.log_warn(&format!(
                    "deferred pending update retry by {} seconds after failure: {err:#}",
                    UPDATE_RETRY_DELAY_SECS
                ));
            }
        }
        result
    }
}

async fn apply_due_update(ctx: &TaskCtx, pending: PendingUpdateRecord) -> Result<TaskOutcome> {
    ctx.log_info(&format!(
        "applying pending update bg={} latest_steam_build={} live={}",
        pending.battlegroup,
        pending.downloaded_version,
        pending.live_version.as_deref().unwrap_or("unknown")
    ))?;

    let bg_doc = bg::bg_json(&ctx.env.kubectl, &pending.namespace, &pending.battlegroup).await?;
    let current_live_version = steam::extract_live_version(&bg_doc);
    let downloaded_version = ctx.env.steamcmd.downloaded_version().await?;
    if let (Some(live), Some(downloaded)) = (
        current_live_version.as_deref(),
        downloaded_version.as_deref(),
    ) {
        if live == downloaded {
            ctx.log_info(&format!(
                    "pending update already applied to live BattleGroup version {downloaded}; clearing pending update"
                ))?;
            if !ctx.dry_run {
                ctx.store.clear_pending_update()?;
            }
            return Ok(TaskOutcome::Done);
        }
    }

    if ctx.dry_run {
        ctx.log_info("[dry-run] would run pre-update backup and vendor battlegroup update")?;
        return Ok(TaskOutcome::Done);
    }

    if let Err(err) = ctx
        .env
        .mq
        .publish_service_broadcast(
            "Server update",
            "Server update is starting now. The server will restart.",
            60,
        )
        .await
    {
        ctx.log_warn(&format!("pre-update broadcast failed: {err:#}"))?;
    }

    ctx.log_info("taking pre-update database backup")?;
    backup_one(ctx, &pending.battlegroup).await?;

    ctx.log_info("running vendor battlegroup update")?;
    ctx.env.bg_cli.update().await?;
    ctx.log_info("vendor battlegroup update completed")?;
    ctx.store.clear_pending_update()?;

    if let Err(err) = ctx
        .env
        .mq
        .publish_service_broadcast(
            "Server update",
            "Server update is complete and the server is back online.",
            60,
        )
        .await
    {
        ctx.log_warn(&format!("post-update broadcast failed: {err:#}"))?;
    }
    Ok(TaskOutcome::Done)
}

async fn backup_one(ctx: &TaskCtx, bg_name: &str) -> Result<()> {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_name = format!("{}-pre-update-{}.backup", bg_name, stamp);
    crate::tasks::backup::run_backup_and_verify(ctx, bg_name, &backup_name).await
}
