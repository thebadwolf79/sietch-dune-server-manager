# Fork Improvement Ideas, Lessons & Roadmap

> Personal planning doc for this fork of `adainrivers/dune-dedicated-server-manager`.
> Born out of the 2026-06-07/08 character-corruption incident (full operational
> runbook lives in the private `dune-awakening-server/OPERATIONS.local.md`).
> Nothing here is committed upstream unless explicitly turned into a PR.

---

## 0. Incident in one paragraph (the "why" behind most of this)

A bulk "unlock every cosmetic" operation hand-edited and JSON round-tripped the
player's `dune.actors.properties` blob, and a follow-up script force-stamped
`map='Arrakeen'` onto the row. That scrambled the blob's internal structure and
desynced `map`/`partition_id` from the saved `transform`. Result: the map-server
pod `SIGSEGV`'d while loading the character (right after `Login [Success]`), which
the client sees as a **`p34` "connection lost."** Targeted single-row fixes weren't
enough (the corruption spanned many interdependent per-character tables); the
reliable cure was a **full `battlegroup import` of the last good scheduled dump**.
After recovery, the schematics and House Scrip the user actually wanted were
re-applied the *safe* way (engine-validated `AddItemToInventory` + a careful
currency write).

---

## 1. Lessons learned (these shape the feature priorities)

1. **Never hand-edit the serialized `actors.properties` blob.** It's an
   engine-serialized structure with cross-references (tech ↔ recipes ↔ journey ↔
   customization). `json.loads → mutate → json.dumps`, appending hundreds of
   entries, or overwriting sub-components breaks invariants → load-time SIGSEGV.
2. **Persisted character state spans many tables**, not just `actors`. Journey
   (`mnemonic_recall`, `journey_story_node`), tags (`player_tags`), specialization,
   `building_progression`, currencies, etc. A partial restore leaves cross-table
   mismatches that still crash. → For structural problems, full import beats
   surgery.
3. **Grants belong on engine-validated paths**, not raw DB writes. `AddItemToInventory`
   (MQ admin command) adds items safely; it only applies to **online** characters.
4. **DB edits require the player offline**, or the server overwrites them on logout.
5. **Blob size matters.** An 834 KB blob *loaded* fine but crashed on **map travel**
   (player state is replicated server-to-server with a buffer limit). Bulk unlocks
   that balloon `properties` break zone transitions even when login works.
6. **`map`/`partition_id`/`transform` must stay mutually consistent**, or the player
   falls through the world / gets unstable spawns.
