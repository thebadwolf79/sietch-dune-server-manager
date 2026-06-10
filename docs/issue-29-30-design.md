# Design notes: #29 (grant currency) and #30 (DR / world-identity backup)

Working notes on `feat/vm-power-on` (will move to dedicated branches when implemented).
These two are larger features whose *full* implementation needs live validation
(an offline player + DB for #29; a controlled reinstall for #30), so this captures
the investigation + design so the build is de-risked and actionable.

## #29 — Grant currency (Solari / House Scrip)

### Investigation (code-grounded, 2026-06-10)
- The engine admin-command catalog (`crates/dune-server-service/src/admin/commands/specs.rs`)
  exposes: **AddItemToInventory** ("Grant item"), **AwardXP** (player+amount),
  **CheatScript** (live-tested no-op), **ServiceBroadcast**. **There is no currency
  command**, and `player_virtual_currency_balances` has zero references in the service.
- Note for #16: `AwardXP` **ignores Category** — the server "always grants generic
  player XP regardless of which value is sent" (live-tested, per the specs.rs comment).
  So specialization XP is **not** grantable via the MQ command path; it would also be a
  DB write. (Journey commands were similarly removed — handlers don't apply state.)
- **Conclusion:** currency can't be granted through a known engine command. Path is a
  **guarded offline DB write**. There *is* DB-write precedent in
  `crates/dune-server-service/src/postgres/queries.rs` (`INSERT INTO dune.items …`),
  so the Postgres write capability already exists to build on.
- **Three distinct spendables, three distinct mechanisms** (operator notes +
  `dune-awakening-server/UPSTREAM-ISSUE-DRAFT.md`):
  - **Solari (on-hand)** — an **item** (`solari` / `SolarisCoin`); already grantable via
    `AddItemToInventory`. No currency path; don't duplicate it.
  - **House Scrip** — a real currency row in `dune.player_virtual_currency_balances`
    (`currency_id 1`, keyed by `player_controller_id`). Bank Solari = `currency_id 0`,
    same table. Offline DB write (UPSERT).
  - **Intel ("Tech Knowledge points")** — **not** a currency row. Stored in
    `dune.actors.properties` (JSONB) at `TechKnowledgePlayerComponent.m_TechKnowledgePoints`
    (integer), on the **character actor** (`BP_DunePlayerCharacter_C`, by class — keyed by
    the **character actor id**, not `player_controller_id`). Granted today via a targeted
    `jsonb_set` (offline). **No engine command exists** — `AwardIntel` is only a *draft*
    (for `Icehunter/dune-admin`), so the engine path is unavailable until Funcom adds a handler.

### Design
1. **Backend — two write paths** (both offline, via the existing Postgres layer):
   - *House Scrip / bank Solari:* UPSERT `dune.player_virtual_currency_balances`
     (key `player_controller_id` + `currency_id`).
   - *Intel:* targeted `jsonb_set(properties, '{TechKnowledgePlayerComponent,
     m_TechKnowledgePoints}', <amount>)` on `dune.actors` for the player's **character
     actor id**. This touches the `actors.properties` blob, so apply the §3.5 guardrails:
     single-integer set only, never round-trip/restructure the blob (the incident lesson),
     verify the actor id, player offline.
2. **Online-state guard (required):** the server overwrites DB edits on logout, so the
   write must refuse while the player is online. Reuse the player-presence read in
   `admin/players.rs` (the same source as `ms_player_location` / `ms_search_players`)
   to check, and return a clear "player must be offline" error otherwise.
3. **UI — dedicated Admin-tab buttons (operator preference), modeled on the existing
   "water" command (`UpdateAllWaterFillables`):** give each spendable its **own** button so
   nobody has to scroll the `AddItemToInventory` list to find Solari. Three buttons, each
   with a player picker + amount:
   - **Grant Solari** → reuses `AddItemToInventory` with `ItemName=solari` (the MQ publish
     path) — exposed as a dedicated entry/prefill, not buried in the item list.
   - **Grant House Scrip** → new management-service **DB write** (currency row, `id 1`).
   - **Grant Intel** → new management-service **DB write** (`actors.properties` `jsonb_set`).
   - **Architecture note:** the current AdminTab command list is **MQ-publish only**
     (`managementApi.publish` → `CommandSpec`s in `specs.rs`). Solari fits that path; House
     Scrip + Intel are **not** MQ commands, so they need new management-service endpoints +
     a distinct "DB grant" button kind in AdminTab (don't shoehorn them into the publish
     list). All three surface the offline-state guard for the DB-write ones.
4. **Pre-flight (live server):** re-confirm House Scrip = `currency_id 1` (fingerprint
   trick) and resolve the Intel **character actor id** (not the controller id). Intel's
   storage + `jsonb_set` path are already verified in `UPSTREAM-ISSUE-DRAFT.md`. If Funcom
   ever ships an `AwardIntel`/currency engine command, prefer it over the DB writes.

### Why not this session
The DB write can't be safely end-to-end verified without a live DB + an offline test
character. Query-building is unit-testable; the integration is not. Implement on a
`feat/grant-currency` branch with the live server available.

---

## #30 — Full-stack DR backup (survive a from-scratch reinstall)

### The gap (lived it)
After a clean reinstall, a DB backup *retains the character* but it can't be brought into
the new battlegroup — the clean install mints a **new world identity**, orphaning the
saved character. (Concretely observed this session: the live battlegroup id is
`sh-431c7b16e03f3f97-jlbdmm` vs the backups' `…-iyyivz` — a different world generation.)

### Identity layer to capture (from `world_creation.rs`)
A fresh world generates, under `/home/dune/.dune/`:
- `WORLD_UNIQUE_NAME` → battlegroup name + `funcom-seabass-<id>` namespace
- `FLS_TOKEN` → `fls-secret.yaml` (Funcom Live Services registration — most likely the
  binding identity)
- `RMQ_SECRET`, `WORLD_DUNE_PASS`, `WORLD_POSTGRES_PASS` → **regenerated every clean
  install** (new secrets ≠ what the old DB expects)
- the world manifest YAML (BattleGroup CRD) + `.manager-bootstrap-world-name`

### Design
1. **Config-capture backup:** alongside the DB dump, archive the manifest YAML, FLS + RMQ
   secrets, DB passwords, `WORLD_UNIQUE_NAME` + namespace, and the bootstrap marker —
   **encrypted at rest** (credentials/tokens; never commit).
2. **Identity-preserving restore:** on a clean reinstall, **bypass** the regenerate-secrets
   branch of world-creation and re-apply the captured manifests/secrets verbatim
   (same name, namespace, FLS registration, DB creds) **then** `battlegroup import` the dump.
3. **Restore preflight / drift check:** before import, compare captured identity vs the
   live install and warn loudly on any mismatch (different world name / FLS token /
   regenerated secrets) — that mismatch *is* the orphaning bug.

### The gating first step (empirical — can't guess)
**Which identifier does the character actually bind to?** Run a controlled reinstall that
**reuses `WORLD_UNIQUE_NAME` + `FLS_TOKEN`** but lets RMQ/DB secrets regenerate, then
`battlegroup import` the dump and see if the character loads:
- loads → FLS identity (+ name) is sufficient; secrets don't bind the character.
- doesn't load → the secrets must be captured/replayed too.
Confirm the k8s CRD `metadata.uid` is **not** the binding key (it changes on recreate).
Perplexity could not find public docs pinning this, so the experiment is required before
building the restore path.

### Why not this session
The binding-identifier experiment needs a deliberate reinstall on a throwaway world (the
exact event that risks the live character). Sequence it as its own session with a
disposable test world; capture the finding here, then implement on `feat/dr-backup`.
