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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeCandidate {
    pub account_id: i64,
    pub fls_id: String,
    pub funcom_id: String,
    pub character_name: Option<String>,
    pub online_status: String,
    pub last_login_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlayer {
    pub account_id: i64,
    pub fls_id: String,
    pub funcom_id: String,
    pub character_name: String,
}

const PLAYER_STATE_COLUMN_SQL: &str = "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'dune'
  AND table_name = 'player_state'
  AND column_name = ANY($1)
";

const WELCOME_CANDIDATES_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id,
    COALESCE(acct.funcom_id::text, '') AS funcom_id,
    NULLIF(ps.character_name, '') AS character_name,
    COALESCE(ps.online_status::text, '') AS online_status,
    COALESCE(to_char(ps.last_login_time AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'), '') AS last_login_time
FROM dune.player_state ps
JOIN dune.accounts acct ON acct.id = ps.account_id
WHERE COALESCE(acct.\"user\"::text, '') <> ''
ORDER BY ps.last_login_time DESC NULLS LAST, acct.id DESC
LIMIT $1
";

const PLAYER_ITEM_QUANTITY_SQL: &str = "
SELECT COALESCE(SUM(CASE WHEN i.id IS NULL THEN 0 ELSE GREATEST(i.stack_size, 1) END), 0)::int8
FROM dune.player_state ps
JOIN dune.accounts acct ON acct.id = ps.account_id
JOIN dune.inventories inv ON inv.actor_id = ps.player_pawn_id
LEFT JOIN dune.items i ON i.inventory_id = inv.id AND i.template_id::text = $2
WHERE acct.\"user\"::text = $1
";

const PLAYER_BACKPACK_ITEM_QUANTITY_SQL: &str = "
SELECT COALESCE(SUM(CASE WHEN i.id IS NULL THEN 0 ELSE GREATEST(i.stack_size, 1) END), 0)::int8
FROM dune.player_state ps
JOIN dune.accounts acct ON acct.id = ps.account_id
JOIN dune.inventories inv ON inv.actor_id = ps.player_pawn_id AND inv.inventory_type = 0
LEFT JOIN dune.items i ON i.inventory_id = inv.id
WHERE acct.\"user\"::text = $1
";

const CHAT_PLAYER_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id,
    COALESCE(acct.funcom_id::text, '') AS funcom_id,
    COALESCE(ps.character_name, '') AS character_name
FROM dune.player_state ps
JOIN dune.accounts acct ON acct.id = ps.account_id
WHERE lower(COALESCE(acct.\"user\"::text, '')) = lower($1)
   OR lower(COALESCE(acct.funcom_id::text, '')) = lower($1)
   OR lower(COALESCE(ps.character_name, '')) = lower($1)
ORDER BY
    CASE
        WHEN lower(COALESCE(acct.\"user\"::text, '')) = lower($1) THEN 0
        WHEN lower(COALESCE(acct.funcom_id::text, '')) = lower($1) THEN 1
        ELSE 2
    END,
    ps.last_login_time DESC NULLS LAST
LIMIT 1
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
            fls_id: row.try_get::<_, String>(0).unwrap_or_default(),
            name: row.try_get::<_, String>(1).unwrap_or_default(),
            online: row.try_get::<_, String>(2).unwrap_or_default(),
            last_seen: row.try_get::<_, String>(3).unwrap_or_default(),
            level: row.try_get::<_, Option<i32>>(4).ok().flatten(),
            partition_id: row.try_get::<_, Option<i64>>(5).ok().flatten(),
        });
    }
    Ok(out)
}

pub async fn list_welcome_candidates(
    pg: &PgClient,
    namespace: &str,
    limit: u32,
) -> Result<Vec<WelcomeCandidate>> {
    let safe_limit = limit.clamp(1, 1000) as i64;
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(WELCOME_CANDIDATES_SQL, &[&safe_limit])
        .await
        .context("querying welcome package candidates")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let last_login_raw = row.try_get::<_, String>(5).unwrap_or_default();
        out.push(WelcomeCandidate {
            account_id: row.try_get::<_, i64>(0).unwrap_or_default(),
            fls_id: row.try_get::<_, String>(1).unwrap_or_default(),
            funcom_id: row.try_get::<_, String>(2).unwrap_or_default(),
            character_name: row.try_get::<_, Option<String>>(3).ok().flatten(),
            online_status: row.try_get::<_, String>(4).unwrap_or_default(),
            last_login_time: if last_login_raw.trim().is_empty() {
                None
            } else {
                Some(last_login_raw)
            },
        });
    }
    Ok(out)
}

pub async fn player_item_quantity(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
    item_name: &str,
) -> Result<i64> {
    let state = pg.client(namespace).await?;
    let row = state
        .client()
        .query_one(PLAYER_ITEM_QUANTITY_SQL, &[&fls_id, &item_name])
        .await
        .with_context(|| format!("querying player inventory quantity for {item_name}"))?;
    Ok(row.try_get::<_, i64>(0).unwrap_or_default())
}

pub async fn player_backpack_item_quantity(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
) -> Result<i64> {
    let state = pg.client(namespace).await?;
    let row = state
        .client()
        .query_one(PLAYER_BACKPACK_ITEM_QUANTITY_SQL, &[&fls_id])
        .await
        .context("querying player backpack item quantity")?;
    Ok(row.try_get::<_, i64>(0).unwrap_or_default())
}

pub async fn resolve_chat_player(
    pg: &PgClient,
    namespace: &str,
    lookup: &str,
) -> Result<Option<ChatPlayer>> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(CHAT_PLAYER_SQL, &[&lookup.trim()])
        .await
        .with_context(|| format!("resolving chat player {lookup}"))?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(ChatPlayer {
        account_id: row.try_get::<_, i64>(0).unwrap_or_default(),
        fls_id: row.try_get::<_, String>(1).unwrap_or_default(),
        funcom_id: row.try_get::<_, String>(2).unwrap_or_default(),
        character_name: row.try_get::<_, String>(3).unwrap_or_default(),
    }))
}

async fn player_level_column(client: &tokio_postgres::Client) -> Result<Option<String>> {
    let rows = client
        .query(PLAYER_STATE_COLUMN_SQL, &[&LEVEL_COLUMN_CANDIDATES])
        .await
        .context("checking player level column")?;
    let available = rows
        .into_iter()
        .filter_map(|row| row.try_get::<_, String>(0).ok())
        .collect::<std::collections::HashSet<_>>();
    Ok(LEVEL_COLUMN_CANDIDATES
        .iter()
        .copied()
        .find(|candidate| available.contains(*candidate))
        .map(str::to_string))
}
