use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

pub mod admin_history;
pub mod pending;
pub mod runs;
pub mod welcome;

pub use admin_history::{AdminHistoryEntry, AdminHistoryFilter};
pub use pending::PendingUpdateRecord;
pub use runs::{LogEntry, LogLevel, NewLogEntry, TaskRun, TaskRunStatus, TaskTrigger};
pub use welcome::{
    WelcomeActionRecord, WelcomeActionStatus, WelcomeGrantRecord, WelcomeGrantStatus,
};

/// Shared store handle. Wraps a single SQLite connection behind a mutex so the
/// async layer can call into it from `spawn_blocking` closures.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating db parent dir {}", parent.display()))?;
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening sqlite at {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        migrate_schema(&conn)?;
        let orphaned_update_apply = count_running_update_apply(&conn)?;
        let orphaned = mark_orphaned_runs(&conn)?;
        if orphaned > 0 {
            tracing::warn!(orphaned, "marked orphaned running task_runs as failed");
        }
        if orphaned_update_apply > 0 {
            defer_pending_update_after_orphan(&conn, 5 * 60)?;
            tracing::warn!(
                orphaned = orphaned_update_apply,
                "deferred pending update after orphaned update-apply run"
            );
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    pub(crate) fn with_conn<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> rusqlite::Result<R>,
    {
        let guard = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("store mutex poisoned"))?;
        f(&guard).map_err(Into::into)
    }

    /// Read a config value from the `task_config` KV table.
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT value FROM task_config WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })
    }

    /// Upsert a config value.
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO task_config(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )
            .map(|_| ())
        })
    }

    /// Convenience for reading a value parseable as an integer.
    pub fn get_config_i64(&self, key: &str) -> Result<Option<i64>> {
        Ok(self
            .get_config(key)?
            .and_then(|raw| raw.parse::<i64>().ok()))
    }
}

fn migrate_schema(conn: &Connection) -> rusqlite::Result<()> {
    add_column_if_missing(
        conn,
        "welcome_grants",
        "first_online_at",
        "ALTER TABLE welcome_grants ADD COLUMN first_online_at TEXT",
    )?;
    migrate_welcome_ledger_account_id_key(conn)?;
    Ok(())
}

fn migrate_welcome_ledger_account_id_key(conn: &Connection) -> rusqlite::Result<()> {
    let grants_pk = primary_key_columns(conn, "welcome_grants")?;
    let actions_pk = primary_key_columns(conn, "welcome_grant_actions")?;
    if grants_pk == ["player_id", "package_version", "account_id"]
        && actions_pk == ["player_id", "package_version", "account_id", "action_index"]
    {
        return Ok(());
    }

    conn.execute_batch(
        "
PRAGMA foreign_keys=OFF;
ALTER TABLE welcome_grant_actions RENAME TO welcome_grant_actions_old;
ALTER TABLE welcome_grants RENAME TO welcome_grants_old;

CREATE TABLE welcome_grants (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    character_name TEXT,
    status TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    granted_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_online_status TEXT,
    first_online_at TEXT,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version, account_id)
);

CREATE TABLE welcome_grant_actions (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    action_index INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_at TEXT,
    confirmed_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    item_name TEXT,
    baseline_quantity INTEGER,
    expected_quantity INTEGER,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version, account_id, action_index),
    FOREIGN KEY (player_id, package_version, account_id)
        REFERENCES welcome_grants(player_id, package_version, account_id)
        ON DELETE CASCADE
);

INSERT OR IGNORE INTO welcome_grants (
    player_id, package_version, account_id, character_name, status,
    detected_at, updated_at, granted_at, attempts, last_online_status,
    first_online_at, last_error
)
SELECT
    player_id, package_version, account_id, character_name, status,
    detected_at, updated_at, granted_at, attempts, last_online_status,
    first_online_at, last_error
FROM welcome_grants_old;

INSERT OR IGNORE INTO welcome_grant_actions (
    player_id, package_version, account_id, action_index, action_type, status,
    created_at, updated_at, published_at, confirmed_at, attempts,
    item_name, baseline_quantity, expected_quantity, last_error
)
SELECT
    a.player_id, a.package_version, COALESCE(g.account_id, 0),
    a.action_index, a.action_type, a.status, a.created_at, a.updated_at,
    a.published_at, a.confirmed_at, a.attempts,
    a.item_name, a.baseline_quantity, a.expected_quantity, a.last_error
FROM welcome_grant_actions_old a
LEFT JOIN welcome_grants_old g
  ON g.player_id = a.player_id
 AND g.package_version = a.package_version;

DROP TABLE welcome_grant_actions_old;
DROP TABLE welcome_grants_old;
CREATE INDEX IF NOT EXISTS idx_welcome_grants_status ON welcome_grants(status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_welcome_grant_actions_status ON welcome_grant_actions(status, updated_at DESC);
PRAGMA foreign_keys=ON;
",
    )
}

