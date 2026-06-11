use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::kubectl::battlegroup as bg;
use crate::kubectl::battlegroup_cli;
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::TaskTrigger;
use crate::tasks::TaskEnv;

/// Sqlite config key recording the epoch-seconds of the last successful
/// restart (manual or scheduled).
const LAST_RESTART_KEY: &str = "last_restart_time";

/// Minimum gap between scheduled restarts. A scheduled fire landing inside this
/// window of the last successful restart is skipped — guards against duplicate
/// fires from clock drift, NTP steps, or DST fall-back replaying the wall-clock.
const RESTART_COOLDOWN_SECS: i64 = 15 * 60;

/// Whether a scheduled restart should be skipped because the last one is too
/// recent. A non-positive elapsed (clock moved backward / bogus future stamp)
/// never skips, so a bad stored value can't wedge restarts permanently.
fn within_restart_cooldown(last_restart: Option<i64>, now: i64) -> bool {
    match last_restart {
        Some(last) => {
            let elapsed = now - last;
            elapsed >= 0 && elapsed < RESTART_COOLDOWN_SECS
        }
        None => false,
    }
}

/// Replaces `scripts/daily-battlegroup-restart`. Delegates the restart itself
/// to the vendor `battlegroup restart` helper, then waits for full readiness.
/// Schedule fires at the configured wall-clock hour:minute in the IANA timezone
/// supplied by `TaskEnv` (default 05:00 Europe/Amsterdam).
pub struct RestartTask {
    env: Arc<TaskEnv>,
}

impl RestartTask {
    pub fn new(env: Arc<TaskEnv>) -> Self {
        Self { env }
    }
}

/// The restart cadence, independent of the `restart_enabled` switch: the
/// operator's cron expression if configured, otherwise the daily wall-clock
/// fallback. Shared with `restart-notice` so the warning fires relative to the
/// real restart time on whichever schedule is active.
pub(crate) fn restart_cadence(env: &TaskEnv) -> Schedule {
    match env.restart_cron.as_ref() {
        Some(cron) => Schedule::Cron(Box::new(cron.clone())),
        None => Schedule::daily(env.restart_hour, env.restart_minute),
    }
}

#[async_trait]
impl Task for RestartTask {
    fn id(&self) -> &'static str {
        "restart"
    }

    fn schedule(&self) -> Schedule {
        if self.env.restart_enabled {
            restart_cadence(&self.env)
        } else {
            Schedule::Disabled
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        let cluster = ctx.env.cluster.get().await?;
        let bg_name = bg::bg_name(&ctx.env.kubectl, &cluster.namespace).await?;
        if ctx.trigger == TaskTrigger::Scheduled {
            let stop_value = bg::bg_field(
                &ctx.env.kubectl,
                &cluster.namespace,
                &bg_name,
                "{.spec.stop}",
            )
            .await
            .unwrap_or_default();
            if stop_value == "true" {
                ctx.log_info(&format!(
                    "battlegroup bg={bg_name} is stopped; skipping scheduled restart"
                ))?;
                return Ok(TaskOutcome::Noop);
            }

            // Cooldown lock (scheduled only): an operator's manual restart is
            // always honored, but a scheduled fire that lands within the
            // cooldown of the last successful restart is a duplicate (clock
            // drift / NTP / DST) and is skipped.
            let last_restart = ctx.store.get_config_i64(LAST_RESTART_KEY)?;
            if within_restart_cooldown(last_restart, Utc::now().timestamp()) {
                let elapsed = Utc::now().timestamp() - last_restart.unwrap_or_default();
                ctx.log_warn(&format!(
                    "skipping scheduled restart for bg={bg_name}: last restart was {elapsed}s ago (cooldown {RESTART_COOLDOWN_SECS}s)"
                ))?;
                return Ok(TaskOutcome::Noop);
            }
        }

        ctx.log_info(&format!(
            "restarting battlegroup bg={bg_name} ns={}",
            cluster.namespace
        ))?;

        if ctx.dry_run {
            ctx.log_info("[dry-run] would invoke battlegroup restart")?;
            return Ok(TaskOutcome::Done);
        }

        ctx.env.bg_cli.restart().await?;
        let summary = battlegroup_cli::wait_until_running(
            &ctx.env.kubectl,
            &cluster.namespace,
            &bg_name,
            Duration::from_secs(1200),
        )
        .await?;
        ctx.log_info(&format!(
            "battlegroup restart complete phase={} serverGroupPhase={} ready={}/{}",
            summary.phase, summary.server_group_phase, summary.ready, summary.size
        ))?;

        // Record the completion time so the scheduled-restart cooldown can skip
        // a near-duplicate fire. Best-effort: a write failure must not turn a
        // successful restart into a task failure.
        if let Err(err) = ctx
            .store
            .set_config(LAST_RESTART_KEY, &Utc::now().timestamp().to_string())
        {
            ctx.log_warn(&format!("could not record {LAST_RESTART_KEY}: {err}"))?;
        }

        Ok(TaskOutcome::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::{within_restart_cooldown, RESTART_COOLDOWN_SECS};

    #[test]
    fn no_prior_restart_never_skips() {
        assert!(!within_restart_cooldown(None, 1_000_000));
    }

    #[test]
    fn skips_inside_cooldown_window() {
        let now = 1_000_000;
        assert!(within_restart_cooldown(Some(now - 60), now));
        assert!(within_restart_cooldown(Some(now - (RESTART_COOLDOWN_SECS - 1)), now));
    }

    #[test]
    fn allows_at_or_after_cooldown_boundary() {
        let now = 1_000_000;
        assert!(!within_restart_cooldown(Some(now - RESTART_COOLDOWN_SECS), now));
        assert!(!within_restart_cooldown(Some(now - 4 * RESTART_COOLDOWN_SECS), now));
    }

    #[test]
    fn future_timestamp_does_not_wedge_restarts() {
        let now = 1_000_000;
        // Clock moved backward or a bogus future stamp: elapsed < 0 -> allow.
        assert!(!within_restart_cooldown(Some(now + 5_000), now));
    }
}
