use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;

use crate::admin::ShutdownType;
use crate::scheduler::{schedule::Schedule, Task, TaskCtx, TaskOutcome};
use crate::tasks::{restart, TaskEnv};

/// Replaces `scripts/daily-battlegroup-restart-notice`. Scheduled daily at the
/// configured wall-clock hour:minute. Computes the target timestamp for the
/// actual restart and publishes a single ServerShutdown broadcast — the server
/// uses the frequency/duration fields to render its own repeating countdown.
///
/// When triggered manually via `POST /api/runs/trigger` with an `options` body
/// of shape `{ leadSecs, frequencySecs, durationSecs, title, body }`, the
/// target timestamp is computed as `now + leadSecs` and the operator-supplied
/// frequency / duration override the scheduler-wide defaults from
/// `TaskEnv::restart_warning_*`. If both `title` and `body` are present, an
/// additional Generic broadcast carrying them is fired before the
/// ServerShutdown notice so the in-game UI shows the operator's message.
pub struct RestartNoticeTask {
    env: Arc<TaskEnv>,
}

impl RestartNoticeTask {
    pub fn new(env: Arc<TaskEnv>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl Task for RestartNoticeTask {
    fn id(&self) -> &'static str {
        "restart-notice"
    }

    fn schedule(&self) -> Schedule {
        // Tied to the daily restart switch: if auto-restart is off, the
        // scheduled countdown would warn about a restart that never fires.
        // Manual triggers still work regardless.
        if !self.env.restart_enabled {
            return Schedule::Disabled;
        }
        // Fire `restart_warning_duration_secs` before the actual restart
        // moment, tracking whichever cadence (cron or daily) the restart task
        // uses. The RestartNotice variant computes the lead-time offset against
        // the inner schedule so day-specific / multi-time crons stay in sync.
        Schedule::RestartNotice {
            inner: Box::new(restart::restart_cadence(&self.env)),
            lead_time: Duration::from_secs(self.env.restart_warning_duration_secs),
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        let opts = ctx.options.as_ref().and_then(|v| v.as_object());

        let lead_secs = opts
            .and_then(|o| o.get("leadSecs"))
            .and_then(|v| v.as_i64());
        let frequency_override = opts
            .and_then(|o| o.get("frequencySecs"))
            .and_then(|v| v.as_u64());
        let duration_override = opts
            .and_then(|o| o.get("durationSecs"))
            .and_then(|v| v.as_u64());
        let title = opts
            .and_then(|o| o.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let body = opts
            .and_then(|o| o.get("body"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        if matches!(lead_secs, Some(secs) if secs < 0) {
            return Err(anyhow!("leadSecs must be >= 0"));
        }
        if matches!(frequency_override, Some(0)) {
            return Err(anyhow!("frequencySecs must be greater than 0"));
        }
        if matches!(duration_override, Some(0)) {
            return Err(anyhow!("durationSecs must be greater than 0"));
        }

        let target_utc = match lead_secs {
            Some(secs) => Utc::now() + chrono::Duration::seconds(secs),
            // Scheduled: the real restart is the next fire of the restart
            // cadence after now. Since this task fires `lead_time` before that
            // moment, the next inner fire is exactly the restart we are warning
            // about — works for both the daily fallback and any cron schedule.
            None => restart::restart_cadence(&ctx.env)
                .next_fire(ctx.env.restart_tz, Utc::now()),
        };
        let target_ts = target_utc.timestamp();
        let frequency = frequency_override.unwrap_or(ctx.env.restart_warning_frequency_secs);
        let duration = duration_override.unwrap_or(ctx.env.restart_warning_duration_secs);

        ctx.log_info(&format!(
            "scheduling restart warning target_ts={target_ts} frequency={frequency}s duration={duration}s tz={} (source={})",
            ctx.env.restart_tz.name(),
            if lead_secs.is_some() { "manual" } else { "scheduled" },
        ))?;

        if ctx.dry_run {
            ctx.log_info("[dry-run] would publish ServerShutdown broadcast")?;
            if title.is_some() && body.is_some() {
                ctx.log_info(
                    "[dry-run] would also publish Generic broadcast with custom title/body",
                )?;
            }
            return Ok(TaskOutcome::Done);
        }

        if let (Some(t), Some(b)) = (title, body) {
            // Operator opted in to a custom in-game banner; fire it for the same
            // wall-clock duration as the countdown so it stays visible.
            let banner = ctx.env.mq.publish_service_broadcast(t, b, duration).await?;
            ctx.log_info(&format!(
                "custom broadcast ok={} output={}",
                banner.ok,
                banner.output.trim()
            ))?;
        }

        // The shared service-broadcast base payload accepts BroadcastDuration
        // for the on-screen pulse length. Match the manual Admin form's
        // default so scheduled + manual paths produce the same shape.
        let broadcast_duration = 30u64;
        let result = ctx
            .env
            .mq
            .publish_server_shutdown(
                ShutdownType::Restart,
                target_ts,
                frequency,
                duration,
                broadcast_duration,
            )
            .await?;
        ctx.log_info(&format!(
            "publish ok={} output={}",
            result.ok,
            result.output.trim()
        ))?;
        Ok(TaskOutcome::Done)
    }
}
