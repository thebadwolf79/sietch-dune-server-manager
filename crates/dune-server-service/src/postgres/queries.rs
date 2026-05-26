use anyhow::{Context, Result};
use serde::Serialize;

use super::conn::PgClient;

#[derive(Debug, Clone, Serialize)]
pub struct PlayerLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(rename = "dimensionIndex")]
    pub dimension_index: Option<i32>,
    #[serde(rename = "partitionId")]
    pub partition_id: Option<i64>,
    /// Pawn actor class — useful sanity for the UI ("…DunePlayerCharacter_C").
    pub source: String,
}

// The live player position is on `dune.actors`, not `dune.player_state`. The
// pawn actor is referenced from `player_state.player_pawn_id`. Its `transform`
// is a composite `(location:(x,y,z), rotation:(x,y,z,w))`. Confirmed via
// schema probe 2026-05-26 against funcom-seabass-sh-* on the LAN test host.
const PLAYER_POSITION_SQL: &str = "
SELECT
    ((a.transform).location).x::float8 AS x,
    ((a.transform).location).y::float8 AS y,
    ((a.transform).location).z::float8 AS z,
    a.dimension_index,
    a.partition_id,
    a.class
FROM dune.player_state ps
JOIN dune.actors a       ON a.id = ps.player_pawn_id
JOIN dune.accounts acct  ON acct.id = ps.account_id
WHERE acct.\"user\"::text = $1
LIMIT 1
";

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    #[serde(rename = "flsId")]
    pub fls_id: String,
    pub name: String,
    pub online: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: String,
    pub level: Option<i32>,
    #[serde(rename = "partitionId")]
    pub partition_id: Option<i64>,
}

const PLAYER_STATE_COLUMN_SQL: &str = "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'dune'
  AND table_name = 'player_state'
  AND column_name = ANY($1)
";

const LEVEL_COLUMN_CANDIDATES: &[&str] = &[
    "level",
    "character_level",
    "player_level",
    "experience_level",
    "current_level",
    "total_level",
];

fn players_sql(level_expr: &str) -> String {
    format!(
        r#"
WITH matches AS (
    SELECT DISTINCT
        COALESCE(acct."user"::text, '') AS fls_id,
        COALESCE(ps.character_name, '')   AS character_name,
        COALESCE(ps.online_status::text, '') AS online_status,
        COALESCE(
            to_char(ps.last_avatar_activity AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),
            ''
        ) AS last_seen,
        {level_expr} AS player_level,
        a.partition_id
    FROM dune.player_state ps
    LEFT JOIN dune.accounts acct           ON acct.id = ps.account_id
    LEFT JOIN dune.encrypted_accounts enc  ON enc.id  = ps.account_id
    LEFT JOIN dune.actors a                ON a.id     = ps.player_pawn_id
    WHERE lower(ps.character_name) LIKE lower($1)
       OR lower(convert_from(enc.encrypted_funcom_id, 'UTF8')) LIKE lower($1)
)
SELECT fls_id, character_name, online_status, last_seen, player_level, partition_id
FROM matches
WHERE fls_id <> ''
ORDER BY
    CASE WHEN lower(online_status) = 'online' THEN 0 ELSE 1 END,
    last_seen DESC,
    character_name ASC
LIMIT $2;
"#
    )
}

/// Outcome of a player-position probe.
pub enum PositionProbe {
    Found(PlayerLocation),
    /// No row matched — usually means the player is offline (no live pawn),
    /// or the fls_id doesn't exist on this server.
    NoRow,
}

/// Look up the live world position for a player. Joins `player_state` →
/// `actors` on `player_pawn_id` and deconstructs the composite
/// `actors.transform` (`(location:(x,y,z), rotation:(x,y,z,w))`).
pub async fn get_player_location(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
) -> Result<PositionProbe> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(PLAYER_POSITION_SQL, &[&fls_id])
        .await
        .context("querying player pawn position")?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(PositionProbe::NoRow);
    };
    Ok(PositionProbe::Found(PlayerLocation {
        x: row.get::<_, f64>(0),
        y: row.get::<_, f64>(1),
        z: row.get::<_, f64>(2),
        dimension_index: row.try_get::<_, i32>(3).ok(),
        partition_id: row.try_get::<_, i64>(4).ok(),
        source: row.try_get::<_, String>(5).unwrap_or_default(),
    }))
}

pub async fn search_players(
    pg: &PgClient,
    namespace: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<Player>> {
    let safe_limit = limit.clamp(1, 200) as i64;
    let pattern = format!("%{}%", query);

    let state = pg.client(namespace).await?;
    let level_column = player_level_column(state.client()).await?;
    let level_expr = level_column
        .as_deref()
        .map(|column| format!("ps.\"{column}\"::int"))
        .unwrap_or_else(|| "NULL::int".to_string());
    let sql = players_sql(&level_expr);
    let rows = state
        .client()
        .query(&sql, &[&pattern, &safe_limit])
        .await
        .context("running player search query")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(Player {
            fls_id: row.get::<_, String>(0),
            name: row.get::<_, String>(1),
            online: row.get::<_, String>(2),
            last_seen: row.get::<_, String>(3),
            level: row.try_get::<_, Option<i32>>(4).ok().flatten(),
            partition_id: row.try_get::<_, Option<i64>>(5).ok().flatten(),
        });
    }
    Ok(out)
}

async fn player_level_column(client: &tokio_postgres::Client) -> Result<Option<String>> {
    let rows = client
        .query(PLAYER_STATE_COLUMN_SQL, &[&LEVEL_COLUMN_CANDIDATES])
        .await
        .context("checking player level column")?;
    let available = rows
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<std::collections::HashSet<_>>();
    Ok(LEVEL_COLUMN_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| available.contains(*candidate))
        .map(str::to_string))
}
