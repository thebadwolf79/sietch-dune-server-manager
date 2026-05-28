# Dune Dedicated Server Manager

A desktop manager for existing Dune Awakening dedicated servers.

![Dashboard — BattleGroup status, lifecycle actions, management service, and tunnel controls](images/ss-1.png)

The app manages already-provisioned Dune dedicated servers over SSH and
Kubernetes control commands. It does not install the game server, create VMs,
configure Hyper-V, provision Ubuntu, or manage external tools such as SteamCMD.

## Features

- Remote server profile management with SSH private-key authentication
- BattleGroup status, start, stop, restart, and update controls
- Component diagnostics, log viewing, and safe restart actions
- Secure Director, File Browser, PostgreSQL, and PgHero access through local SSH tunnels
- Bundled `dune-server-service` daemon for on-host scheduled maintenance (daily restarts with in-game warnings, automated backups, server update check + apply) — installed over SSH straight from the Management card
- Admin console for in-game actions: item grants, vehicle spawns, skill/journey/XP tags, player lookup with live pawn location, and a logged history of every published command
- Automated tasks tab with editable schedule settings (daily restart time, warning lead/frequency, update apply lead, IANA timezone) — saving auto-restarts the service so changes apply immediately
- Welcome Package automation: a per-player onboarding chain (item grants, water refill, welcome whisper) driven by Postgres player detection, tracked in the management service's SQLite ledger, and configurable from the Welcome Package tab with both a visual editor and a raw JSON mode

![Admin tab — granting items to online players with a searchable Funcom item picker](images/ss-2.png)

More management features coming soon.

## Install

Download the latest release for your operating system from GitHub Releases.

- Windows: run the NSIS installer.
- Linux: use the AppImage or Debian package.
- macOS: use the DMG for your Mac architecture.

After launching the app, add an existing server profile with its host, SSH user,
and private key path, then refresh it to detect BattleGroups and management
endpoints.

## Managed Server Assumptions

The target server must already be installed and reachable over SSH. The app
expects the Dune Kubernetes resources and vendor management scripts to exist on
the server before you add it.

Required player-facing/server ports depend on your own server deployment. A
typical dedicated-server deployment uses:

- UDP 7777-7810 for game servers
- TCP 31982 for RMQ

If you found a bug or are having other issues, please create an issue here:
https://github.com/adainrivers/dune-dedicated-server-manager/issues

## Building From Source

See [Building From Source](docs/building-from-source.md).

## License

MIT License. See [LICENSE](LICENSE).
