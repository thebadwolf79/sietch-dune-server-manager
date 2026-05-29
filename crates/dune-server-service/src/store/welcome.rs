use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use super::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WelcomeGrantStatus {
    Pending,
    Granted,
    Failed,
}

impl WelcomeGrantStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Granted => "granted",
            Self::Failed => "failed",
        }
    }

    fn from_str(raw: &str) -> Self {
        match raw {
            "granted" => Self::Granted,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeGrantRecord {
    pub player_id: String,
    pub package_version: String,
    pub account_id: i64,
    pub character_name: Option<String>,
    pub status: WelcomeGrantStatus,
    pub detected_at: String,
    pub updated_at: String,
    pub granted_at: Option<String>,
    pub attempts: i64,
    pub last_online_status: Option<String>,
    pub first_online_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WelcomeActionStatus {
    Pending,
    Published,
    Confirmed,
    Failed,
}

impl WelcomeActionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Published => "published",
            Self::Confirmed => "confirmed",
            Self::Failed => "failed",
        }
    }

    fn from_str(raw: &str) -> Self {
        match raw {
            "published" => Self::Published,
            "confirmed" => Self::Confirmed,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeActionRecord {
    pub player_id: String,
    pub package_version: String,
    pub account_id: i64,
    pub action_index: i64,
    pub action_type: String,
    pub status: WelcomeActionStatus,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub attempts: i64,
    pub item_name: Option<String>,
    pub baseline_quantity: Option<i64>,
    pub expected_quantity: Option<i64>,
    pub last_error: Option<String>,
}

impl Store {
    pub fn welcome_grant_exists(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
    ) -> Result<bool> {
        self.with_conn(|c| {
            let exists = c
                .query_row(
                    "SELECT 1 FROM welcome_grants
                     WHERE player_id = ?1 AND package_version = ?2 AND account_id = ?3
                     LIMIT 1",
                    params![player_id, package_version, account_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            Ok(exists)
        })
    }

    /// Cheap sqlite check used by the welcome-message worker to skip accounts
    /// whose whisper is already confirmed before paying for a Postgres chat
    /// lookup on every scan.
    pub fn welcome_action_confirmed(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        action_index: i64,
    ) -> Result<bool> {
        self.with_conn(|c| {
            let confirmed = c
                .query_row(
                    "SELECT 1 FROM welcome_grant_actions
                     WHERE player_id = ?1 AND package_version = ?2 AND account_id = ?3
                       AND action_index = ?4 AND status = 'confirmed'
                     LIMIT 1",
                    params![player_id, package_version, account_id, action_index],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            Ok(confirmed)
        })
    }

    pub fn insert_welcome_grant_granted(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        character_name: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO welcome_grants (
                    player_id, package_version, account_id, character_name, status,
                    detected_at, updated_at, granted_at, attempts, last_error
                 )
                 VALUES (?1, ?2, ?3, ?4, 'granted', ?5, ?5, ?5, 1, NULL)",
                params![player_id, package_version, account_id, character_name, now],
            )?;
            Ok(())
        })
    }

    pub fn insert_welcome_grant_failed(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        character_name: Option<&str>,
        error: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO welcome_grants (
                    player_id, package_version, account_id, character_name, status,
                    detected_at, updated_at, granted_at, attempts, last_error
                 )
                 VALUES (?1, ?2, ?3, ?4, 'failed', ?5, ?5, NULL, 1, ?6)",
                params![
                    player_id,
                    package_version,
                    account_id,
                    character_name,
                    now,
                    error
                ],
            )?;
            Ok(())
        })
    }

    pub fn ensure_welcome_grant(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        character_name: Option<&str>,
        online_status: &str,
    ) -> Result<WelcomeGrantRecord> {
        let now = Utc::now().to_rfc3339();
        let is_online = online_status.eq_ignore_ascii_case("Online");
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO welcome_grants (
                    player_id, package_version, account_id, character_name, status,
                    detected_at, updated_at, last_online_status, first_online_at
                 )
                 VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?5, ?6, ?7)
                 ON CONFLICT(player_id, package_version, account_id) DO UPDATE SET
                    character_name = COALESCE(excluded.character_name, welcome_grants.character_name),
                    last_online_status = excluded.last_online_status,
                    first_online_at = CASE
                        WHEN welcome_grants.status != 'pending' THEN welcome_grants.first_online_at
                        WHEN excluded.first_online_at IS NULL THEN NULL
                        ELSE COALESCE(welcome_grants.first_online_at, excluded.first_online_at)
                    END,
                    updated_at = CASE
                        WHEN welcome_grants.status = 'pending' THEN excluded.updated_at
                        ELSE welcome_grants.updated_at
                    END",
                params![
                    player_id,
                    package_version,
                    account_id,
                    character_name,
                    now,
                    online_status,
                    if is_online { Some(now.as_str()) } else { None },
                ],
            )?;
            select_welcome_grant(c, player_id, package_version, account_id)
        })
    }

    /// Clears a `failed` welcome-grant ledger row so the next scan re-attempts
    /// it. Only `failed` rows are removed — `granted` rows are left in place so
    /// a retry can never duplicate a successful package. Returns the number of
    /// rows deleted (0 if the grant was not failed or does not exist).
    pub fn delete_welcome_grant(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
    ) -> Result<usize> {
        self.with_conn(|c| {
            let removed = c.execute(
                "DELETE FROM welcome_grants
                 WHERE player_id = ?1 AND package_version = ?2 AND account_id = ?3
                   AND status = 'failed'",
                params![player_id, package_version, account_id],
            )?;
            Ok(removed)
        })
    }

    pub fn list_welcome_grants(&self, limit: u32) -> Result<Vec<WelcomeGrantRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT player_id, package_version, account_id, character_name, status,
                        detected_at, updated_at, granted_at, attempts, last_online_status,
                        first_online_at, last_error
                 FROM welcome_grants
                 WHERE package_version NOT LIKE '%:message'
                 ORDER BY updated_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit], read_welcome_grant)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn ensure_welcome_action(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        action_index: i64,
        action_type: &str,
    ) -> Result<WelcomeActionRecord> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO welcome_grant_actions (
                    player_id, package_version, account_id, action_index, action_type, status,
                    created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)
                 ON CONFLICT(player_id, package_version, account_id, action_index) DO UPDATE SET
                    action_type = excluded.action_type",
                params![
                    player_id,
                    package_version,
                    account_id,
                    action_index,
                    action_type,
                    now
                ],
            )?;
            select_welcome_action(c, player_id, package_version, account_id, action_index)
        })
    }

    pub fn mark_welcome_action_published(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        action_index: i64,
        item_name: Option<&str>,
        baseline_quantity: Option<i64>,
        expected_quantity: Option<i64>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "UPDATE welcome_grant_actions
                 SET status = 'published',
                     updated_at = ?4,
                     published_at = ?4,
                     attempts = attempts + 1,
                     item_name = COALESCE(?5, item_name),
                     baseline_quantity = COALESCE(?6, baseline_quantity),
                     expected_quantity = COALESCE(?7, expected_quantity),
                     last_error = NULL
                 WHERE player_id = ?1 AND package_version = ?2 AND action_index = ?3 AND account_id = ?8",
                params![
                    player_id,
                    package_version,
                    action_index,
                    now,
                    item_name,
                    baseline_quantity,
                    expected_quantity,
                    account_id
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_welcome_action_confirmed(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        action_index: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "UPDATE welcome_grant_actions
                 SET status = 'confirmed',
                     updated_at = ?4,
                     confirmed_at = ?4,
                     last_error = NULL
                 WHERE player_id = ?1 AND package_version = ?2 AND action_index = ?3 AND account_id = ?5",
                params![player_id, package_version, action_index, now, account_id],
            )?;
            Ok(())
        })
    }

    pub fn mark_welcome_action_failed(
        &self,
        player_id: &str,
        package_version: &str,
        account_id: i64,
        action_index: i64,
        error: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.with_conn(|c| {
            c.execute(
                "UPDATE welcome_grant_actions
                 SET status = 'failed',
                     updated_at = ?4,
                     last_error = ?5
                 WHERE player_id = ?1 AND package_version = ?2 AND action_index = ?3 AND account_id = ?6",
                params![player_id, package_version, action_index, now, error, account_id],
            )?;
            Ok(())
        })
    }
}

