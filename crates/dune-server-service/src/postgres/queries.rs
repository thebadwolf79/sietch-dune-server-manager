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
JOIN dune.encrypted_accounts acct ON acct.id = ps.account_id
WHERE acct.\"user\"::text = $1
LIMIT 1
";

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    #[serde(rename = "flsId")]
    pub fls_id: String,
    pub name: String,
    /// Live online status. Seeded from `player_state.online_status` in the DB
    /// (`"online"` / `"offline"` / `"loading"`), then enriched by the Director
    /// (BGD) overlay in `admin::players::search_players` to also carry
    /// `"grace period"` and `"transit"` for players the DB still reports as
    /// online/offline. Free-form string so new BGD states pass through to the UI.
    pub online: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: String,
    pub level: Option<i32>,
    #[serde(rename = "partitionId")]
    pub partition_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WelcomeAccount {
    pub account_id: i64,
    pub fls_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBackpack {
    pub inventory_id: i64,
    pub character_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPlayer {
    pub account_id: i64,
    pub fls_id: String,
    pub funcom_id: String,
    pub character_name: String,
}

#[derive(Debug, Clone)]
pub struct BackpackGrantItem {
    pub template_id: String,
    pub quantity: i64,
    pub stats_json: String,
}

const PLAYER_STATE_COLUMN_SQL: &str = "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'dune'
  AND table_name = 'player_state'
  AND column_name = ANY($1)
";

const WELCOME_ACCOUNTS_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id
FROM dune.encrypted_accounts acct
WHERE COALESCE(acct.\"user\"::text, '') <> ''
ORDER BY acct.id ASC
";

const PLAYER_BACKPACK_INVENTORY_SQL: &str = "
SELECT
    inv.id::int8 AS inventory_id,
    NULLIF(ps.character_name, '') AS character_name
FROM dune.player_state ps
JOIN dune.actors pawn ON pawn.id = ps.player_pawn_id
JOIN dune.inventories inv ON inv.actor_id = ps.player_pawn_id
                         AND inv.inventory_type = 0
WHERE ps.account_id = $1::int8
  AND pawn.class = '/Game/Dune/Characters/Player/BP_DunePlayerCharacter.BP_DunePlayerCharacter_C'
ORDER BY ps.last_login_time DESC NULLS LAST, inv.id DESC
LIMIT 1
";

const PLAYER_BACKPACK_FREE_SLOTS_SQL: &str = "
SELECT gs::int8 AS position_index
FROM generate_series(0, 10000) AS gs
WHERE NOT EXISTS (
    SELECT 1
    FROM dune.items i
    WHERE i.inventory_id = $1::int8
      AND i.position_index = gs
)
ORDER BY gs
LIMIT $2
";

const PLAYER_BACKPACK_INSERT_ITEM_SQL: &str = "
INSERT INTO dune.items (
    inventory_id,
    stack_size,
    position_index,
    template_id,
    is_new,
    acquisition_time,
    stats,
    quality_level
)
VALUES (
    $1::int8,
    $2::int8,
    $3::int8,
    $4::text,
    TRUE,
    EXTRACT(EPOCH FROM now())::int8,
    $5::text::jsonb,
    0
)
RETURNING id::int8
";

const CHAT_PLAYER_SQL: &str = "
SELECT
    acct.id::int8 AS account_id,
    COALESCE(acct.\"user\"::text, '') AS fls_id,
    COALESCE(acct.funcom_id::text, '') AS funcom_id,
    COALESCE(ps.character_name, '') AS character_name
FROM dune.player_state ps
JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
LEFT JOIN dune.accounts acct ON acct.id = ps.account_id
WHERE lower(COALESCE(enc.\"user\"::text, '')) = lower($1)
   OR lower(COALESCE(acct.funcom_id::text, '')) = lower($1)
   OR lower(COALESCE(ps.character_name, '')) = lower($1)
ORDER BY
    CASE
        WHEN lower(COALESCE(enc.\"user\"::text, '')) = lower($1) THEN 0
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
        COALESCE(enc."user"::text, '') AS fls_id,
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
       OR lower(COALESCE(enc."user"::text, '')) LIKE lower($1)
       OR lower(COALESCE(acct.funcom_id::text, '')) LIKE lower($1)
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

pub async fn list_welcome_accounts(pg: &PgClient, namespace: &str) -> Result<Vec<WelcomeAccount>> {
    let state = pg.client(namespace).await?;
    let rows = state
        .client()
        .query(WELCOME_ACCOUNTS_SQL, &[])
        .await
        .context("querying welcome package accounts")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WelcomeAccount {
            account_id: row.try_get::<_, i64>(0).unwrap_or_default(),
            fls_id: row.try_get::<_, String>(1).unwrap_or_default(),
        });
    }
    Ok(out)
}

pub async fn resolve_account_backpack(
    pg: &PgClient,
    namespace: &str,
    account_id: i64,
) -> Result<Option<AccountBackpack>> {
    let state = pg.client(namespace).await?;
    let row = state
        .client()
        .query_opt(PLAYER_BACKPACK_INVENTORY_SQL, &[&account_id])
        .await
        .context("resolving account backpack inventory")?;
    Ok(row.map(|row| AccountBackpack {
        inventory_id: row.try_get::<_, i64>(0).unwrap_or_default(),
        character_name: row.try_get::<_, Option<String>>(1).ok().flatten(),
    }))
}

pub async fn insert_items_to_backpack(
    pg: &PgClient,
    namespace: &str,
    inventory_id: i64,
    items: &[BackpackGrantItem],
) -> Result<Vec<i64>> {
    let mut state = pg.dedicated_client(namespace).await?;
    let tx = state
        .client_mut()
        .transaction()
        .await
        .context("starting welcome item grant transaction")?;

    let slot_limit = items.len() as i64;
    let slot_rows = tx
        .query(
            PLAYER_BACKPACK_FREE_SLOTS_SQL,
            &[&inventory_id, &slot_limit],
        )
        .await
        .context("finding free backpack slots")?;
    if slot_rows.len() != items.len() {
        return Err(anyhow::anyhow!(
            "not enough free backpack slots for welcome package: needed {}, found {}",
            items.len(),
            slot_rows.len()
        ));
    }

    let insert = tx
        .prepare(PLAYER_BACKPACK_INSERT_ITEM_SQL)
        .await
        .context("preparing welcome item insert")?;
    let mut inserted_ids = Vec::with_capacity(items.len());
    for (item, slot) in items.iter().zip(slot_rows.iter()) {
        let position_index = slot.try_get::<_, i64>(0).unwrap_or_default();
        let row = tx
            .query_one(
                &insert,
                &[
                    &inventory_id,
                    &item.quantity,
                    &position_index,
                    &item.template_id,
                    &item.stats_json,
                ],
            )
            .await
            .with_context(|| format!("inserting welcome item {}", item.template_id))?;
        inserted_ids.push(row.try_get::<_, i64>(0).unwrap_or_default());
    }

    tx.commit()
        .await
        .context("committing welcome item grant transaction")?;
    Ok(inserted_ids)
}

/// Resolve a player's controller id + normalized online status from their FLS
/// id, locking the `player_state` row so the online check stays consistent for
/// the grant transaction. Normalizes online_status to lower/trimmed so a
/// strict `= 'offline'` test fails closed on NULL / 'Loading' / 'Connecting'.
const CURRENCY_RESOLVE_SQL: &str = "
SELECT ps.player_controller_id,
       lower(btrim(COALESCE(ps.online_status::text, ''))) AS online_norm
FROM dune.player_state ps
JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
WHERE enc.\"user\"::text = $1
FOR UPDATE OF ps
";

// ADD semantics: "grant N" adds N to the existing balance, never overwrites it
// (the player may already hold currency). New row -> balance = N.
const CURRENCY_UPSERT_SQL: &str = "
INSERT INTO dune.player_virtual_currency_balances (player_controller_id, currency_id, balance)
VALUES ($1::int8, $2::int2, $3::int8)
ON CONFLICT (player_controller_id, currency_id)
DO UPDATE SET balance = dune.player_virtual_currency_balances.balance + EXCLUDED.balance
RETURNING balance::int8
";

#[derive(Debug, Clone)]
pub struct CurrencyGrantOutcome {
    pub player_controller_id: i64,
    pub new_balance: i64,
}

/// Outcome of a currency grant. Non-`Granted` variants are expected, recoverable
/// states (not errors) the handler turns into clear messages; `Err` is reserved
/// for actual DB faults.
pub enum CurrencyGrantResult {
    Granted(CurrencyGrantOutcome),
    /// No `player_state` row for the FLS id.
    PlayerNotFound,
    /// More than one `player_state` row matched the FLS id — refuse rather than
    /// guess which controller to credit.
    Ambiguous,
    /// Player is not strictly offline. Carries the normalized status we saw.
    PlayerOnline(String),
}

/// Grant currency to a player by UPSERTing `dune.player_virtual_currency_balances`
/// (`House Scrip` = currency_id 1, `Bank Solari` = 0). There is no engine command
/// for currency, so this is a guarded offline DB write.
///
/// Safety (verified against the live schema; reviewed by QC + Stress):
/// - one transaction: resolve + `FOR UPDATE` lock the `player_state` row, then UPSERT;
/// - refuses unless the player is strictly offline (the engine overwrites DB
///   currency edits on logout, so online/loading is unsafe);
/// - fails closed on unknown FLS id or >1 matching row.
///
/// The caller (HTTP handler) whitelists `currency_id` and clamps `amount`; this
/// function trusts neither game-state assumption beyond what it can verify here.
pub async fn grant_currency(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
    currency_id: i16,
    amount: i64,
) -> Result<CurrencyGrantResult> {
    let mut state = pg.dedicated_client(namespace).await?;
    let tx = state
        .client_mut()
        .transaction()
        .await
        .context("starting currency grant transaction")?;

    let rows = tx
        .query(CURRENCY_RESOLVE_SQL, &[&fls_id])
        .await
        .context("resolving player for currency grant")?;
    if rows.is_empty() {
        return Ok(CurrencyGrantResult::PlayerNotFound);
    }
    if rows.len() > 1 {
        return Ok(CurrencyGrantResult::Ambiguous);
    }

    let row = &rows[0];
    let controller_id: i64 = row
        .try_get(0)
        .context("reading player_controller_id")?;
    let online_norm: String = row.try_get(1).unwrap_or_default();
    if online_norm != "offline" {
        return Ok(CurrencyGrantResult::PlayerOnline(online_norm));
    }

    let upserted = tx
        .query_one(CURRENCY_UPSERT_SQL, &[&controller_id, &currency_id, &amount])
        .await
        .context("upserting player currency balance")?;
    let new_balance: i64 = upserted.try_get(0).context("reading new balance")?;

    tx.commit().await.context("committing currency grant")?;
    Ok(CurrencyGrantResult::Granted(CurrencyGrantOutcome {
        player_controller_id: controller_id,
        new_balance,
    }))
}

// Intel ("Tech Knowledge points") is NOT a currency row. It's a single integer
// inside the player blob: dune.actors.properties (jsonb) at
// {TechKnowledgePlayerComponent, m_TechKnowledgePoints}, on the CHARACTER actor
// (class BP_DunePlayerCharacter_C, id = player_state.player_pawn_id). The same
// component also holds a large m_TechKnowledge unlocked-items array.
//
// INCIDENT LESSON: never round-trip / restructure that blob. We resolve the pawn
// id + online status (locking player_state), then do ONE server-side jsonb_set on
// exactly that 2-element path — the rest of properties is untouched.
const INTEL_RESOLVE_SQL: &str = "
SELECT ps.player_pawn_id,
       lower(btrim(COALESCE(ps.online_status::text, ''))) AS online_norm
FROM dune.player_state ps
JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
WHERE enc.\"user\"::text = $1
FOR UPDATE OF ps
";

// Single-leaf, ADD semantics. Guards (fail closed on 0 rows):
//  - id is the resolved pawn,
//  - class is EXACTLY the player-character class (no subclass/test-actor match),
//  - the leaf exists AND is a JSON number (so ::bigint can't be fed garbage; a
//    non-number leaf yields 0 rows rather than a corrupting write).
// `create_missing` is false — we never invent the path. The path is a fixed
// literal here, never built from input.
const INTEL_UPDATE_SQL: &str = "
UPDATE dune.actors
SET properties = jsonb_set(
        properties,
        '{TechKnowledgePlayerComponent,m_TechKnowledgePoints}',
        to_jsonb(
            (properties #>> '{TechKnowledgePlayerComponent,m_TechKnowledgePoints}')::int8 + $2::int8
        ),
        false
    )
WHERE id = $1::int8
  AND class = '/Game/Dune/Characters/Player/BP_DunePlayerCharacter.BP_DunePlayerCharacter_C'
  AND jsonb_typeof(properties #> '{TechKnowledgePlayerComponent,m_TechKnowledgePoints}') = 'number'
RETURNING (properties #>> '{TechKnowledgePlayerComponent,m_TechKnowledgePoints}')::int8 AS new_points
";

/// Outcome of an Intel grant. Non-`Granted` variants are expected, recoverable
/// states the handler turns into clear messages; `Err` is reserved for DB faults.
pub enum IntelGrantResult {
    Granted { new_points: i64 },
    PlayerNotFound,
    Ambiguous,
    PlayerOnline(String),
    /// Pawn id was NULL, or the UPDATE matched 0 rows (wrong class / missing or
    /// non-numeric TechKnowledge leaf). We refuse rather than create/guess.
    CharacterActorMissing,
}

/// Award Intel (Tech Knowledge points) to a player's character via a guarded
/// offline single-leaf jsonb_set. No engine command exists for Intel.
///
/// Safety (verified live; reviewed by QC + Stress):
/// - one transaction: resolve + `FOR UPDATE` the player_state row, then jsonb_set;
/// - refuses unless strictly offline (engine overwrites DB edits on logout);
/// - touches ONLY the m_TechKnowledgePoints leaf — the rest of the blob
///   (incl. the unlocked-tech array) is left intact (the incident lesson);
/// - fails closed on unknown / ambiguous FLS id, NULL pawn, wrong actor class,
///   or a missing/non-numeric leaf.
///
/// Caller (HTTP handler) clamps `amount`. NOTE: this verifies the DB write via
/// RETURNING; whether the engine honors a pawn-only write is confirmed in-game on
/// first real use.
pub async fn grant_intel(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
    amount: i64,
) -> Result<IntelGrantResult> {
    let mut state = pg.dedicated_client(namespace).await?;
    let tx = state
        .client_mut()
        .transaction()
        .await
        .context("starting intel grant transaction")?;

    let rows = tx
        .query(INTEL_RESOLVE_SQL, &[&fls_id])
        .await
        .context("resolving player for intel grant")?;
    if rows.is_empty() {
        return Ok(IntelGrantResult::PlayerNotFound);
    }
    if rows.len() > 1 {
        return Ok(IntelGrantResult::Ambiguous);
    }

    let row = &rows[0];
    let pawn_id: Option<i64> = row.try_get(0).ok().flatten();
    let online_norm: String = row.try_get(1).unwrap_or_default();
    if online_norm != "offline" {
        return Ok(IntelGrantResult::PlayerOnline(online_norm));
    }
    let Some(pawn_id) = pawn_id else {
        return Ok(IntelGrantResult::CharacterActorMissing);
    };

    let updated = tx
        .query_opt(INTEL_UPDATE_SQL, &[&pawn_id, &amount])
        .await
        .context("applying intel jsonb_set")?;
    let Some(updated) = updated else {
        return Ok(IntelGrantResult::CharacterActorMissing);
    };
    let new_points: i64 = updated.try_get(0).context("reading new tech points")?;

    tx.commit().await.context("committing intel grant")?;
    Ok(IntelGrantResult::Granted { new_points })
}

// Specialization XP lives in dune.specialization_tracks, one row per
// (player_id, track_type) where player_id is the CHARACTER actor id — the same
// `player_state.player_pawn_id` we resolve for Intel. The engine's AwardXP MQ
// command ignores Category (live-tested), so per-track XP can only be set by a
// guarded offline DB write.
//
// Resolve the pawn id + online status, locking player_state, identically to the
// Intel path. (verified schema 2026-06-11)
const SPEC_XP_RESOLVE_SQL: &str = "
SELECT ps.player_pawn_id,
       lower(btrim(COALESCE(ps.online_status::text, ''))) AS online_norm
FROM dune.player_state ps
JOIN dune.encrypted_accounts enc ON enc.id = ps.account_id
WHERE enc.\"user\"::text = $1
FOR UPDATE OF ps
";

// UPSERT, ADD semantics. The table starts empty for a player, so a plain UPDATE
// would match 0 rows — INSERT ... ON CONFLICT covers both first grant and
// top-up. Notes:
//  - $2 is forced through `::text::enum` so tokio_postgres binds it as text and
//    Postgres casts to the enum (a bare `$2::enum` would bind as the enum OID,
//    which a Rust &str can't encode);
//  - `level` is set to 0.0 only on first insert and never touched on update —
//    the engine recomputes level from xp on next login;
//  - track_type is whitelisted by the caller, so the cast can't hit Invalid/Count.
const SPEC_XP_UPSERT_SQL: &str = "
INSERT INTO dune.specialization_tracks (player_id, track_type, xp_amount, level)
VALUES ($1::int8, $2::text::dune.specializationtracktype, $3::int4, 0.0)
ON CONFLICT (player_id, track_type)
DO UPDATE SET xp_amount = dune.specialization_tracks.xp_amount + EXCLUDED.xp_amount
RETURNING xp_amount::int4
";

/// Outcome of a specialization-XP grant. Non-`Granted` variants are expected,
/// recoverable states the handler turns into clear messages; `Err` is reserved
/// for actual DB faults.
pub enum SpecXpGrantResult {
    Granted { new_xp: i32 },
    /// No `player_state` row for the FLS id.
    PlayerNotFound,
    /// More than one `player_state` row matched — refuse rather than guess.
    Ambiguous,
    /// Player is not strictly offline. Carries the normalized status we saw.
    PlayerOnline(String),
    /// Pawn id was NULL — the player has no resolvable character actor to key
    /// the track row on, so we refuse rather than invent one.
    CharacterActorMissing,
}

/// Grant specialization XP to one track by UPSERTing `dune.specialization_tracks`
/// (ADD semantics on `xp_amount`). A guarded offline DB write, mirroring
/// `grant_intel`:
/// - one transaction: resolve + `FOR UPDATE` the player_state row, then UPSERT;
/// - refuses unless strictly offline (the engine overwrites DB edits on logout);
/// - touches only `xp_amount` on conflict; never `level` (engine recomputes it);
/// - fails closed on unknown / ambiguous FLS id or a NULL pawn.
///
/// The caller (HTTP handler) whitelists `track_type` against the valid enum
/// strings and clamps `amount`; this function trusts neither beyond the guards
/// it can verify here.
pub async fn grant_spec_xp(
    pg: &PgClient,
    namespace: &str,
    fls_id: &str,
    track_type: &str,
    amount: i32,
) -> Result<SpecXpGrantResult> {
    let mut state = pg.dedicated_client(namespace).await?;
    let tx = state
        .client_mut()
        .transaction()
        .await
        .context("starting spec-xp grant transaction")?;

    let rows = tx
        .query(SPEC_XP_RESOLVE_SQL, &[&fls_id])
        .await
        .context("resolving player for spec-xp grant")?;
    if rows.is_empty() {
        return Ok(SpecXpGrantResult::PlayerNotFound);
    }
    if rows.len() > 1 {
        return Ok(SpecXpGrantResult::Ambiguous);
    }

    let row = &rows[0];
    let pawn_id: Option<i64> = row.try_get(0).ok().flatten();
    let online_norm: String = row.try_get(1).unwrap_or_default();
    if online_norm != "offline" {
        return Ok(SpecXpGrantResult::PlayerOnline(online_norm));
    }
    let Some(pawn_id) = pawn_id else {
        return Ok(SpecXpGrantResult::CharacterActorMissing);
    };

    let upserted = tx
        .query_one(SPEC_XP_UPSERT_SQL, &[&pawn_id, &track_type, &amount])
        .await
        .context("upserting specialization track xp")?;
    let new_xp: i32 = upserted.try_get(0).context("reading new spec xp")?;

    tx.commit().await.context("committing spec-xp grant")?;
    Ok(SpecXpGrantResult::Granted { new_xp })
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
