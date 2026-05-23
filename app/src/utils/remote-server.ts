import type {
  RemoteBattlegroupStatus,
  RemoteServerComponent,
  RemoteServerKind,
  RemoteServerPackageStatus,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../types/server";
import type { TunnelService } from "../types/tunnel";
import type { StatusTone } from "../components/ui/StatusPill";

export function remoteServerDefaultUser(_kind: RemoteServerKind): string {
  return "dune";
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

export type ResolvedServerStatus = {
  tone: StatusTone;
  label: string;
  pulse: boolean;
};

/**
 * Reduces an attached server's various status signals (live status, error,
 * busy label, persisted record phase) into a single tone + label + pulse
 * triple. Shared by RemoteServer detail pages, the top tab strip, and the
 * compact list view.
 */
export function resolveServerStatus(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  busy: boolean,
  server: RemoteServerRecord,
): ResolvedServerStatus {
  if (statusError) return { tone: "err", label: "Check failed", pulse: false };
  if (!liveStatus) return { tone: "gray", label: busy ? "Checking" : "Unknown", pulse: busy };
  const battlegroup = liveStatus.battlegroup;
  if (isBattlegroupStarted(battlegroup)) return { tone: "ok", label: "Started", pulse: false };
  if (!battlegroup.stop) return { tone: "warn", label: battlegroup.phase || "Starting", pulse: true };
  if (battlegroup.stop) return { tone: "gray", label: "Stopped", pulse: false };
  if (server.phase === "Setup running") return { tone: "warn", label: "Setup running", pulse: true };
  return { tone: "gray", label: server.phase || "Unknown", pulse: false };
}

/**
 * Maps a Kubernetes/operator phase string onto the shared status tone
 * vocabulary used by metric tiles and per-map server-stats rows.
 */
export function phaseTone(phase: string): StatusTone {
  const v = phase.trim().toLowerCase();
  if (["running", "ready", "healthy", "available", "reconciling"].includes(v)) return "ok";
  if (["pending", "starting", "deploying", "scheduling", "creating"].includes(v)) return "warn";
  if (["failed", "error", "crashloop", "crashloopbackoff", "unhealthy"].includes(v)) return "err";
  return "gray";
}
