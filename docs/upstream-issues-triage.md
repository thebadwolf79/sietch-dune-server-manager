# Upstream Open-Issue Triage

> Snapshot of open issues on `adainrivers/dune-dedicated-server-manager` with a
> first-pass solution **direction** for each (not full designs — we dig in properly
> later). Cross-refs point at our own [`improvement-ideas.md`](./improvement-ideas.md).
>
> Context: the maintainer (adainrivers) is **active and AI-assisted** (replies sign off
> "Hi, I'm Claude, Adain asked me to reply on his behalf. Banana!"). Several issues are
> already partly shipped — coordinate before duplicating.

---

## Clusters / recurring themes

These tie multiple issues together — fixing the root theme knocks out several at once:

- **A. Read richer status from the Director/BGD endpoints** — the manager derives status
  from local k8s/DB state, but Funcom's own tool reads the **Director (BGD)** for health,
  phase, uptime, and grace-period data. Affects **#21** (wrong phase/health/uptime) and
  **#14** (grace-period players). One integration (consume BGD player/health endpoints)
  addresses both + improves accuracy generally.
- **B. Auto-refresh / auto-update toggles + state-aware gating** — persist toggle states,
  and **stop polling when the battlegroup is down** so the UI doesn't freeze on dead
  endpoints. Affects **#25**, **#22**, **#10**. Same "be aware of server state" idea as
  our [improvement-ideas §3.6](./improvement-ideas.md) availability gating.
- **C. Backup/dump failure: better surfacing + real root causes** — failures show an
  opaque "dump operation failed; run kubectl describe." Capture & display the dump pod's
  actual logs/exit reason, and handle the two real causes: **stray non-`dune` tables
  (exit 1 — our §2.3)** and **OOM (#23)**. Affects **#7**, **#23**.
- **D. Admin "grant" surface** — a unified, engine-validated granting UI: items (§3.2),
  currency (§3.1), and **specialization XP (#16)**. Avoid raw `properties` edits.

---

## Issues

### #25 — [Bug 0.3.16] Auto Update toggle won't stay off — **OPEN**
- **Two problems:** (1) the "Auto Update" toggle on the User screen reverts to **on** after
  navigating away and back — state isn't persisted (or the read re-defaults to `true`);
  (2) when the BG is shut down, the user-list auto-refresh keeps hitting a dead endpoint
  and **briefly freezes the app**.
- **Direction:** (1) persist the auto-update flag to the schedule/config and read it back
  (note: #10 added auto-update/-backup/-restart switches in v0.3.14 — this may be a
  regression or a separate User-screen control not wired to the persisted flag). (2) gate
  polling on BG-running state (theme **B**) and make the call non-blocking with a short
  timeout so a dead endpoint never freezes the UI.
- **Effort:** low–medium. Good first fix.

### #24 — [Bug] `battlegroup update` reports failure on success — **OPEN (fix already in our fork)**
- Root cause confirmed (non-idempotent `ln -s` in `system.sh` → exit 1 even on success).
  Our fork treats exit 1 as OK. **Action: turn the fork fix into a PR.** Cross-ref §2.2.

### #23 — [NotVerified Bug] DB Backup OOMKilled — **OPEN (not ours; did not reproduce)**
- wofnull's: dump pod runs ~600 s then `OOMKilled` (exit 137). Distinct from our exit-1
  stray-table failure. **Direction:** raise dump pod memory limit / stream-compress.
  Cross-ref §2.1. (Leaving the thread alone — we have no OOM data to add.)

### #22 — [Feature] Auto-refresh the Battlegroup info panel — **OPEN**
- Wants the BG info/status panel to auto-refresh like the players list does.
- **Direction:** reuse the existing players-list auto-refresh pattern (toggle + interval)
  for the BG status panel; gate on BG-up (theme **B**). Low effort.

### #21 — [Bug 0.3.16] Battlegroup Info wrong vs Funcom tool — **OPEN**
- Three mismatches: (1) "server uptime" shows **BG** uptime, not the **pod** uptime;
  (2) Phase shows "Running" while it's really the **Startup** phase; (3) Gateway shows
  "Initializing"/"Running" where Funcom shows **"Healthy."**
- **Direction (theme A):** read per-pod `status.startTime` for uptime; distinguish
  startup vs running and map gateway state to Funcom's health semantics — ideally by
  pulling from the **Director/BGD** health/phase endpoints rather than raw pod phase.
- **Effort:** medium (depends on BGD endpoint integration).

### #18 — Game Server Update doesn't appear to work — **OPEN (owner says fixed in v0.3.15)**
- Owner: faulty "already applied" guard; fixed in v0.3.15. **Action:** confirm with
  reporter / likely close. No code work expected.

### #16 — [Feature] Grant Specialization XP (Combat/Crafting/Gathering/Exploration/Sabotage) — **OPEN**
- Grant XP/levels per specialization (great for onboarding players from public servers).
  **Directly aligns with our grant tooling (theme D / §3.1–3.2).**
- **Direction:** first find the safe mechanism — is there an engine admin command (à la
  `AddItemToInventory`/`AddExperience`) for specialization XP? If yes, expose via the MQ
  `/api/admin/publish` path with a specialization picker + amount. If not, locate where
  it's stored (candidate tables seen in DB: `specialization_tracks`,
  `purchased_specialization_keystones`, `specialization_refund_id`, or in
  `actors.properties`) — but **prefer the engine path; avoid raw `properties` edits**
  (incident lesson). **Strong testbed candidate.**
- **Effort:** medium; needs in-game validation first.

### #14 — [Feature] Users tab additional info — **OPEN (partly shipped)**
- Owner shipped auto-refresh (v0.3.14). Remaining: show players still in the **grace
  period** after logoff — not in the local game DB; needs the **Director player
  endpoints** (theme A).
- **Direction:** integrate the BGD player endpoint; surface the grace-window group.

### #10 — [Feature] Fine-tune scheduled restarts — **OPEN (partly shipped)**
- Owner shipped an on/off switch (v0.3.14). Remaining: full **cron string** for restart
  time (mirror the backup-cron field), with migration from the existing hour/minute pair.
  Owner already specced it (`restart_cron` on `ScheduleConfig` + UI + migration).
- **Direction:** implement to the owner's stated plan; e.g. `0 6 * * 1-5` to skip weekends.

### #7 — backup failed (exit 1, "dump operation failed") — **OPEN**
- `battlegroup backup … failed (exit 1): ERROR: dump operation … failed.` In comments the
  reporter found the dump operation came back **NotFound** (cleaned up before describe).
- **This is our §2.3 family, not #23** (exit 1, not OOM). **Direction (theme C):**
  (1) make the manager **capture and display the dump pod's logs/exit reason** on failure
  instead of telling the user to run `kubectl describe`; (2) check for the **stray
  non-`dune` table → `pg_dump` permission-denied** cause (our finding) and the
  cleanup-race that makes the op NotFound. Good place to contribute our §2.3 analysis.
- **Effort:** medium (error-surfacing) + the §2.3 dump hardening.

---

## Suggested order to tackle (tomorrow)

1. **#24** — open the PR (fix already done in fork). Easy win, builds rapport.
2. **#7 + §2.3** — dump-failure surfacing + stray-table hardening (we have first-hand data).
3. **#25 / #22 / theme B** — toggle persistence + state-aware polling/auto-refresh.
4. **#16** — specialization-XP grant (validate mechanism on the testbed; pairs with §3.1/3.2).
5. **#21 / #14 / theme A** — Director/BGD status integration (bigger; do together).
6. **#10** — restart cron (owner already specced it).
7. **#18** — confirm fixed, nudge to close.
