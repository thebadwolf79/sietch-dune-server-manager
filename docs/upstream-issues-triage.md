# Upstream Open-Issue Triage

> Snapshot of open issues on `adainrivers/dune-dedicated-server-manager` with a
> first-pass solution **direction** for each (not full designs — we dig in properly
> later). Cross-refs point at our own [`improvement-ideas.md`](./improvement-ideas.md).
>
> Context: the maintainer (adainrivers) is **active and AI-assisted** (replies sign off
> "Hi, I'm Claude, Adain asked me to reply on his behalf. Banana!"). Several issues are
> already partly shipped — coordinate before duplicating.

> **Refresh 2026-06-10:** re-pulled all open issues + comments. **No new issues** since the
> 2026-06-08 snapshot — still 11 open (#7, #10, #14, #16, #18, #21, #22, #23, #24, #25, #26).
> New activity: **#23** got a 3rd recurrence comment (2026-06-09) confirming the OOM is
> *intermittent/non-deterministic*, not strictly scheduled-vs-manual. Cross-fork survey added
> below. The game's **1.4.5.0** self-hosted update (2026-06-10) also landed — its native
> `change-battlegroup-ip`, per-map "Minimum Servers", and `enable-experimental-swap` features
> intersect several issues here; see [§1.4.5.0 implications](#1450-self-hosted-update-implications)
> and the companion doc `dune-awakening-server/PATCH-1.4.x-SELF-HOSTED.md`.

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

### #26 — [Feature] Landsraad Goal Amount — lower the house-completion threshold — **OPEN (new, 2026-06-08)**
- skullrazor-Vyvorant: the Landsraad house-completion goal (~70,000 points) is brutal for a
  **small community server**; wants either a lower goal *or* more contribution per quest.
  maurerk1993 (the #16 reporter) **+1'd** and explicitly asked to be able to **change it to a
  setting of our choosing**.
- **Direction — feasibility first (read-only), this is a different category from the rest of
  the tool.** Two unknowns to resolve before promising anything:
  1. **Is the Landsraad goal tunable at all on a self-hosted server?** Hunt for where the
     threshold lives — a `ServerSettings`/game `.ini`, a field on the BattleGroup CRD /
     world manifest, or a DB table — versus baked into the game build / decided FLS-side
     (in which case the manager genuinely can't touch it; say so on the thread).
  2. **If adjustable, where + how to write it safely** (prefer an engine/config path over a
     raw DB poke — same incident lesson as everywhere else).
- **If it turns out to be an editable server setting,** this becomes the first case for a
  **"server settings / game-balance editor"** surface — aligns with the `edit` /
  `edit-advanced` YAML parity gap in [improvement-ideas §3.6](./improvement-ideas.md).
- **Scope note:** gameplay-balance tuning is a new lane vs. lifecycle/admin ops — worth a
  quick word with the maintainer on whether it's in scope before building.
- **Effort:** unknown until feasibility is confirmed; likely a config-surface question, not
  code-heavy. Lowest-risk first step is the read-only investigation above.

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
- **Update (2026-06-06):** we posted a **live-repro confirmation** comment on the issue —
  caught `battlegroup update` exiting 1 while reporting success, with the exact
  `ln: …/bin/battlegroup: File exists` / `ln: …/bin/bg-util: File exists` output. So it's
  now confirmed in-vivo, not just reasoned from source — strengthens the PR.

### #23 — [NotVerified Bug] DB Backup OOMKilled — **OPEN (not ours; did not reproduce)**
- wofnull's: dump pod runs ~600 s then `OOMKilled` (exit 137). Distinct from our exit-1
  stray-table failure. **Direction:** raise dump pod memory limit / stream-compress.
  Cross-ref §2.1. (Leaving the thread alone — we have no OOM data to add.)
- **Update (2026-06-06):** reporter says it **recurred** — the scheduled 4 AM backup
  `OOMKilled` again, and a manual backup run right afterward finished fine. So it's a
  **recurring** pattern for them (idle server, scheduled run fails / manual succeeds), not a
  one-off. Still distinct from our instant exit-1, and we still can't reproduce the OOM —
  but it strengthens the case that the dump pod's memory limit is genuinely too low on
  some installs (theme **C** / §2.1).
- **Update (2026-06-09):** 3rd report — "found **no real pattern**": worked 2 days clean,
  then a scheduled run failed, an immediate **manual retry also OOM'd**, and ~an hour later a
  manual run on the same host (0 players, no server-side change) succeeded. So it's
  **intermittent/non-deterministic, not strictly scheduled-vs-manual** — manual sometimes
  OOMs too. From the pod describe in the issue body: the dump container ran **~24 min**
  (05:07:20→05:31:28) before `OOMKilled` (exit 137), while `dumpdb.py` has only a 600 s
  internal timeout — i.e. it's **memory growth**, not the timeout, that kills it, and the
  available headroom at dump time varies with whatever else the k3s node is holding.
- **New angle via 1.4.5.0:** this is the same RAM-pressure class the game's 1.4.5.0 update
  now gives levers for: **`enable-experimental-swap`** (documented escape hatch for <20 GB
  VMs) and **per-map "Minimum Servers"** (fewer warm game-server pods → more node headroom
  for the dump pod). Concrete suggestion to add to the thread (we still can't repro): have
  the reporter (a) check VM RAM vs. number of warm maps, (b) try reducing Minimum Servers on
  unused maps, and/or (c) enable experimental swap — alongside the vendor-side fix of raising
  the dump pod's memory limit / stream-compressing. This turns "can't repro" into an
  actionable, data-gathering reply.

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
- **Full root cause (owner comment, 2026-06-04):** the "already applied" guard compared the
  live build against a version file that only gets **rewritten later in the same run** — so on
  a fresh update both sides looked equal, the pending update was cleared, and the manager
  flipped back to "up to date" *without downloading or restarting*. It then re-detected the
  update ~15 min later and looped. v0.3.15 only marks an update applied once the Steam download
  has genuinely advanced **and** the BG is live on the new build. **Note:** the fix is in the
  **host service**, so users must push it via the **Management Service card → Install/Update**,
  not just update the desktop app. Reporter (TempestWales) hasn't confirmed yet — nudge to close.

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
- **Onboarding-friction note (reporter comment):** SkyDrift couldn't run the suggested
  `kubectl describe` because `hvc ssh dune-awakening` rejected the password; they only got in
  after discovering `ssh -i <privatekey> dune@<ip>`. Two takeaways: (1) the "run kubectl
  describe yourself" guidance assumes SSH access many Windows users haven't set up — another
  reason to **surface the dump pod logs in-app** rather than punting to the CLI; (2) worth a
  one-line doc/snippet giving the exact `ssh -i %LOCALAPPDATA%\DuneAwakeningServer\sshKey
  dune@<ip>` command. (1.4.5.0's stock tool now also exposes `shell-vm`/`shell-pod` menu
  entries that wrap this — see §1.4.5.0 implications.)

---

## Suggested order to tackle (tomorrow)

1. **#24** — open the PR (fix already done in fork). Easy win, builds rapport.
2. **#7 + §2.3** — dump-failure surfacing + stray-table hardening (we have first-hand data).
3. **#25 / #22 / theme B** — toggle persistence + state-aware polling/auto-refresh.
4. **#16** — specialization-XP grant (validate mechanism on the testbed; pairs with §3.1/3.2).
5. **#21 / #14 / theme A** — Director/BGD status integration (bigger; do together).
6. **#10** — restart cron (owner already specced it).
7. **#18** — confirm fixed, nudge to close.
8. **#26** — read-only **feasibility check** first: is the Landsraad goal even server-tunable?
   Only commit to building once we know where (if anywhere) the threshold lives. Could seed a
   "server settings editor" if it pans out; otherwise close with an explanation.

---

## Cross-fork survey (2026-06-10)

`adainrivers/dune-dedicated-server-manager` has **7 forks**. Surveyed all active ones for
reachable work we could reuse. **Bottom line: no fork is fixing the open issues — the
contribution lane is clear for us.** Two forks have packaging prior-art worth knowing:

| Fork | Activity | Useful to us? |
|---|---|---|
| **bsmr/**`adainrivers---dune-dedicated-server-manager` | Source of merged upstream **PR #4** — Linux/Fedora **WebKitGTK DMABuf Wayland startup-crash fix** (`WEBKIT_DISABLE_DMABUF_RENDERER=1`, already in upstream). | Reference only; already merged. |
| **Mhynlo/**`dune-dedicated-server-manager` | **PR #2 (closed, unmerged)** — **Linux AppImage build**: Dockerfile (Ubuntu 26.04), `tauri.linux.conf.json`, keyring feature `linux-native-sync-persistent`, docs. Blocked because **the backend is PowerShell-only** ("need to support backend calls that are not based on PowerShell"). | **Yes, as prior art** if we ever pursue Linux/cross-platform — both the starting point *and* the known blocker (abstract the PS backend). |
| **drkshrk/**`dune-dedicated-server-manager` | Filed #21/#22/#25 but the fork's **4 open PRs are all Dependabot bumps** (vite, openssl, esbuild, tar) — **no feature work**. | No. |
| **thebadwolf79/**… | Ours. | — |

**Upstream PR history is thin:** only **#4 merged** (bsmr Wayland fix); **#2 closed unmerged**
(Mhynlo Linux AppImage). So our planned **#24 PR** (battlegroup-update exit-code) would be only
the 3rd external PR and the first bug-fix contribution — good rapport opportunity, low collision risk.

---

## 1.4.5.0 self-hosted-update implications {#1450-self-hosted-update-implications}

The game's **1.4.5.0** self-hosted update (2026-06-10) shipped native features that overlap
several issues here. Detail + the Steam-tooling file analysis is in
`dune-awakening-server/PATCH-1.4.x-SELF-HOSTED.md`; manager-relevant points:

- **#23 (OOM):** 1.4.5.0 adds **`enable-experimental-swap`** and persistent per-map
  **"Minimum Servers"** — the manager could expose both as the in-UI OOM mitigation (see #23).
- **New `change-battlegroup-ip` / `change-vm-ip` commands** now manage the broadcast/VM IP
  natively (logic moved inside the VM). If the manager scripts any IP reconciliation, re-check
  it against these so we don't fight the VM-side auto-refresh.
- **"Proper polling for Start/Stop"** (patch note) lives in the **VM-side `battlegroup` binary**,
  not the host launcher. Worth **re-validating our Stop/Start/Restart wrapper-status fix
  (`663ea27`) and the #24 exit-code handling against 1.4.5.0 output** — the status text/exit
  semantics may have changed, which could either simplify our handling or break the status regex.
- **`shell-vm` / `shell-pod`** menu entries in the stock tool now wrap SSH-into-VM/pod — relevant
  to the #7 onboarding-friction note (users no longer need to hand-craft the `ssh -i` command).
