# Contributing to Sietch

Thanks for your interest! **Sietch** is an unofficial community fork of
[adainrivers/dune-dedicated-server-manager](https://github.com/adainrivers/dune-dedicated-server-manager)
(© gaming.tools, MIT).

## Where to send changes

- **Broadly useful fixes** — bugs or improvements that aren't specific to
  Sietch's redesign or extra features — are best sent **upstream** first, so the
  whole community benefits. We track upstream and will pull them in. (Recent
  examples we contributed back: the `battlegroup update` exit-code fix, the
  Auto-Update/Users-tab fixes, and backup-failure diagnostics.)
- **Sietch-specific** work — the UI redesign, Hyper-V VM power controls, in-game
  grants, the Host Health & Hardening advisor, etc. — belongs here.

If you're unsure, open an issue and we'll point you the right way.

## Building & verifying

See [docs/building-from-source.md](docs/building-from-source.md). In short:
Rust stable + Node 22 + npm. Before opening a PR, these must be clean:

```
cargo test --workspace          # Rust: no warnings, all tests pass
cd app && npm run build         # tsc && vite build, clean
```

## Standards

- **Rust:** no warnings; `cargo test` green. **TS:** `tsc && vite build` clean.
- **Verify state, don't trust vendor exit codes** — confirm the desired result
  rather than relying on a wrapper script's return code.
- **Game-data writes** are guarded: the player must be offline, currency/Intel
  use targeted writes, and the `dune.actors.properties` blob is never
  round-tripped (single-field `jsonb_set` only — an incident lesson).
- Keep commits small and well scoped; note how you verified the change.

## Not affiliated

This is a fan-made server-admin tool, not affiliated with or endorsed by
gaming.tools, Funcom, or Legendary Entertainment.