fn select_welcome_grant(
    c: &rusqlite::Connection,
    player_id: &str,
    package_version: &str,
    account_id: i64,
) -> rusqlite::Result<WelcomeGrantRecord> {
    c.query_row(
        "SELECT player_id, package_version, account_id, character_name, status,
                detected_at, updated_at, granted_at, attempts, last_online_status,
                first_online_at, last_error
         FROM welcome_grants
         WHERE player_id = ?1 AND package_version = ?2 AND account_id = ?3",
        params![player_id, package_version, account_id],
        read_welcome_grant,
    )
    .optional()?
    .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

fn read_welcome_grant(row: &rusqlite::Row<'_>) -> rusqlite::Result<WelcomeGrantRecord> {
    let status: String = row.get(4)?;
    Ok(WelcomeGrantRecord {
        player_id: row.get(0)?,
        package_version: row.get(1)?,
        account_id: row.get(2)?,
        character_name: row.get(3)?,
        status: WelcomeGrantStatus::from_str(&status),
        detected_at: row.get(5)?,
        updated_at: row.get(6)?,
        granted_at: row.get(7)?,
        attempts: row.get(8)?,
        last_online_status: row.get(9)?,
        first_online_at: row.get(10)?,
        last_error: row.get(11)?,
    })
}

fn select_welcome_action(
    c: &rusqlite::Connection,
    player_id: &str,
    package_version: &str,
    account_id: i64,
    action_index: i64,
) -> rusqlite::Result<WelcomeActionRecord> {
    c.query_row(
        "SELECT player_id, package_version, account_id, action_index, action_type, status,
                created_at, updated_at, published_at, confirmed_at, attempts,
                item_name, baseline_quantity, expected_quantity, last_error
         FROM welcome_grant_actions
         WHERE player_id = ?1 AND package_version = ?2 AND account_id = ?3 AND action_index = ?4",
        params![player_id, package_version, account_id, action_index],
        read_welcome_action,
    )
    .optional()?
    .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

fn read_welcome_action(row: &rusqlite::Row<'_>) -> rusqlite::Result<WelcomeActionRecord> {
    let status: String = row.get(5)?;
    Ok(WelcomeActionRecord {
        player_id: row.get(0)?,
        package_version: row.get(1)?,
        account_id: row.get(2)?,
        action_index: row.get(3)?,
        action_type: row.get(4)?,
        status: WelcomeActionStatus::from_str(&status),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        published_at: row.get(8)?,
        confirmed_at: row.get(9)?,
        attempts: row.get(10)?,
        item_name: row.get(11)?,
        baseline_quantity: row.get(12)?,
        expected_quantity: row.get(13)?,
        last_error: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::tempdir;

    #[test]
    fn welcome_grant_lifecycle() {
        let s = Store::open(&tempdir().join("s.sqlite")).unwrap();
        assert!(!s.welcome_grant_exists("P1", "v1", 10).unwrap());

        s.insert_welcome_grant_granted("P0", "v1", 9, Some("Duncan"))
            .unwrap();
        assert!(s.welcome_grant_exists("P0", "v1", 9).unwrap());

        s.insert_welcome_grant_failed("P1", "v1", 10, Some("Chani"), "db timeout")
            .unwrap();
        assert!(s.welcome_grant_exists("P1", "v1", 10).unwrap());

        let rows = s.list_welcome_grants(10).unwrap();
        assert_eq!(rows.len(), 2);
        let p1 = rows.iter().find(|row| row.player_id == "P1").unwrap();
        assert_eq!(p1.status, WelcomeGrantStatus::Failed);
        assert_eq!(p1.last_error.as_deref(), Some("db timeout"));

        // Retry clears only the failed row so the next scan re-attempts it.
        assert_eq!(s.delete_welcome_grant("P1", "v1", 10).unwrap(), 1);
        assert!(!s.welcome_grant_exists("P1", "v1", 10).unwrap());
        // A granted row is never removed by retry, so items cannot duplicate.
        assert_eq!(s.delete_welcome_grant("P0", "v1", 9).unwrap(), 0);
        assert!(s.welcome_grant_exists("P0", "v1", 9).unwrap());
    }

    #[test]
    fn welcome_action_lifecycle() {
        let s = Store::open(&tempdir().join("s.sqlite")).unwrap();
        s.ensure_welcome_grant("P1", "v1", 10, Some("Chani"), "Online")
            .unwrap();
        let rec = s
            .ensure_welcome_action("P1", "v1", 10, 0, "grant_item")
            .unwrap();
        assert_eq!(rec.status, WelcomeActionStatus::Pending);
        assert_eq!(rec.account_id, 10);

        s.mark_welcome_action_published("P1", "v1", 10, 0, Some("Literjon"), Some(0), Some(1))
            .unwrap();
        let rec = s
            .ensure_welcome_action("P1", "v1", 10, 0, "grant_item")
            .unwrap();
        assert_eq!(rec.status, WelcomeActionStatus::Published);
        assert_eq!(rec.item_name.as_deref(), Some("Literjon"));
        assert_eq!(rec.expected_quantity, Some(1));

        s.mark_welcome_action_confirmed("P1", "v1", 10, 0).unwrap();
        let rec = s
            .ensure_welcome_action("P1", "v1", 10, 0, "grant_item")
            .unwrap();
        assert_eq!(rec.status, WelcomeActionStatus::Confirmed);
    }

    #[test]
    fn welcome_actions_are_account_scoped() {
        let s = Store::open(&tempdir().join("s.sqlite")).unwrap();
        s.ensure_welcome_grant("P1", "v1", 10, Some("Chani"), "Online")
            .unwrap();
        s.ensure_welcome_grant("P1", "v1", 11, Some("Paul"), "Online")
            .unwrap();
        s.mark_welcome_action_published("P1", "v1", 10, 0, Some("Literjon"), Some(0), Some(1))
            .unwrap();
        let rec = s
            .ensure_welcome_action("P1", "v1", 11, 0, "grant_item")
            .unwrap();
        assert_eq!(rec.status, WelcomeActionStatus::Pending);
        assert_eq!(rec.account_id, 11);
    }
}
