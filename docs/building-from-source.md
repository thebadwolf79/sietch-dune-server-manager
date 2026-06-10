# Building From Source

## Prerequisites

- Rust stable
- Node.js 22
- npm
- Git

Platform-specific desktop dependencies:

- Windows: WebView2 runtime
- Linux: WebKitGTK 4.1, AppIndicator, librsvg, patchelf, pkg-config, and OpenSSL development headers
- macOS: Xcode Command Line Tools

On Ubuntu 22.04, install the Linux desktop build dependencies with:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf pkg-config libssl-dev
```

## Install Frontend Dependencies

```bash
cd app
npm ci
```

## Run Checks

```bash
cargo check --workspace
cargo test --workspace
cargo doc -p dune-manager-core --no-deps
cd app
npm run build
```

## Run In Development

```bash
cd app
npm run tauri -- dev
```

On Linux the app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` automatically to avoid
a WebKitGTK 4.1 crash on GNOME Wayland (`Error 71 dispatching to Wayland
display`). Export the variable yourself with a different value to override.

## Build A Local Production App

For a local production executable without updater signing or release bundling:

```bash
cd app
npm run tauri -- build --no-bundle
```

The executable is written under the workspace `target/release` directory.

## Build Installers

Release packaging is normally handled by GitHub Actions when a version tag is
pushed. Local full packaging may require platform-specific signing or installer
setup.

Common local bundle commands:

```bash
cd app
npm run tauri -- build --bundles nsis
npm run tauri -- build --bundles appimage,deb
npm run tauri -- build --bundles dmg
```

The app manages already-provisioned servers only. Building from source does not
add any server setup, provisioning, Hyper-V, or installer workflow to the app.

## Local build note: bundled service binaries must exist

`tauri.conf.json` declares three `bundle.resources` under `app/src-tauri/binaries/`:
`dune-server-service` (Linux musl binary), `dune-server-service.service` (systemd
unit), and `dune-server-service.openrc` (OpenRC init). With this Tauri version,
`tauri-build`'s build script validates those paths on **every** `cargo build`/`cargo
test` of the app crate — so an empty `binaries/` dir fails the build before any app
source compiles (`resource path 'binaries\dune-server-service.service' doesn't exist`).

For a local debug build/test you do **not** need the real musl binary — the three
paths just have to exist. The two text files can be copied from the repo and the
binary stubbed (all three are gitignored, so they stay local):

```powershell
$src = "crates\dune-server-service"; $dst = "app\src-tauri\binaries"
Copy-Item "$src\systemd\dune-server-service.service" "$dst\dune-server-service.service" -Force
Copy-Item "$src\openrc\dune-server-service"          "$dst\dune-server-service.openrc" -Force
Set-Content "$dst\dune-server-service" "local debug placeholder" -NoNewline
```

For a real bundle, build the musl binary per `binaries/README.md` (cargo-zigbuild).
