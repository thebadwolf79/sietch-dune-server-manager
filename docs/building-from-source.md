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
