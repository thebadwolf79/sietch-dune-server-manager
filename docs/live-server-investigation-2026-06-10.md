# Live-server investigation — 2026-06-10

Two reported symptoms (the ~15 failed welcome-package runs, and the #23 "Database
OOMKilled") turned out to be **one incident** plus a standing **resilience gap**.

## Symptom 1 — ~15+ failed welcome-package runs

- All failures are `trigger: scheduled`, clustered in a 45-second window
  **18:33:27–18:34:12 UTC**, identical error:
  `no funcom-seabass-* namespace with a Game RMQ pod found`.
- The **welcome-grants ledger is clean** — the only player (Maren Shai,
  `431C7B16E03F3F97`) is `granted`, attempts 1, `lastError: null`. Nobody is stuck.
- **Cause:** the Game RMQ (RabbitMQ) pod was absent during that window because the
  whole battlegroup was mid-restart (see Symptom 2). The welcome task scans for the
  Game RMQ pod, didn't find it, and retried ~every 2s → a burst of identical failures.
- **Verdict: benign / downstream.** Not a welcome-package bug; a symptom of the
  cluster restart. The only real issue is **log/run noise** (a retry storm of ~20
  identical FAILED rows for one transient).

## Symptom 2 — DB instability (#23)

Pod evidence (at ~23:46 UTC):
- `db-dbdepl-sts-0`: **restartCount 8**, last terminated **18:30:39Z** (exitCode 255,
  reason "Unknown"), had started 17:24:47Z. Its postgres log for that instance ends
  abruptly at a normal 17:29 checkpoint with **no graceful-shutdown line** → the
  process was killed, not asked to stop.
- `db-util-mon` (6), `db-util-pghero` (6), `fb-deploy` (8) all restarted **"5h16m ago"
  (≈18:30)** — i.e. several infra pods died **together** at 18:30.
- All game pods (`mq-game`, `mq-admin`, `bgd`, `tr`, `sg-overmap`, `sg-survival`,
  `sgw`) are **age 5h12m (≈18:34)** → the battlegroup **cascade-restarted ~4 min
  after** the DB/infra went down. That 18:30→18:34 gap is exactly when the welcome
  task failed.
- A scheduled **dump pod was `OOMKilled`** 28h ago (`dump-20260609-190000`), plus
  older `dump`/`import` pods in `Error`.

Memory state:
- VM: **30 GB total, ~13.9 GB available, and SWAP = 0.**
- The DB pod has **no memory requests/limits** set (so its kills aren't per-pod cgroup
  OOMs); the dump pod likewise has none → the dump OOMKill was a **node-level** kill.
- Current `dmesg` shows no OOM lines (ring buffer rotated; the 18:30 k8s events are
  also aged out), so the *exact* 18:30 trigger isn't captured. Exit 255 (not 137)
  means that specific death may have been a probe-timeout/crash rather than a clean OOM.

**Verdict:** a single-node VM running k3s + the full game server + Postgres **with zero
swap** is brittle to memory spikes (backups, world load). The OOMKilled dump pod + the
simultaneous 18:30 multi-pod restart + no swap are the systemic risk behind #23. The DB
recovered cleanly each time (crash-recovery/redo in the logs), so no data loss observed —
but the restarts cascade into full battlegroup bounces and transient task failures.

## Recommendations

### For this (solo) server — quick wins
1. **Add swap** (e.g. a 4–8 GB swapfile). Biggest cheap win: absorbs spikes instead of
   letting the kernel OOM-killer drop Postgres. Zero swap on a 30 GB all-in-one box is
   the clearest gap.
   **✅ DONE 2026-06-10:** 8 GB `/swapfile` created (ext4, mode 600), activated, persisted
   in `/etc/fstab` (`/swapfile none swap sw 0 0`) with the OpenRC `swap` service added to
   the boot runlevel; `vm.swappiness=10` set live + persisted in
   `/etc/sysctl.d/99-dune-swap.conf` (the `sysctl` service already runs at boot). Verified
   via `/proc/swaps` (8 GB, 0 used) and `free` (Swap: 8192). Survives reboot.
2. **Right-size the DB dump** so backups don't balloon: confirm the dump path and, if it
   shells `pg_dump`, cap its working memory / avoid parallelism on the constrained box.
3. **(Optional) memory requests/limits on the DB pod** so scheduling + node pressure are
   predictable. On a single-node solo box, swap matters more than limits.

### For the tool (broad-audience)
1. **Restart-aware welcome scan:** treat "no Game RMQ pod" during a known/likely restart
   window as *transient* — back off (exponential, capped) and/or collapse the retry storm
   into a single suppressed/aggregated run instead of ~20 identical FAILED rows. *(Still TODO.)*
2. **Health surfacing:** flag **swap=0**, **DB restartCount climbing**, and **dump pods
   OOMKilled** as health warnings in the UI — leading indicators of the #23 pattern.
   **✅ DONE:** built the **Host Health & Hardening advisor** (`commands/host_health.rs` +
   `HostHealthPanel`). SSH-probes RAM/swap/swappiness/disk and (when a namespace is given)
   DB restart count + OOMKilled pods, renders severity-ranked findings, and offers
   one-click idempotent fixes (`add_swap`, `set_swappiness`) behind a confirmation. The
   exact thing we did by hand for #23 is now a button any operator can use.
3. **Solo-mode awareness:** on a single-player server (one account, already granted),
   the scheduled welcome scan is near-pointless; the profile could downgrade its
   cadence / severity. *(Still TODO.)*

## Status
Live server is currently **healthy**: all pods Running, 13.9 GB available, DB serving
(`/api/health` ok). No player data affected. No action taken on the live server during
this investigation (read-only).
