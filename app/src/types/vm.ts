// Mirrors the Rust `SystemState` (app/src-tauri/src/dto.rs), serialized as
// `{ state, data? }` (serde tag="state", content="data", camelCase variants).
// Authority for the value lives in Rust; the UI renders it and gates actions on it.
export type SystemState =
  | { state: "unknown" }
  | { state: "hostPermissionUnavailable"; data: { reason: string } }
  | { state: "error"; data: { message: string } }
  | { state: "vmOff" }
  | { state: "vmSaved" }
  | { state: "vmPaused" }
  | { state: "vmStarting"; data: { step: string } }
  | { state: "vmRunning" }
  | { state: "battlegroupStopped" }
  | { state: "battlegroupStarting"; data: { step: string } }
  | { state: "battlegroupHealthy" }
  | { state: "battlegroupDegraded"; data: { reason: string } }
  | { state: "battlegroupStopping"; data: { step: string } };

// Mirrors the Rust `HostReadiness` (orchestration shared_types).
export type HostReadiness = {
  elevated: boolean;
  hypervAvailable: boolean;
  vmmsRunning: boolean;
  virtualizationFirmwareEnabled: boolean | null;
  totalPhysicalMemoryBytes: number;
  availablePhysicalMemoryBytes: number;
  logicalProcessorCount: number;
};

// Gating helpers mirroring the Rust SystemState methods, so the UI and backend
// agree on the same vocabulary.
export function canStartVm(state: SystemState): boolean {
  return state.state === "vmOff" || state.state === "vmSaved" || state.state === "vmPaused";
}

export function battlegroupActionsEnabled(state: SystemState): boolean {
  switch (state.state) {
    case "vmRunning":
    case "battlegroupStopped":
    case "battlegroupStarting":
    case "battlegroupHealthy":
    case "battlegroupDegraded":
    case "battlegroupStopping":
      return true;
    default:
      return false;
  }
}

// Whether this machine can power the VM (manager is on the Hyper-V host).
export function canManageVm(readiness: HostReadiness): boolean {
  return readiness.hypervAvailable && readiness.vmmsRunning;
}

// A short, human-readable label for the current state (for buttons/badges).
export function systemStateLabel(state: SystemState): string {
  switch (state.state) {
    case "unknown":
      return "Unknown";
    case "hostPermissionUnavailable":
      return "Connect-only (Hyper-V unavailable)";
    case "error":
      return state.data.message;
    case "vmOff":
      return "VM off";
    case "vmSaved":
      return "VM saved";
    case "vmPaused":
      return "VM paused";
    case "vmStarting":
      return state.data.step;
    case "vmRunning":
      return "VM running";
    case "battlegroupStopped":
      return "Battlegroup stopped";
    case "battlegroupStarting":
      return state.data.step;
    case "battlegroupHealthy":
      return "Healthy";
    case "battlegroupDegraded":
      return state.data.reason;
    case "battlegroupStopping":
      return state.data.step;
  }
}
