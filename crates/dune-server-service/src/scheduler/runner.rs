use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::store::{NewLogEntry, Store, TaskRunStatus, TaskTrigger};
use crate::tasks::TaskEnv;

use super::task::{Task, TaskCtx, TaskOutcome};

/// Coordinates task execution: assigns run IDs, persists status transitions,
/// and enforces the single-instance-per-task overlap guard.
pub struct TaskRunner {
    store: Store,
    env: Arc<TaskEnv>,
    running: Mutex<HashSet<&'static str>>,
}

impl TaskRunner {
    pub fn new(store: Store, env: Arc<TaskEnv>) -> Self {
        Self {
            store,
            env,
            running: Mutex::new(HashSet::new()),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn env(&self) -> &Arc<TaskEnv> {
        &self.env
    }

    /// Execute `task`, recording start/finish/error in the store and routing
    /// `Noop` outcomes to a row delete so the history stays clean.
    pub async fn run(
        self: &Arc<Self>,
        task: Arc<dyn Task>,
        trigger: TaskTrigger,
        dry_run: bool,
        options: Option<Value>,
    ) -> Result<TaskOutcome> {
        let id = task.id();

        let already_running = {
            let mut guard = self.running.lock().await;
            if guard.contains(id) {
                true
            } else {
                guard.insert(id);
                false
            }
        };

        if already_running {
            let run_id = self.store.start_run(id, trigger, dry_run)?;
            self.store.log(&NewLogEntry {
                level: crate::store::LogLevel::Warn,
                message: &format!("{id} is still running; skipping overlapping run."),
                task_id: Some(id),
                run_id: Some(run_id),
            })?;
            self.store
                .finish_run(run_id, TaskRunStatus::Skipped, Some("overlap"))?;
            tracing::warn!(task = id, "overlap; skipping");
            return Ok(TaskOutcome::Done);
        }

        let run_id = self.store.start_run(id, trigger, dry_run)?;
        let ctx = TaskCtx {
            run_id,
            dry_run,
            trigger,
            store: self.store.clone(),
            env: self.env.clone(),
            options,
        };
        ctx.log_info(&format!("Starting task {id}."))?;

        let result = task.run(&ctx).await;

        {
            let mut guard = self.running.lock().await;
            guard.remove(id);
        }

        match result {
            Ok(TaskOutcome::Done) => {
                ctx.log_info(&format!("Finished task {id}."))?;
                self.store
                    .finish_run(run_id, TaskRunStatus::Success, None)?;
                Ok(TaskOutcome::Done)
            }
            Ok(TaskOutcome::Noop) => {
                self.store.delete_run(run_id)?;
                Ok(TaskOutcome::Noop)
            }
            Err(err) => {
                let msg = crate::logger::redact(&format!("{err:#}")).into_owned();
                ctx.log_error(&format!("{id} failed: {msg}"))?;
                self.store
                    .finish_run(run_id, TaskRunStatus::Failed, Some(&msg))?;
                Err(err)
            }
        }
    }
}
