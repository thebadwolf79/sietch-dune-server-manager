use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;

use crate::kubectl::battlegroup as bg;
use crate::kubectl::battlegroup_cli;
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::TaskTrigger;
use crate::tasks::TaskEnv;

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
        Ok(TaskOutcome::Done)
    }
}
