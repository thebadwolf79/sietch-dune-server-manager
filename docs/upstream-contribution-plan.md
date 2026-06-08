# Upstream Contribution Plan

> How to get the ideas in [`improvement-ideas.md`](./improvement-ideas.md) **into
> the original project** (`adainrivers/dune-dedicated-server-manager`) rather than
> maintaining a divergent fork. This fork's real job is a **testbed + demo**: prove
> how each function behaves in-game (especially the online/offline gating) and give
> the maintainer something concrete to look at and merge.

---

## 0. Goal & posture

- **Contribute, don't compete.** Offer features + code to the upstream author,
  shaped so they're easy to adopt into *their* active development. Respect their
  roadmap and vision; propose, don't dictate.
- **Fork = staging ground.** Use it to (a) build and demo features, (b) validate them
  against a live server, and (c) work out the UX rules — then upstream the polished,
  evidence-backed result.
- License is **MIT** (already confirmed), so contribution mechanics are simple. Use
  clear commit attribution and follow whatever `CONTRIBUTING`/style the repo defines.

---

## 1. Engage the maintainer first (before big PRs)

- **Warm intro already exists:** issues **#23** (backup OOMKill) and **#24** (update
  exit-code) are open contributions. Keep being a good citizen there.
- **Open a Discussion or umbrella issue** outlining the proposed feature set and the
  *motivation* (the real-world character-corruption incident — a compelling "this is
  why server admins need safe grant/recovery tooling" story). Link
  `improvement-ideas.md`. Ask: is this welcome? any design preferences? anything
  already in progress so we don't collide?
- Let the maintainer steer scope **before** investing in large PRs. This is the single
  biggest predictor of a PR getting merged.

---

## 2. "See what it can look like" — preview options for the maintainer

Give the author low-effort ways to evaluate without committing:

1. **Draft PRs, one per feature**, each with **screenshots / short screen recordings**
   of the new UI (currency grant dialog, filterable item picker, character panel).
   A picture of the dialog does 10× the convincing of a prose description.
2. **A runnable demo branch** in this fork they can try:
   ```
   git remote add thebadwolf <fork-url>
   git fetch thebadwolf
   git checkout thebadwolf/feature/<x>
   # build per docs/building-from-source.md
   ```
3. **Feature-flag** new panels (config/env toggle) so they can merge-and-hide, or try
   it behind a flag, without it being "live" for all users immediately.
4. **A short demo video** of the end-to-end flow on a real server (grant → in-game
   result), which also doubles as the in-game validation evidence.

---

## 3. Make it trivial to adopt into their active dev

- **Track upstream and stay rebased:**
  ```
  git remote add upstream https://github.com/adainrivers/dune-dedicated-server-manager
  git fetch upstream && git rebase upstream/main   # keep feature branches current
  ```
- **One feature per branch / PR**, small and independent, rebased on `upstream/main`,
  clean commit history — so they can review, merge, or cherry-pick incrementally.
- **Additive & backward-compatible only.** New admin commands alongside existing ones;
  new `items.json` fields (`tier`, `unique`) that old readers ignore; no behavior
  changes to current flows.
- **Split data from code.** The `items.json` tier/unique metadata and the
  `currency_id`→name mapping make great tiny, low-risk, easy-to-merge PRs that unblock
  the bigger feature PRs.
- **PR description template:** problem → approach → how to test (exact steps on a live
  server) → screenshots → compatibility/migration notes → linked issue.
- **Match conventions:** reuse the existing admin-command pattern
  (`admin/commands/specs.rs` + `/api/admin/publish`), the Tauri command style, and add
  tests where the repo has them.
- **Be responsive in review** and willing to adjust to the maintainer's preferences —
  the aim is *their* codebase absorbing it cleanly.

---

## 4. The fork's core job: validate behavior + define the gating rules

The most valuable thing the fork produces (beyond code) is **certainty about when an
action is safe and what server/player state it requires** — i.e. exactly when the UI
should block and tell the user "log out first" (or "log in first"). Build each feature
here, exercise it on the live server, and record the findings as the spec the upstream
guards implement.

### Online/offline requirement matrix (living — validated empirically so far)

| Operation | Required state | Why / evidence | UI guard |
|---|---|---|---|
| Grant item/schematic (`AddItemToInventory` via MQ) | **Player ONLINE** | Probe while offline returned `ok:true` but **never hit the DB** — only applies to a live character | If offline: block, "log in and load your character first" |
| Set currency (`player_virtual_currency_balances` write) | **Player OFFLINE** | Server overwrites the row from in-memory state on logout if online | If online: block, "log out to main menu first" |
| Edit faction / reputation (DB) | **Player OFFLINE** | Same overwrite-on-logout risk | If online: block, "log out first" |
| Relocate / edit character (`actors` row) | **Player OFFLINE** | Overwrite risk + load-time consistency | If online: block, "log out first" |
| Full DB `import` | **Battlegroup STOPPED** | Destructive; not runtime-safe; needs `-sg-` pods drained | Block unless stopped; offer to stop/drain/start |
| Reads / status / inspection | Any state | Read-only | none |

> **Key UX insight:** gating is **bidirectional** — some actions require the player
> *offline*, others require *online*. The panel should know the live online status and
> enforce the right direction per-action, with a clear, specific message.

### What still needs validating on the testbed
- Is there an **engine-side admin command for currency** (preferred over a raw DB
  write)? If yes, currency-grant can use the same safe MQ path as items and may not
  need the offline guard.
- Does `AddItemToInventory` respect inventory capacity (overflow → ground drop)?
- Exact `properties` size threshold where **map travel** (S2S replication) starts to
  fail — to set the "blob too large" warning.
- Behavior of grants during the brief map-load window after login.

---

## 5. Suggested contribution sequence

1. **Bug fixes first** — land #23 (backup OOM) and confirm #24; easy merges that build
   trust and restore the safety net.
2. **Data PR** — `items.json` `tier` + `unique` metadata (+ `currency_id` map). Tiny,
   low-risk, unblocks filtering.
3. **Filterable item-grant picker** — draft PR with screenshots; uses the data PR.
4. **Currency granting** — after validating the engine path + gating rules above.
5. **Recovery tooling** (character panel, targeted restore, guardrails) — bigger;
   propose via discussion first, then incremental PRs.

---

## 6. Etiquette checklist

- [ ] Discussed/agreed scope with maintainer before large PRs
- [ ] Branch rebased on `upstream/main`, one concern per PR
- [ ] Additive, backward-compatible, feature-flagged where sensible
- [ ] Screenshots/recording included; tested-on-live steps documented
- [ ] Conventions + tests matched; CI green
- [ ] Credit/sign-off clean; responsive to review feedback
