# Dune Dedicated Server Manager

A Windows-first manager for the Dune Awakening Playtest dedicated server.

![Dune Dedicated Server Manager](images/screenshot.png)

The app manages existing Dune dedicated servers over SSH and Kubernetes control commands.

## Features

- Remote server profile management over SSH
- BattleGroup status, start, stop, and update controls
- Component diagnostics, log viewing, and safe restart actions
- Secure Director, File Browser, PostgreSQL, and PgHero access through local SSH tunnels

More management features coming soon.

## Installation Guide

1. Download the latest Windows installer from GitHub Releases.
2. Run the installer.
3. Start Dune Dedicated Server Manager from the Start menu or installed shortcut.
4. Add an existing remote server profile with its host and SSH private key path.
5. Refresh the server to detect BattleGroups and management endpoints.

If you found a bug or are having other issues, please create an issue here:
https://github.com/adainrivers/dune-dedicated-server-manager/issues

## Server Setup References

- [Manual Ubuntu setup guide](docs/ubuntu-manual-setup-guide.md)

Required game ports:

- UDP 7777-7810 for game servers
- TCP 31982 for RMQ

## Building From Source

See [Building From Source](docs/building-from-source.md).

## License

MIT License. See [LICENSE](LICENSE).
