import type {
  RemoteBattlegroupStatus,
  RemoteServerComponent,
  RemoteServerKind,
  RemoteServerPackageStatus,
  RemoteServerRecord,
} from "../types/server";
import type { TunnelService } from "../types/tunnel";

export function remoteServerDefaultUser(kind: RemoteServerKind): string {
  return kind === "ubuntu" ? "root" : "root";
}

export function remoteServerActionRequest(server: RemoteServerRecord) {
  return {
    serverType: server.type,
    host: server.host,
    user: server.user || remoteServerDefaultUser(server.type),
    keyPath: server.keyPath || undefined,
    namespace: server.namespace,
    battlegroupName: server.battlegroupName,
  };
}

export function isCriticalRestartComponent(component: RemoteServerComponent): boolean {
  const key = component.logKey.toLowerCase();
  const name = component.name.toLowerCase();
  return (
    key.includes("database") ||
    key.includes("messagequeue") ||
    name.includes("database") ||
    name.includes("message queue")
  );
}

const STARTED_PHASES = new Set([
  "running",
  "ready",
  "healthy",
  "available",
  "reconciling",
]);

function isStartedPhase(phase: string): boolean {
  return STARTED_PHASES.has(phase.trim().toLowerCase());
}

export function isBattlegroupStarted(status: RemoteBattlegroupStatus): boolean {
  if (status.stop) return false;
  if (!isStartedPhase(status.phase)) return false;
  if (status.serverGroupPhase && !isStartedPhase(status.serverGroupPhase)) return false;
  if (status.directorPhase && !isDirectorReadyPhase(status.directorPhase)) return false;
  return true;
}

/**
 * Returns true when the downloaded battlegroup version differs from the
 * version currently running in Kubernetes. Both versions must be known; if
 * either is missing we treat the state as "no actionable update" so the
 * Update Server button stays hidden.
 */
export function hasBattlegroupUpdateAvailable(
  pkg: RemoteServerPackageStatus | undefined,
): boolean {
  if (!pkg) return false;
  const downloaded = pkg.battlegroupVersion?.trim();
  const live = pkg.liveBattlegroupVersion?.trim();
  if (!downloaded || !live) return false;
  return downloaded !== live;
}

export function isDirectorReadyPhase(phase: string): boolean {
  const normalized = phase.trim().toLowerCase();
  if (normalized === "" || normalized === "true") return true;
  return isStartedPhase(normalized);
}

export function serverTunnelKey(serverKey: string, service: TunnelService): string {
  return `${serverKey}:tunnel:${service}`;
}

export function componentLogStateKey(serverKey: string, component: RemoteServerComponent): string {
  return `${serverKey}:${component.logKey}`;
}

export function omitKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = record;
  return rest;
}

export function omitPrefix<T>(record: Record<string, T>, prefix: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([key]) => !key.startsWith(prefix)));
}
