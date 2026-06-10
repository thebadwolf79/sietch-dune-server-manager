# Feature: VM power-on + lifecycle state machine (working spec)

Branch `feat/vm-power-on`. Implements upstream issue #28. Synthesizes the role-panel design
(architect/risk/mechanism) against the actual code in this repo. Working notes, not upstream docs.

## What already exists (confirmed by reading the code)
- `crates/dune-manager-core/.../orchestration/hyperv_lifecycle.rs` —
  `HyperVVmLifecycleOrchestrator<V: VmProvider>` with `start(name, sink)` / `stop(name, sink)`.
- `.../orchestration/providers/vm_provider.rs` — `VmProvider` trait: `get_vm(name) ->
  Option<VmInventoryRecord>` (state detection), `list_vms()`, `start_vm`, `stop_vm`, etc.
- `.../orchestration/dune_vm_detection.rs` — VM auto-discovery by fingerprint.
- `crates/dune-server-service/src/kubectl/mod.rs` — **`ProcessResult { exit_code, stdout, stderr }`
  with `ok()` + `require_ok()`** and a generic `run_process()`. This is already the
  `CommandOutput` the review (§A1) called for — the abstraction is half-built.
- `app/src-tauri/src/commands/` — `battlegroup.rs`, `status.rs`, `discovery.rs`, `preflight.rs`,
  `management_service.rs`, … but **no `vm.rs`** → VM power-on is never surfaced. This is the gap.

## Scope of this feature (small, because the primitives exist)
1. **App command layer:** add `app/src-tauri/src/commands/vm.rs` exposing `vm_get_state`,
   `vm_start`, `vm_stop`, registered in `lib.rs`'s `generate_handler!`. These call the existing
   `HyperVVmLifecycleOrchestrator` / `VmProvider` for the host-side Windows provider.
2. **State model (from the architect design):** a serializable `SystemState` enum owned in Rust:
   `Unknown · HostPermissionUnavailable · Error{msg} · VmOff · VmSaved · VmPaused ·
   VmStarting{step} · VmRunning · BattlegroupStopped · BattlegroupStarting{step} ·
   BattlegroupHealthy · BattlegroupDegraded{reason} · BattlegroupStopping{step}`.
   Authority in Rust; React renders + gates actions; push changes via a Tauri event
   `system-state-changed` (events over polling — confirmed idiomatic for Tauri v2).
3. **Start flow:** `Start-VM` → poll `get_vm` until `Running` → wait IP → wait SSH (reuse the
   existing readiness checks) → emit progress at each step via `VmStarting{step}` so the UI never
   freezes. Long op runs on `tauri::async_runtime` with shared state behind a `Mutex`.
4. **Host-vs-remote detection:** probe `Get-Module -ListAvailable Hyper-V`; if absent/denied →
   `HostPermissionUnavailable` / connect-only mode, VM power buttons disabled with a reason.

## Risk/QA matrix to satisfy (from Grok) — assert these in tests via a mock `VmProvider`
- P1: non-elevated → `Get-VM`/`Start-VM` access-denied ⇒ surface "Run as Administrator", disable
  controls, no silent retry. (We hit this live: non-elevated `Get-VM` = access denied.)
- P1: VM name mismatch ⇒ never start an unknown VM; require exact match / explicit config.
- P1: `Start-VM` ok but no IP within timeout ⇒ timeout, cancelable, → `Error`.
- P2: IP up but sshd not ready ⇒ separate SSH-readiness probe; keep BG actions gated.
- P2: double-click Start ⇒ single in-flight guard (idempotent).
- P2: Stop while a battlegroup op is in flight ⇒ block/abort cleanly, never cut a live SSH op.
- P3: Saved/Paused/Starting/Critical/missing ⇒ correct button state; only Start on Off/Saved/Paused.
- P3: Hyper-V disabled ⇒ detect at startup, graceful message, no cmdlet attempts.
- P4: remote mode (not on host) ⇒ power controls disabled with explanation.
- Cross-cutting: every cmdlet preceded by a permission check; structured errors to the frontend
  (never raw PowerShell text); BG actions allowed only when VM=Running + SSH ready.

## Command-runner refactor (review §A1–A2) — do alongside, it's smaller than thought
- Extract a `CommandRunner` trait over the existing `run_process` / `KubectlClient`, returning the
  existing `ProcessResult`; add a `MockCommandRunner` for tests.
- Change `BattlegroupCli::update()` from `require_ok` to: tolerate exit 1, then **verify** via the
  already-tracked `battlegroupVersion` vs `liveBattlegroupVersion` (state, not exit code). Same
  pattern upstream used for `restart` (#20). This removes the scattered exit-code hack and makes
  the wrapper unit-testable — and it's the architecture of the "wrap, don't replace" philosophy.

## Currency grant (#29) — mechanism note (from ChatGPT)
Prefer an engine/admin command if one exists (mirror `AddItemToInventory` via the MQ
`/api/admin/publish` path, engine-validated, online-only). If none exists, fall back to a guarded
**offline** write to `player_virtual_currency_balances` (refuse while the player is online; named
currency picker; log the change). Investigate the engine-command path first.

## DR/world-identity backup (#30) — open question first
Perplexity could not find public docs pinning the character→world binding key. So step 1 is the
controlled experiment: reinstall reusing `WORLD_UNIQUE_NAME` + `FLS_TOKEN` with fresh RMQ/DB
secrets, import the dump, see if the character loads — isolates whether FLS identity alone binds it.
Build the identity-capture backup after that's known.

## Next code steps
1. Read the Windows `VmProvider` impl + how `app/src-tauri` holds/accesses orchestrators and app
   state (Tauri `State`), and `lib.rs` handler registration.
2. Add `SystemState` (serde) in core; add `commands/vm.rs` + register; emit `system-state-changed`.
3. Wire the start flow with progress events; add the host-vs-remote probe.
4. Extract `CommandRunner` trait + mock; switch `update()` to verify-state; add unit tests.
