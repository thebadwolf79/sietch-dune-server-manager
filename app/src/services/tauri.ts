import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { open as openShell } from "@tauri-apps/plugin-shell";
import { relaunch as relaunchProcess } from "@tauri-apps/plugin-process";

import type {
  RemoteComponentLogResult,
  RemoteComponentRestartResult,
} from "../types/component";
import type {
  RemoteServerComponent,
  RemoteServerKind,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../types/server";
import type { CustomTunnelStartRequest, ServerTunnelStartRequest, ServerTunnelStatus } from "../types/tunnel";
import type {
  HostApplyFixResult,
  HostHealthReport,
  HostReadiness,
  SystemState,
} from "../types/vm";

type RemoteActionRequest = {
  serverType: RemoteServerKind;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  namespace: string;
  battlegroupName: string;
};

type DetectRemoteServersRequest = {
  host: string;
  keyPath: string;
  serverType: RemoteServerKind;
  user: string;
  port?: number;
};

type RemoteComponentLogRequest = {
  serverType: RemoteServerKind;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  namespace: string;
  component: string;
  tail: number;
};

type RemoteComponentRestartRequest = {
  serverType: RemoteServerKind;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  namespace: string;
  component: string;
};

export async function detectRemoteUbuntuServers(
  request: DetectRemoteServersRequest,
): Promise<RemoteServerRecord[]> {
  return invoke<RemoteServerRecord[]>("detect_remote_ubuntu_servers", { request });
}

// Best-effort connection defaults for the local Funcom VM (host-only). Mirrors the
// Rust VmConnectionDefaults (camelCase). Used to pre-fill the Add Remote Server dialog.
export type VmConnectionDefaults = {
  found: boolean;
  host?: string | null;
  user: string;
  port: number;
  keyPath?: string | null;
  vmName?: string | null;
  serverType: string;
  confidence?: string | null;
  note?: string | null;
};

// Auto-detect the running Funcom VM (IP) + the Funcom SSH key path to pre-fill the
// add-server form. Never throws meaningfully — returns safe defaults when off-host.
export async function detectLocalVmConnection(): Promise<VmConnectionDefaults> {
  return invoke<VmConnectionDefaults>("detect_local_vm_connection");
}

export async function getRemoteServerStatus(request: RemoteActionRequest): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>("remote_server_status", { request });
}

export async function getRemoteServerComponents(
  request: RemoteActionRequest,
): Promise<RemoteServerComponent[]> {
  return invoke<RemoteServerComponent[]>("remote_server_components", { request });
}

export async function startRemoteBattlegroup(request: RemoteActionRequest): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>("start_remote_battlegroup", { request });
}

export async function stopRemoteBattlegroup(request: RemoteActionRequest): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>("stop_remote_battlegroup", { request });
}

export async function updateRemoteBattlegroup(request: RemoteActionRequest): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>("update_remote_battlegroup", { request });
}

export async function restartRemoteBattlegroup(request: RemoteActionRequest): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>("restart_remote_battlegroup", { request });
}

// --- Hyper-V VM power management (issue #28; host-only) ---

/// Reports whether this machine can manage the Hyper-V VM (connect-only vs power-capable).
export async function vmHostReadiness(): Promise<HostReadiness> {
  return invoke<HostReadiness>("vm_host_readiness");
}

export async function vmGetState(vmName: string): Promise<SystemState> {
  return invoke<SystemState>("vm_get_state", { vmName });
}

export async function vmStart(vmName: string): Promise<SystemState> {
  return invoke<SystemState>("vm_start", { vmName });
}

export async function vmStop(vmName: string): Promise<SystemState> {
  return invoke<SystemState>("vm_stop", { vmName });
}

// --- Host Health & Hardening advisor (SSH-based; works on any reachable VM) ---

export type HostHealthCheckRequest = {
  serverType?: string;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  namespace?: string;
};

export async function hostHealthCheck(request: HostHealthCheckRequest): Promise<HostHealthReport> {
  return invoke<HostHealthReport>("host_health_check", { request });
}

export type HostApplyFixRequest = {
  serverType?: string;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  fixId: string;
  param?: number;
};

export async function hostApplyFix(request: HostApplyFixRequest): Promise<HostApplyFixResult> {
  return invoke<HostApplyFixResult>("host_apply_fix", { request });
}

export async function startServerTunnel(request: ServerTunnelStartRequest): Promise<ServerTunnelStatus> {
  return invoke<ServerTunnelStatus>("start_server_tunnel", { request });
}

export async function startCustomTunnel(request: CustomTunnelStartRequest): Promise<ServerTunnelStatus> {
  return invoke<ServerTunnelStatus>("start_custom_tunnel", { request });
}

export async function stopServerTunnel(tunnelId: string): Promise<void> {
  await invoke("stop_server_tunnel", { request: { tunnelId } });
}

export async function serverTunnelStatus(tunnelId: string): Promise<ServerTunnelStatus | null> {
  return invoke<ServerTunnelStatus | null>("server_tunnel_status", { request: { tunnelId } });
}

export async function stopAllTunnels(): Promise<void> {
  await invoke("stop_all_tunnels");
}

export async function remoteComponentLogTail(
  request: RemoteComponentLogRequest,
): Promise<RemoteComponentLogResult> {
  return invoke<RemoteComponentLogResult>("remote_component_log_tail", { request });
}

export async function restartRemoteComponent(
  request: RemoteComponentRestartRequest,
): Promise<RemoteComponentRestartResult> {
  return invoke<RemoteComponentRestartResult>("restart_remote_component", { request });
}

export function listenToEvent<T>(
  channel: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(channel, (event) => handler(event.payload));
}

export async function openFileDialog(title: string): Promise<string | null> {
  const selected = await openDialog({ directory: false, multiple: false, title });
  return typeof selected === "string" ? selected : null;
}

export async function openExternal(url: string): Promise<void> {
  await openShell(url);
}

export async function relaunch(): Promise<void> {
  await relaunchProcess();
}

export type PreflightCheck = {
  sshOk: boolean;
  sudoToDuneOk: boolean;
  duneNopasswdOk: boolean;
  isDuneLogin: boolean;
  rawOutput: string;
};

export async function checkRemoteSudo(request: {
  host: string;
  user: string;
  keyPath: string;
  port?: number;
}): Promise<PreflightCheck> {
  return invoke<PreflightCheck>("check_remote_sudo", { request });
}

export async function recordOperationLog(level: string, scope: string, message: string): Promise<void> {
  await invoke("record_operation_log", { level, scope, message });
}

export async function getLogsFolder(): Promise<string> {
  return invoke<string>("get_logs_folder");
}

export async function openLogsFolder(): Promise<void> {
  const path = await getLogsFolder();
  if (path) await openShell(path);
}