fn primary_key_columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;
    let mut cols = Vec::new();
    for row in rows {
        let (name, pk) = row?;
        if pk > 0 {
            cols.push((pk, name));
        }
    }
    cols.sort_by_key(|(pk, _)| *pk);
    Ok(cols.into_iter().map(|(_, name)| name).collect())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    sql: &str,
) -> rusqlite::Result<()> {
    let exists = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut exists = false;
        for row in rows {
            if row? == column {
                exists = true;
                break;
            }
        }
        exists
    };
    if !exists {
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(err) if err.to_string().contains("duplicate column name") => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn count_running_update_apply(conn: &Connection) -> rusqlite::Result<usize> {
    conn.query_row(
        "SELECT count(*) FROM task_runs WHERE status = 'running' AND task_id = 'update-apply'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
}

fn mark_orphaned_runs(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE task_runs
         SET status = 'failed',
             finished_at = ?1,
             error = COALESCE(error, 'orphaned by daemon restart')
         WHERE status = 'running'",
        rusqlite::params![chrono::Utc::now().to_rfc3339()],
    )
}

fn defer_pending_update_after_orphan(
    conn: &Connection,
    delay_secs: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE pending_update SET due_ts = ?1 WHERE id = 1",
        rusqlite::params![chrono::Utc::now().timestamp() + delay_secs],
    )
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS task_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    trigger TEXT NOT NULL,
    dry_run INTEGER NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    duration_ms INTEGER,
    error TEXT
);

CREATE TABLE IF NOT EXISTS log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    task_id TEXT,
    run_id INTEGER,
    FOREIGN KEY (run_id) REFERENCES task_runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    command TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    ok INTEGER NOT NULL,
    message TEXT
);

CREATE TABLE IF NOT EXISTS pending_update (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    battlegroup TEXT NOT NULL,
    namespace TEXT NOT NULL,
    latest_steam_build TEXT,
    local_steam_build TEXT,
    live_version TEXT,
    downloaded_version TEXT NOT NULL,
    due_ts INTEGER NOT NULL,
    created_ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS task_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS welcome_grants (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    character_name TEXT,
    status TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    granted_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_online_status TEXT,
    first_online_at TEXT,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version, account_id)
);

CREATE TABLE IF NOT EXISTS welcome_grant_actions (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    action_index INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_at TEXT,
    confirmed_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    item_name TEXT,
    baseline_quantity INTEGER,
    expected_quantity INTEGER,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version, account_id, action_index),
    FOREIGN KEY (player_id, package_version, account_id)
        REFERENCES welcome_grants(player_id, package_version, account_id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_runs_started_at ON task_runs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_runs_task_id ON task_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_log_entries_created_at ON log_entries(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_log_entries_run_id ON log_entries(run_id);
CREATE INDEX IF NOT EXISTS idx_admin_commands_created_at ON admin_commands(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_welcome_grants_status ON welcome_grants(status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_welcome_grant_actions_status ON welcome_grant_actions(status, updated_at DESC);
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_and_pragmas() {
        let dir = tempdir();
        let path = dir.join("s.sqlite");
        let store = Store::open(&path).unwrap();
        store
            .with_conn(|c| {
                let mode: String = c.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
                assert_eq!(mode.to_lowercase(), "wal");
                let count: i64 = c.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
                     ('task_runs','log_entries','admin_commands','pending_update','welcome_grants','welcome_grant_actions')",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 6);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn open_migrates_welcome_ledger_to_account_scoped_key() {
        let dir = tempdir();
        let path = dir.join("s.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "
CREATE TABLE welcome_grants (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    account_id INTEGER NOT NULL,
    character_name TEXT,
    status TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    granted_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_online_status TEXT,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version)
);
CREATE TABLE welcome_grant_actions (
    player_id TEXT NOT NULL,
    package_version TEXT NOT NULL,
    action_index INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_at TEXT,
    confirmed_at TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    item_name TEXT,
    baseline_quantity INTEGER,
    expected_quantity INTEGER,
    last_error TEXT,
    PRIMARY KEY (player_id, package_version, action_index)
);
INSERT INTO welcome_grants (
    player_id, package_version, account_id, character_name, status,
    detected_at, updated_at
) VALUES ('P1', 'v1', 10, 'Chani', 'granted', 'now', 'now');
INSERT INTO welcome_grant_actions (
    player_id, package_version, action_index, action_type, status,
    created_at, updated_at, published_at
) VALUES ('P1', 'v1', -1, 'welcome_message', 'published', 'now', 'now', 'now');
",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        store
            .with_conn(|conn| {
                assert_eq!(
                    primary_key_columns(conn, "welcome_grants")?,
                    ["player_id", "package_version", "account_id"]
                );
                assert_eq!(
                    primary_key_columns(conn, "welcome_grant_actions")?,
                    ["player_id", "package_version", "account_id", "action_index"]
                );
                Ok(())
            })
            .unwrap();
        let existing = store
            .ensure_welcome_action("P1", "v1", 10, -1, "welcome_message")
            .unwrap();
        assert_eq!(existing.status, WelcomeActionStatus::Published);

        store
            .ensure_welcome_grant("P1", "v1", 11, Some("Paul"), "Online")
            .unwrap();
        let fresh = store
            .ensure_welcome_action("P1", "v1", 11, -1, "welcome_message")
            .unwrap();
        assert_eq!(fresh.status, WelcomeActionStatus::Pending);
    }

    pub(crate) fn tempdir() -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("dune-sms-test-{}", uuid()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn uuid() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{nanos:x}")
    }
}
