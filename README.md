# Dune Dedicated Server Manager

A desktop manager for existing Dune Awakening dedicated servers.

![Dune Dedicated Server Manager](images/screenshot.png)

The app manages already-provisioned Dune dedicated servers over SSH and
Kubernetes control commands. It does not install the game server, create VMs,
configure Hyper-V, provision Ubuntu, or manage external tools such as SteamCMD.

## Features

- Remote server profile management with SSH private-key authentication
- BattleGroup status, start, stop, and update controls
- Component diagnostics, log viewing, and safe restart actions
- Secure Director, File Browser, PostgreSQL, and PgHero access through local SSH tunnels
- Compact desktop UI for day-to-day operations against an existing server

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
