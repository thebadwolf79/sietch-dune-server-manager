import type {
  RemoteBattlegroupStatus,
  RemoteServerComponent,
  RemoteServerKind,
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

export function isBattlegroupStarted(status: RemoteBattlegroupStatus): boolean {
  return !status.stop && status.phase.toLowerCase() === "running";
}

export function isDirectorReadyPhase(phase: string): boolean {
  const normalized = phase.toLowerCase();
  return normalized.includes("ready") || normalized.includes("running") || normalized === "true";
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