7. **Always have a fresh backup before editing.** The scheduled 4 h dumps saved us;
   the on-demand `battlegroup backup` failed right when we needed it — but **not** from
   OOM (that's the separate upstream #23): ours died instantly (exit 1) because
   `pg_dump` couldn't lock leftover non-`dune` scratch tables (see §2.3).
8. **Diagnose from the map-server crash log + a known-good baseline**, not from scary
   warnings. The `m_PersistentName is Empty!` warning was a red herring (chronic,
   present even on the working character).
9. **A throwaway DB is a great surgical tool** — restore a dump into `dune_check`,
   read/extract the rows you need, copy into live; no destructive full import.

---

## 2. Bug fixes

### 2.1 DB backup dump pod `OOMKilled` (upstream issue #23) — **MEDIUM (upstream; we did NOT reproduce this)**
- **Not ours — don't conflate.** This is wofnull's report: the dump pod runs the full
  ~600 s and then gets `OOMKilled` (exit **137**). We could **not** reproduce it — our
  backups normally finish in seconds, and our own mid-incident backup failure was a
  *different* cause (exit **1**, instant — see §2.3). Kept here only to support the
  upstream fix and to keep the two failure modes distinct.
- **Likely cause (theirs):** the dump job's memory limit is too low for `pg_dump` on a
  larger DB (or pg_dump buffering).
- **Fix options:** raise the dump pod's memory request/limit; or stream/compress.
- **Where:** the `DatabaseOperation` / dbutil dump pod spec.
- **Triage tip:** check the failed pod's exit code/reason — **137 / OOMKilled** = this
  (memory); **1 / instant** = the stray-table issue in §2.3.

### 2.2 `battlegroup update` exit-code false failure (issue #24) — *already in fork*
- Update exits `1` on success (non-idempotent `ln -s` in vendor script). Already
  patched here to treat exit 1 as OK. Keep regression coverage.

### 2.3 `pg_dump` aborts on non-`dune`-owned tables — backup **and** import (exit 1) — **HIGH (this is what actually bit us)**
- **Symptom:** both `battlegroup backup` (`dumpdb.py`) and `battlegroup import`'s
  safety pre-dump run `pg_dump`, which `LOCK`s **every** table. Any stray table not
  owned by `dune` (e.g. `temp_backup_*` left from manual DB poking, owned by
  `postgres`) → `ERROR: permission denied for table …` → the dump **fails instantly
  with exit 1**. This broke our on-demand backup *and* our first import attempt
  mid-incident. **Distinct from §2.1's OOM** (different exit code; instant vs ~600 s).
- **Fixes:** run the dump as the table owner/superuser; OR scope it to the known
  game-schema tables; OR detect stray/non-game tables and surface a clear, actionable
  error ("drop these N non-game tables first") instead of the opaque permission error.
- *Silver lining to preserve:* the import abort happened **before** any data change, so
  the DB stayed intact. The "snapshot-then-restore, fail-safe" ordering is good — keep it.

---

## 3. New features

### 3.1 Grant **currency** (House Scrip, Solari, …) from the UI — **HIGH (user-requested)**
- Today the manager can grant **items** but **not currency**; the only path is a raw
  DB write to `dune.player_virtual_currency_balances`, which (a) requires the player
  offline and (b) uses **opaque numeric `currency_id`s** with no name lookup.
- **Empirically-mapped currency IDs on this stack** (capture in data/config):
  - `0` = **Solari** (bank balance; on-hand Solari is a separate `SolarisCoin`
    inventory item, not a currency row)
  - `1` = **House Scrip**
  - `2–5` = no real/displayed currency (unused on this build)
- **Build:** a "Grant Currency" admin action with a **named** currency picker, player
  picker, amount, and an **online-state guard**. First investigate whether the engine
  exposes an admin command for currency (like `AddItemToInventory`); if so, route it
  through the existing MQ `/api/admin/publish` path (preferred — engine-validated). If
  not, provide a safe offline DB-write helper that refuses to run while the player is
  online.

### 3.2 **Filterable** item / inventory-grant picker — **HIGH (user-requested)**
- Make the "add item to inventory" flow a searchable, filterable catalog instead of
  raw template-name entry. Filters:
  - **Type / category:** weapon, armor, stillsuit, tool, consumable, vehicle module,
    **schematic**, customization, building, currency. `items.json` already carries a
    `category` field — extend/normalize it.
  - **Technology / metal tier:** e.g. Tier 1–6 (the in-game "Iron products" = Tier 2,
    etc.). Today this is only inferable from id naming (`_01_`…`_06_`,
    `…Unique02…`). **Add an explicit `tier` field to `items.json`** (and a
    `unique: true/false` flag) so filtering is reliable rather than regex-on-names.
  - Free-text name search; unique-only toggle.
- **UX:** multi-select rows + per-row quantity + durability, then a single batch grant
  via `AddItemToInventory`. This directly supports "give me all Iron uniques as
  schematics" — exactly the manual workflow from the incident (3× consumables, 1× the
  rest). Bonus: a "grant as schematic vs. as built item" toggle.

### 3.3 **Character / player admin panel** — **HIGH**
- Read-only view first: online status, current `map`/`partition_id`, `properties`
  **size** (with a warning threshold well under the S2S replication limit), faction,
  currencies, learned-schematic count, last save time.
- Safe actions: **relocate character** (set `map`+`partition_id` *consistently* with
  the transform — the exact fix for stuck / fall-through-the-world characters),
  force-offline, and a "validate blob" check (parse + sanity-check component set).

