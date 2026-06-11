import type { Update } from "../services/updater";
import type { RemoteServerRecord, RemoteServerStatus } from "../types/server";
import type { TunnelService } from "../types/tunnel";
import type { UpdateStatus } from "../types/update";

// Backend timestamps are RFC3339 with a UTC offset (chrono `Utc::now().to_rfc3339()`),
// so `new Date(iso)` parses them as UTC. `toLocale*` then renders in the operator's
// local timezone. Never slice `.toISOString()` for display — that leaks raw UTC.
export function formatTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.toLocaleDateString([], {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  })} ${formatTime(iso)}`;
}

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
  if (status === "failed") return "Update check unavailable";
  if (availableUpdate) return `${availableUpdate.version} available`;
  if (status === "current") return "Up to date";
  return "Not checked";
}

export function updateTone(status: UpdateStatus): "green" | "amber" | "red" {
  // A failed/unreachable check (e.g. no published release yet) is not an error
  // worth alarming about — keep it neutral rather than red.
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
