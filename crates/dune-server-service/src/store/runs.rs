use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskRunStatus {
    Running,
    Success,
    Failed,
    Skipped,
}

impl TaskRunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskTrigger {
    Scheduled,
    Manual,
    Startup,
}

impl TaskTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Manual => "manual",
            Self::Startup => "startup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRun {
    pub id: i64,
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub trigger: TaskTrigger,
    #[serde(rename = "dryRun")]
    pub dry_run: bool,
    pub status: TaskRunStatus,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub level: LogLevel,
    pub message: String,
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    #[serde(rename = "runId")]
    pub run_id: Option<i64>,
}

pub struct NewLogEntry<'a> {
    pub level: LogLevel,
    pub message: &'a str,
    pub task_id: Option<&'a str>,
    pub run_id: Option<i64>,
}

impl Store {
    pub fn start_run(&self, task_id: &str, trigger: TaskTrigger, dry_run: bool) -> Result<i64> {
        let started_at = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO task_runs (task_id, trigger, dry_run, status, started_at)
                 VALUES (?1, ?2, ?3, 'running', ?4)",
                params![task_id, trigger.as_str(), dry_run as i32, started_at],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn finish_run(
        &self,
        run_id: i64,
        status: TaskRunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let finished_at = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "UPDATE task_runs
                 SET status = ?1,
                     finished_at = ?2,
                     duration_ms = CAST((julianday(?2) - julianday(started_at)) * 86400000 AS INTEGER),
                     error = ?3
                 WHERE id = ?4",
                params![status.as_str(), finished_at, error, run_id],
            )?;
            Ok(())
        })
    }

    pub fn delete_run(&self, run_id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM log_entries WHERE run_id = ?1", params![run_id])?;
            c.execute("DELETE FROM task_runs WHERE id = ?1", params![run_id])?;
            Ok(())
        })
    }

    pub fn log(&self, entry: &NewLogEntry<'_>) -> Result<()> {
        let created_at = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO log_entries (created_at, level, message, task_id, run_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    created_at,
                    entry.level.as_str(),
                    entry.message,
                    entry.task_id,
                    entry.run_id
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_runs(&self, limit: u32, task_id: Option<&str>) -> Result<Vec<TaskRun>> {
        let limit = limit.clamp(1, 500) as i64;
        self.with_conn(|c| match task_id {
            Some(tid) => {
                let mut stmt = c.prepare(
                    "SELECT id, task_id, trigger, dry_run, status, started_at, finished_at, \
                     duration_ms, error
                     FROM task_runs WHERE task_id = ?1
                     ORDER BY started_at DESC LIMIT ?2",
                )?;
                let rows = stmt
                    .query_map(params![tid, limit], map_run)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            }
            None => {
                let mut stmt = c.prepare(
                    "SELECT id, task_id, trigger, dry_run, status, started_at, finished_at, \
                     duration_ms, error
                     FROM task_runs ORDER BY started_at DESC LIMIT ?1",
                )?;
                let rows = stmt
                    .query_map(params![limit], map_run)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            }
        })
    }

    pub fn list_logs(&self, limit: u32, run_id: Option<i64>) -> Result<Vec<LogEntry>> {
        let limit = limit.clamp(1, 2000) as i64;
        self.with_conn(|c| match run_id {
            Some(rid) => {
                let mut stmt = c.prepare(
                    "SELECT id, created_at, level, message, task_id, run_id
                     FROM log_entries WHERE run_id = ?1
                     ORDER BY created_at ASC LIMIT ?2",
                )?;
                let rows = stmt
                    .query_map(params![rid, limit], map_log)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            }
            None => {
                // Take the N most recent rows but return them oldest-first so
                // the client can scroll down through chronological order.
                let mut stmt = c.prepare(
                    "SELECT id, created_at, level, message, task_id, run_id FROM (\
                       SELECT * FROM log_entries ORDER BY created_at DESC LIMIT ?1\
                     ) sub ORDER BY created_at ASC",
                )?;
                let rows = stmt
                    .query_map(params![limit], map_log)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            }
        })
    }

    pub fn count_runs_by_status(&self) -> Result<(i64, i64, i64, i64)> {
        self.with_conn(|c| {
            let total: i64 = c.query_row("SELECT count(*) FROM task_runs", [], |r| r.get(0))?;
            let succeeded: i64 = c.query_row(
                "SELECT count(*) FROM task_runs WHERE status='success'",
                [],
                |r| r.get(0),
            )?;
            let failed: i64 = c.query_row(
                "SELECT count(*) FROM task_runs WHERE status='failed'",
                [],
                |r| r.get(0),
            )?;
            let running: i64 = c.query_row(
                "SELECT count(*) FROM task_runs WHERE status='running'",
                [],
                |r| r.get(0),
            )?;
            Ok((total, succeeded, failed, running))
        })
    }

    pub fn get_run(&self, run_id: i64) -> Result<Option<TaskRun>> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT id, task_id, trigger, dry_run, status, started_at, finished_at, \
                 duration_ms, error
                 FROM task_runs WHERE id = ?1",
                params![run_id],
                map_run,
            )
            .optional()
        })
    }
}

fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRun> {
    let trigger_raw: String = row.get(2)?;
    let status_raw: String = row.get(4)?;
    let dry_run_raw: i64 = row.get(3)?;
    Ok(TaskRun {
        id: row.get(0)?,
        task_id: row.get(1)?,
        trigger: match trigger_raw.as_str() {
            "scheduled" => TaskTrigger::Scheduled,
            "startup" => TaskTrigger::Startup,
            _ => TaskTrigger::Manual,
        },
        dry_run: dry_run_raw != 0,
        status: TaskRunStatus::from_str(&status_raw),
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        duration_ms: row.get(7)?,
        error: row.get(8)?,
    })
}

fn map_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<LogEntry> {
    let level_raw: String = row.get(2)?;
    Ok(LogEntry {
        id: row.get(0)?,
        created_at: row.get(1)?,
        level: LogLevel::from_str(&level_raw),
        message: row.get(3)?,
        task_id: row.get(4)?,
        run_id: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::tempdir;
    use crate::store::Store;

    fn open_store() -> Store {
        let dir = tempdir();
        Store::open(&dir.join("s.sqlite")).unwrap()
    }

    #[test]
    fn run_lifecycle_and_logging_roundtrip() {
        let s = open_store();
        let id = s
            .start_run("backup", TaskTrigger::Scheduled, false)
            .unwrap();
        s.log(&NewLogEntry {
            level: LogLevel::Info,
            message: "starting",
            task_id: Some("backup"),
            run_id: Some(id),
        })
        .unwrap();
        s.finish_run(id, TaskRunStatus::Success, None).unwrap();

        let runs = s.list_runs(10, None).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, id);
        assert_eq!(runs[0].status, TaskRunStatus::Success);
        assert!(runs[0].duration_ms.is_some());

        let logs = s.list_logs(10, Some(id)).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "starting");
    }

    #[test]
    fn delete_run_cascades_logs() {
        let s = open_store();
        let id = s.start_run("backup", TaskTrigger::Manual, true).unwrap();
        s.log(&NewLogEntry {
            level: LogLevel::Info,
            message: "noop",
            task_id: Some("backup"),
            run_id: Some(id),
        })
        .unwrap();
        s.delete_run(id).unwrap();
        assert!(s.list_runs(10, None).unwrap().is_empty());
        assert!(s.list_logs(10, Some(id)).unwrap().is_empty());
    }

    #[test]
    fn counts_track_status_transitions() {
        let s = open_store();
        let r1 = s.start_run("a", TaskTrigger::Scheduled, false).unwrap();
        let r2 = s.start_run("b", TaskTrigger::Scheduled, false).unwrap();
        s.finish_run(r1, TaskRunStatus::Success, None).unwrap();
        s.finish_run(r2, TaskRunStatus::Failed, Some("boom"))
            .unwrap();
        let (total, succ, fail, running) = s.count_runs_by_status().unwrap();
        assert_eq!((total, succ, fail, running), (2, 1, 1, 0));
    }
}