### 3.4 **Backup & restore UX** — **HIGH**
- One-click **safety backup** (once 2.1 is fixed) before any destructive action.
- **Backup browser:** list scheduled + manual dumps with timestamps/sizes; pick one.
- **Targeted restore:** restore a single character's/account's rows from a chosen
  backup using the throwaway-DB technique (`pg_restore` into `dune_check`, copy the
  rows into live) — recover one player without a destructive full-world import.
- Make `import` clearly flagged destructive + auto-handle the stop/drain/start dance.
- **Scope caveat:** everything above assumes the *same* battlegroup identity. A DB
  dump alone does **not** survive a from-scratch reinstall — see §3.7.

### 3.7 **Full-stack disaster-recovery backup (survive a from-scratch reinstall)** — **HIGH (user-requested)**
- **The gap (lived it):** after uninstalling and reinstalling the self-hosted server
  from Steam, the DB backup *retained the character* but that character **could not be
  brought into the new battlegroup** — it was bound to the *original* battlegroup's
  identity, and the clean install minted a new one. A world/DB dump is necessary but
  **not sufficient** for true disaster recovery.
- **Root cause — identity is regenerated on every clean install.** In
  `guest_bootstrap_ssh/world_creation.rs::create_world_script`, a fresh world mints:
  - `WORLD_UNIQUE_NAME` → battlegroup **name** + namespace `funcom-seabass-<unique>`
    (deterministic *only* if you deliberately reuse the same unique name).
  - `FLS_TOKEN` → `fls-secret.yaml` — the world's **Funcom Live Services** registration
    identity (the most likely thing the saved character is actually bound to).
  - `RMQ_SECRET`, `WORLD_DUNE_PASS`, `WORLD_POSTGRES_PASS` → **freshly `openssl rand`'d
    every clean install** (lines 85–87). New secrets ≠ the ones the old DB expects.
  - The full **world manifest YAML** (the BattleGroup CRD spec) + `.manager-bootstrap-world-name`.
  All of this lives on the VM under `/home/dune/.dune/` (`$WORLD_UNIQUE_NAME.yaml`,
  `$WORLD_UNIQUE_NAME-fls-secret.yaml`, `$WORLD_UNIQUE_NAME-rmq-secret.yaml`).
- **Build — a "Full configuration backup / restore" that captures the identity layer,
  not just the world:**
  1. **Config-capture backup:** alongside the DB dump, archive the world manifest YAML,
     the FLS + RMQ secrets, the DB passwords (from the manifest / k8s Secrets), the
     `WORLD_UNIQUE_NAME` + namespace, and the `.manager-bootstrap-world-name` marker.
     Pull secrets via the existing `secret_value()` helper
     (`kubectl/battlegroup.rs`) rather than re-reading files where possible. **Encrypt
     at rest** — these are credentials/tokens (mirror how `OPERATIONS.local.md` is kept
     private; never commit them).
  2. **Identity-preserving restore path:** on a clean reinstall, *bypass* the
     regenerate-secrets branch of world-creation and instead **re-apply the captured
     manifests/secrets verbatim**, recreating the *same* battlegroup name, namespace,
     FLS registration, and DB credentials — **then** `battlegroup import` the DB dump.
     Character ↔ battlegroup binding is preserved, no mismatch.
  3. **Restore preflight / drift check:** before importing, compare the captured identity
     against the live install and warn loudly on any mismatch (different world unique
     name, different FLS token, regenerated secrets) — that mismatch *is* the bug class
     the user hit, so surface it instead of silently producing an orphaned character.
- **Open question to investigate first (don't guess in the impl):** *which* identifier is
  the binding one the saved character keys off — the FLS world/server registration
  (from the self-host token), the DB-internal battlegroup row/UID, or the generated
  secrets? Determine empirically: do a controlled reinstall reusing the same
  `WORLD_UNIQUE_NAME` + `FLS_TOKEN` but fresh RMQ/DB secrets, import the dump, and see
  whether the character loads. That isolates "name + FLS identity is enough" vs "the
  secrets matter too." Capture the finding here. (Note: the k8s CRD `metadata.uid`
  changes on recreate but is almost certainly *not* what the character binds to —
  confirm.)
