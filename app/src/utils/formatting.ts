import type { Update } from "../services/updater";
import type { RemoteServerRecord, RemoteServerStatus } from "../types/server";
import type { TunnelService } from "../types/tunnel";
import type { UpdateStatus } from "../types/update";

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${Math.round(bytes / 1024 / 1024)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export function updateLabel(status: UpdateStatus, availableUpdate: Update | null, progress: string | null): string {
  if (status === "checking") return "Checking";
  if (status === "installing") return progress ?? "Installing";
  if (status === "relaunching") return progress ?? "Relaunching";
  if (status === "failed") return "Check failed";
  if (availableUpdate) return `${availableUpdate.version} available`;
  if (status === "current") return "Up to date";
  return "Not checked";
}

export function updateTone(status: UpdateStatus): "green" | "amber" | "red" {
  if (status === "failed") return "red";
  if (status === "current") return "green";
  return "amber";
}

export function remoteStatusTone(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  battlegroupStarted: boolean,
  battlegroupStartRequested: boolean,
  battlegroupStopped: boolean,
  server: RemoteServerRecord,
): "green" | "amber" | "red" | "gray" {
  if (statusError) return "red";
  if (battlegroupStarted) return "green";
  if (battlegroupStartRequested) return "amber";
  if (battlegroupStopped) return "gray";
  if (server.phase === "Setup running") return "amber";
  return liveStatus ? "green" : "gray";
}

export function remoteStatusLabel(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  busyLabel: string | undefined,
  battlegroupStarted: boolean,
  battlegroupStartRequested: boolean,
  server: RemoteServerRecord,
): string {
  if (statusError) return "Check failed";
  if (busyLabel) return "Retrieving";
  if (!liveStatus) return server.phase || "Unknown";
  if (battlegroupStarted) return "Started";
  return battlegroupStartRequested ? "Starting" : "Stopped";
}

export function tunnelServiceLabel(service: TunnelService): string {
  if (service === "fileBrowser") return "File Browser";
  if (service === "database") return "Postgres";
  if (service === "pgHero") return "PgHero";
  return "Director";
}
