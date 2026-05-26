use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;

use super::Store;

#[derive(Debug, Clone, Serialize)]
pub struct AdminHistoryEntry {
    pub id: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub command: String,
    pub payload: Value,
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdminHistoryFilter {
    pub limit: Option<u32>,
}

impl Store {
    pub fn record_admin_command(
        &self,
        command: &str,
        payload: &Value,
        ok: bool,
        message: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let payload_json = serde_json::to_string(payload)?;
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO admin_commands (created_at, command, payload_json, ok, message)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![now, command, payload_json, ok as i32, message],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    pub fn list_admin_commands(
        &self,
        filter: AdminHistoryFilter,
    ) -> Result<Vec<AdminHistoryEntry>> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 500) as i64;
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, created_at, command, payload_json, ok, message
                 FROM admin_commands ORDER BY created_at DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], |row| {
                    let payload_text: String = row.get(3)?;
                    let payload = serde_json::from_str(&payload_text).unwrap_or(Value::Null);
                    let ok_raw: i64 = row.get(4)?;
                    Ok(AdminHistoryEntry {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        command: row.get(2)?,
                        payload,
                        ok: ok_raw != 0,
                        message: row.get(5)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::tempdir;

    #[test]
    fn admin_history_roundtrip() {
        let s = Store::open(&tempdir().join("s.sqlite")).unwrap();
        let payload = serde_json::json!({"ServerCommand": "ServiceBroadcast", "Title": "hi"});
        let id = s
            .record_admin_command("ServiceBroadcast", &payload, true, None)
            .unwrap();
        assert!(id > 0);
        let list = s
            .list_admin_commands(AdminHistoryFilter::default())
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].command, "ServiceBroadcast");
        assert!(list[0].ok);
    }
}