- **Why it matters / why upstream-worthy:** turns "I reinstalled Windows / reinstalled
  from Steam" from a character-loss event into a one-click recover. It's the natural
  superset of §3.4 and the strongest possible version of the safety net §2.3 protects.

### 3.5 **Guardrails / safety rails** — **MEDIUM**
- Refuse (or hard-warn + require typed confirmation) on any operation that writes the
  raw `properties` blob.
- **Online-state awareness everywhere:** block/warn on DB writes while the player is
  online; show live online status next to player actions.
- **`properties` size monitor:** flag characters approaching the replication-buffer
  limit (the thing that broke map travel).

### 3.6 **Power on the VM from the tool + full Funcom-`battlegroup.ps1` parity** — **HIGH (user-requested; daily friction)**

- **Problem:** on launch the tool assumes the VM is already running and only *connects*
  (SSH/kubectl). If the Hyper-V VM is powered off, it just fails — you have to hand-run
  Funcom's `battlegroup.bat` → `battlegroup.ps1` `start-vm` first, every time.
- **Good news — the capability already exists in core:**
  `HyperVVmLifecycleOrchestrator::start()` → `VmProvider::start_vm()` (and `stop_vm`).
  It's simply **not surfaced in the app or auto-invoked**. This is mostly a wiring +
  UX job, not new orchestration.
