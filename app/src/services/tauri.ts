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
