# Add Remote Server: auto-detect + Alpine alignment

Branch `feat/vm-power-on`. Makes "Add Remote Server" smart-by-default and corrects the
Ubuntu→Alpine mislabeling, aligned to Funcom's self-hosted requirements.

## Operating assumption (matches Funcom's self-hosted page)
The tool comes into use **after** the operator has satisfied
<https://duneawakening.com/self-hosted-servers/> at least once — i.e. they've run Funcom's
setup, which creates the Hyper-V VM and the SSH key. So we can safely assume those exist at
known locations and auto-fill from them. Verified against Funcom's own tooling
(`…/Dune Awakening Self-Hosted Server/battlegroup-management/vm-utilities.ps1`):
- VM name: **`dune-awakening`**
- SSH user: **`dune`**, port **22**
- SSH key: **`%LOCALAPPDATA%\DuneAwakeningServer\sshKey`**
- Guest OS: **Alpine** (k3s) — *every* Funcom self-hosted VM is Alpine.

## What changed
- **`detect_local_vm_connection` command** (`commands/vm.rs`): host-only, best-effort. Uses
  core's `DuneVmDetector` (Hyper-V fingerprinting) to find the Dune VM + its routable IPv4,
  and resolves the Funcom SSH-key path if present. Returns `VmConnectionDefaults
  { found, host, user, port, keyPath, vmName, serverType: "alpine", confidence, note }`.
  Never errors the dialog — off-host it just returns safe defaults (`dune`/22) for manual entry.
- **Dialog auto-fill** (`useRemoteServers`): when the Add Remote Server dialog opens, it calls
  the command and pre-fills **only empty** fields (host = VM IP, keyPath = Funcom key), so the
  user usually just confirms — but can edit anything, and re-opening never clobbers an edit.
- **Alpine typing/labels:** `RemoteServerKind = "ubuntu" | "alpine"`; detection labels servers
  `alpine` (`discovery.rs`); storage accepts both; empty-state copy updated. The
  management-service install already runtime-detects systemd vs OpenRC, so behavior was always
  Alpine-correct — this fixes the *typing/labeling* to match reality.

## Future feature (out of scope now): Linux self-hosting
Some operators have run the Funcom self-hosted server directly on Linux (KVM), bypassing the
Windows/Hyper-V wrapper (see closed upstream issue #1's generic KVM guide). That's a **future**
addition to the tool. Current focus is aligning as closely as possible with Funcom's official
**Windows + Hyper-V** requirements; the auto-detect above is Hyper-V/host-only by design and
degrades gracefully (manual entry) everywhere else, which already leaves room for a Linux path
later without breaking the Windows assumption.
