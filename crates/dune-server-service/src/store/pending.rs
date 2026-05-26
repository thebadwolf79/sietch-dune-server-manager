use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdateRecord {
    pub battlegroup: String,
    pub namespace: String,
    pub latest_steam_build: Option<String>,
    pub local_steam_build: Option<String>,
    pub live_version: Option<String>,
    pub downloaded_version: String,
    /// Unix epoch seconds.
    pub due_ts: i64,
    /// Unix epoch seconds.
    pub created_ts: i64,
}

impl Store {
    pub fn load_pending_update(&self) -> Result<Option<PendingUpdateRecord>> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT battlegroup, namespace, latest_steam_build, local_steam_build, \
                 live_version, downloaded_version, due_ts, created_ts
                 FROM pending_update WHERE id = 1",
                [],
                |row| {
                    Ok(PendingUpdateRecord {
                        battlegroup: row.get(0)?,
                        namespace: row.get(1)?,
                        latest_steam_build: row.get(2)?,
                        local_steam_build: row.get(3)?,
                        live_version: row.get(4)?,
                        downloaded_version: row.get(5)?,
                        due_ts: row.get(6)?,
                        created_ts: row.get(7)?,
                    })
                },
            )
            .optional()
        })
    }

    pub fn upsert_pending_update(&self, record: &PendingUpdateRecord) -> Result<()> {
        let created_ts = if record.created_ts == 0 {
            Utc::now().timestamp()
        } else {
            record.created_ts
        };
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO pending_update (id, battlegroup, namespace, latest_steam_build, \
                 local_steam_build, live_version, downloaded_version, due_ts, created_ts)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    battlegroup = excluded.battlegroup,
                    namespace = excluded.namespace,
                    latest_steam_build = excluded.latest_steam_build,
                    local_steam_build = excluded.local_steam_build,
                    live_version = excluded.live_version,
                    downloaded_version = excluded.downloaded_version,
                    due_ts = excluded.due_ts",
                params![
                    record.battlegroup,
                    record.namespace,
                    record.latest_steam_build,
                    record.local_steam_build,
                    record.live_version,
                    record.downloaded_version,
                    record.due_ts,
                    created_ts,
                ],
            )?;
            Ok(())
        })
    }

    pub fn clear_pending_update(&self) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM pending_update WHERE id = 1", [])?;
            Ok(())
        })
    }

    pub fn defer_pending_update(&self, delay_secs: i64) -> Result<()> {
        let due_ts = Utc::now().timestamp() + delay_secs.max(60);
        self.with_conn(|c| {
            c.execute(
                "UPDATE pending_update SET due_ts = ?1 WHERE id = 1",
                params![due_ts],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::tempdir;

    #[test]
    fn pending_update_roundtrip() {
        let s = Store::open(&tempdir().join("s.sqlite")).unwrap();
        assert!(s.load_pending_update().unwrap().is_none());
        let rec = PendingUpdateRecord {
            battlegroup: "alpha".into(),
            namespace: "funcom-seabass-alpha".into(),
            latest_steam_build: Some("12345".into()),
            local_steam_build: Some("12340".into()),
            live_version: Some("12340".into()),
            downloaded_version: "12345".into(),
            due_ts: 1_700_000_000,
            created_ts: 0,
        };
        s.upsert_pending_update(&rec).unwrap();
        let loaded = s.load_pending_update().unwrap().unwrap();
        assert_eq!(loaded.downloaded_version, "12345");
        assert!(loaded.created_ts > 0);
        s.clear_pending_update().unwrap();
        assert!(s.load_pending_update().unwrap().is_none());
    }
}
