# Design Document: Linux Native Support

This document outlines the architectural changes required to support deploying, managing, and orchestrating the Dune dedicated server on native Linux hosts (Debian, Ubuntu, Alpine) alongside the existing Windows Hyper-V VM path.

---

## 1. Architectural Comparison

Currently, the manager works by creating and managing an Alpine Linux guest VM on a Windows host via Hyper-V. On a native Linux host, virtualization is unnecessary—the server stack can run directly on the host using `k3s` and containerized operators.

| Component | Windows Host Path (Hyper-V) | Linux Native Path (Direct Host) |
|---|---|---|
| **Host OS** | Windows 10/11 Pro (Hyper-V enabled) | Ubuntu 24.04+, Debian, Alpine Linux |
| **Virtualization** | Hyper-V VM (Alpine guest) | None (bare-metal direct host execution) |
| **Orchestrator** | `k3s` running inside the guest VM | `k3s` running directly on host OS |
| **Service Manager** | `OpenRC` (in Alpine guest VM) | `systemd` (Ubuntu/Debian) or `OpenRC` (Alpine host) |
| **Manager Service** | `dune-server-service` (musl in VM guest) | `dune-server-service` (musl or glibc on host) |
| **Host Connectivity** | SSH to `192.168.200.10` from Windows | Local process execution OR SSH to remote Linux host |

---

## 2. Core Crate Refactoring (`crates/dune-manager-core`)

To support native Linux hosts, the orchestration layer must abstract away Hyper-V specific operations.

### A. The `VmProvider` Abstraction
`VmProvider` is currently implemented by `StrictPowerShellHyperV` using PowerShell commands. We need to introduce a `DirectHostProvider` (or `NoVmProvider`) that:
* Bypasses VM creation, network adapters, and virtual switch configuration.
* Implements `start_vm` and `stop_vm` as system-level controls (e.g., systemd service commands to start/stop the `k3s` service).
* Implements `list_vms` to return a virtual "host-local" VM record.

```rust
pub struct DirectHostProvider {
    // Manages direct host orchestration on Linux
}

impl VmProvider for DirectHostProvider {
    fn start_vm(&self, _name: &str) -> CommandResult<()> {
        // systemctl start k3s
        Ok(())
    }
    fn stop_vm(&self, _name: &str, _turn_off: bool) -> CommandResult<()> {
        // systemctl stop k3s
        Ok(())
    }
    // Other methods either map to direct host operations or return no-ops
}
```

### B. Dynamically Gated Service Operations
Operating system scripts differ. The official Hyper-V VM is Alpine and uses `OpenRC` (`rc-service`), whereas native Linux hosts (like Ubuntu or Debian) use `systemd` (`systemctl`). The manager must query host metadata and compile templates accordingly:
* **Systemd Configuration:** Deploy `dune-server-service.service` and configure `/etc/systemd/system/`.
* **OpenRC Configuration:** Deploy `dune-server-service.openrc` and configure `/etc/init.d/`.
* **Sudo Permissions:** Configure `/etc/sudoers.d/dune-server` to allow `dune` user to run `kubectl`, `k3s`, and service control commands without password prompts.

### C. Local vs. Remote Command Execution
Currently, all operations assume we SSH to the Alpine guest. On a native Linux host, if the Tauri application runs on the same machine, SSH can be bypassed in favor of local `std::process::Command` calls (with appropriate privilege elevation where needed). If managing a remote Linux host, we continue using SSH.

---

## 3. Deployment and Installation Changes

### A. Packaging and Build Steps
The Tauri build script (`app/src-tauri/build.rs`) must package:
1. `dune-server-service` binary compiled for the target host environment.
2. Systemd service template file (`dune-server-service.service`).
3. OpenRC service script file (`dune-server-service.openrc`).
4. Automated bootstrap script (`dune-bootstrap-kubernetes.sh`) to import images and initialize Operators.

### B. Setup Script Patching
Funcom's official setup scripts contain hardcoded Alpine/OpenRC references (e.g., `rc-service k3s start`). When executing on systemd-based hosts, the manager must automatically patch these scripts before executing them (as detailed in [Manual Ubuntu Setup Guide](file:///C:/Users/theba/dune-dedicated-server-manager/docs/ubuntu-manual-setup-guide.md#L690-L742)):
* Replace `rc-service k3s start` with `systemctl start k3s`
* Replace `rc-update add k3s` with `systemctl enable k3s`
* Configure cgroup limits under systemd (`/sys/fs/cgroup/system.slice/k3s.service/memory.swap.max`)

---

## 4. Open Design Questions

1. **Host-vs-Remote Model:** Should the Tauri UI default to running local shell commands when running on a Linux host, or should it run an SSH loop back to `localhost` to keep the code path unified? (Unified SSH to `localhost` simplifies state tracking but requires setting up an SSH key on the host).
2. **Alpine KVM Alternative:** Instead of bare-metal k3s, should we offer a Linux KVM path that boots the official Alpine VM image? (This maintains 100% parity with Funcom's VM setup but incurs virtualization overhead and requires nested virtualization support).