- **Build:**
  1. **Detect VM state on launch** (`orchestration/dune_vm_detection.rs` / `Get-VM`).
     If the VM is off or unreachable, don't hard-fail — present a **"Start Server"** action.
  2. **"Start Server" flow:** `Start-VM` → **wait for readiness** (Funcom polls
     `Get-VMNetworkAdapter` for the static `192.168.200.10` until it answers, with a
     timeout — mirror that so the tool doesn't race the boot) → SSH reachable →
     optionally **ensure the battlegroup is started** → then connect.
  3. **VM-state availability gating** — mirror Funcom's `Get-VmCmdAvailability` /
     `Get-BgCmdAvailability`: when the VM isn't running, disable battlegroup actions
     with a clear *"Start the VM first"* message. (Same gating philosophy as the
     online/offline matrix in `upstream-contribution-plan.md` §4 — make the tool
     state-aware and tell the user exactly what to do.)
- **Parity audit — every `battlegroup.ps1` capability should have a tool equivalent.**
  Enumerated from the live Funcom script:

  | Funcom command | Layer | Tool status (to audit/fill) |
  |---|---|---|
  | `start-vm` (`Start-VM` + wait for IP) | host / Hyper-V | core has `start_vm` → **wire into app** ← *the gap* |
  | `stop-vm` (`Stop-VM -Force`) | host / Hyper-V | core has `stop_vm` → wire into app |
  | `status` | battlegroup (on VM) | likely present — confirm |
  | `start` / `restart` / `stop` | battlegroup | present (battlegroup flows) |
  | `update` | battlegroup | present (exit-code fix already in this fork) |
  | `edit` / `edit-advanced` (YAML) | battlegroup | audit — may be missing |
  | `enable-experimental-swap` | battlegroup | audit — likely missing |
  | `backup` | database | present (can fail: OOM §2.1 / stray-table exit-1 §2.3) |
  | `import` | database | present (used in the recovery) |
  | `logs-export` / `operator-logs-export` | logs | audit — may be missing |
  | `open-file-browser` (edit ini/logs) | monitoring | audit — convenience gap |
  | `open-director` (server/travel/queue status) | monitoring | audit — convenience gap |
  | `shell-pod` (SSH into a pod) | monitoring | audit — convenience gap |
  | SSH key rotation (`Update-SshKey`) | host | audit (`battlegroup-management/vm-utilities.ps1`) |
  | set VM password (`Set-VmPassword`) | host | audit (`vm-utilities.ps1`) |

- **Reference material on disk:** Funcom's scripts are the spec —
  `…/Dune Awakening Self-Hosted Server/battlegroup-management/{battlegroup.ps1,vm-utilities.ps1}`.
  `battlegroup.ps1` already implements the start-vm-and-wait + availability gating logic
  to copy.
- **Why it's a strong upstream PR:** turns the tool into a true one-stop manager
  (cold VM → fully running server with no batch file), and the gating UX is reusable
  for every other state-sensitive action.

---

## 4. Niceties / smaller wins (tackle any rainy day)

- **Admin action audit log** (who/what/when/old→new). We'd have loved a trail during
  the incident.
- **Grant presets / "loadouts":** save a named set of items + currency to re-apply in
  one click (e.g. after a wipe). Would've turned re-granting the 31 schematics into a
  single button.
- **Currency-fingerprint identifier tool:** automate the "set IDs to 1M/2M/3M… and read
  them back in-game" trick to map opaque `currency_id`s to names on any build.
- **Item DB browser/search** over `items.json` (independent of granting).
- **Crash-log surfacing:** tail/parse the map-server `DuneSandbox_PIDX-*.log` for
  `Critical error|SIGSEGV|Fatal` and show recent crashes in the UI (we grepped these
  by hand over SSH).
- **Health dashboard:** pod restart counts, recent map-server crashes, map "Ready"
  state, DB pod status — at a glance.
- **Player respawn / journey-state inspector** (read-only) for debugging quest issues.

---

## 5. Codebase pointers (so a future session starts fast)

- **Admin commands & publish:**
  `crates/dune-server-service/src/admin/commands/{mod.rs,specs.rs,data.rs}`;
  HTTP endpoint `crates/dune-server-service/src/http/api_admin.rs`
  (`POST /api/admin/publish`). Working payload shape:
  ```json
  { "command": "AddItemToInventory",
    "fields": { "PlayerId": "<FLS hex id>", "ItemName": "<template>",
                "Quantity": 1, "Durability": 1.0, "ServerCommand": "AddItemToInventory" } }
  ```
  Publishes to the MQ; applies to **online** characters only.
- **Item catalog:** `crates/dune-server-service/data/items.json` (has `id`,
  `category`; **add `tier` + `unique`**). This is the data source for 3.2.
- **DB:** Postgres in the `*-db-dbdepl-sts-0` pod, port **15432**, schema `dune.*`.
  Currency → `player_virtual_currency_balances` (key `player_controller_id`,
  `currency_id`). Character → `actors.properties` (jsonb) + `map`/`partition_id`/
  `transform`. Per-character progression spread across `mnemonic_recall`,
  `journey_story_node`, `player_tags`, `building_progression`, `player_faction*`.
  `psql -U $POSTGRES_USER` in the pod connects as **`postgres`** (superuser).
- **Desktop app:** Tauri — `app/src-tauri` (Rust) + `app/src` (React/TS). New panels =
  React UI + Tauri commands bridging to `dune-server-service` (HTTP) and/or DB.
- **Lifecycle / backup / import:** vendor script `~/.dune/bin/battlegroup`
  (`stop`/`start`/`backup`/`import`); dump/import run as `DatabaseOperation` CRDs.
  Dumps land in `/funcom/artifacts/database-dumps/<bg>/` (scheduled every ~4 h).

---

## 6. Suggested priority order

1. **2.3** harden `pg_dump` against stray non-`dune` tables (broke our backup *and*
   import — restores the safety net; do first).
2. **3.6** VM power-on from the tool + availability gating (kills the daily batch-file
   friction; core `start_vm` already exists, so it's mostly wiring + UX).
3. **3.1** currency granting + **3.2** filterable item picker (highest user value).
4. **3.3** character admin panel + **3.4** targeted restore + **3.7** full-stack
   disaster-recovery backup (recovery tooling; do the §3.7 binding-identifier
   investigation early since it de-risks both 3.4 and 3.7).
5. **3.5** guardrails + the full §3.6 Funcom parity audit (close gaps; prevent repeats).
6. **2.1** support the upstream #23 OOM fix (not our bug, but a good contribution) +
   **§4** niceties as time allows.
