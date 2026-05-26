# Bundled service binaries

This directory holds the Linux `dune-server-service` binary (musl-static), its
systemd unit, and its OpenRC init script. They are populated by the
`linux-service-binary` job in `.github/workflows/release.yml` and bundled into
the desktop installer as Tauri resources.

For local debug builds the directory can be empty — the `install_management_service`
Tauri command surfaces a friendly error when the resource is missing.

For a local end-to-end test, build the service yourself:

```powershell
rustup target add x86_64-unknown-linux-musl
cargo install --locked cargo-zigbuild
cargo zigbuild -p dune-server-service --release --target x86_64-unknown-linux-musl
Copy-Item target\x86_64-unknown-linux-musl\release\dune-server-service `
  app\src-tauri\binaries\dune-server-service
Copy-Item crates\dune-server-service\systemd\dune-server-service.service `
  app\src-tauri\binaries\dune-server-service.service
Copy-Item crates\dune-server-service\openrc\dune-server-service `
  app\src-tauri\binaries\dune-server-service.openrc
```
