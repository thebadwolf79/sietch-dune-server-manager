import { type LogRow, type LogLevel, type LogLevelFilter, type UpdateStatus, type ServerPackageCheckStatus, type ServerPackageStatus, type DetectionState } from "../types";
import { type Update } from "@tauri-apps/plugin-updater";

let nextLogRowId = 1;
export const maxStoredLogRows = 2500;
export const maxRenderedLogRows = 1200;

export const zeroToFour = ["0", "1", "2", "3", "4"];
export const oneToFour = ["1", "2", "3", "4"];
export const zeroToOne = ["0", "1"];

export const playerPortForwards = [
  { ports: "7777-7810", protocol: "UDP", purpose: "Game servers" },
  { ports: "31982", protocol: "TCP", purpose: "RMQ" },
];

export function errorMessage(err: unknown): string {
  if (!err) return "Operation failed.";
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (typeof err === "object" && "message" in err && typeof (err as any).message === "string") {
    return (err as any).message;
  }
  return "Operation failed.";
}

export function sanitizeLogMessage(message: string): string {
  return message.replace(
    /\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?::\d{1,5})?\b/g,
    "IP address",
  );
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${Math.round(bytes / 1024 / 1024)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export function formatGiB(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "unknown";
  return `${Math.round(bytes / 1024 / 1024 / 1024)} GB`;
}

export function formatGiBFloor1(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "unknown";
  const gib = bytes / 1024 / 1024 / 1024;
  return `${(Math.floor(gib * 10) / 10).toFixed(1)} GB`;
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "00:00:00";
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  return [hours, minutes, secs].map((value) => String(value).padStart(2, "0")).join(":");
}

export function parsePositiveInt(value: string): number {
  if (!value) return 0;
  const parsed = parseInt(value, 10);
  return Number.isNaN(parsed) || parsed < 0 ? 0 : parsed;
}

export function logEntry(level: LogLevel, scope: string, message: string): LogRow {
  return {
    id: nextLogRowId++,
    timestamp: new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
    level,
    scope,
    message: sanitizeLogMessage(message),
  };
}

export const log = {
  debug: (scope: string, message: string): LogRow => logEntry("debug", scope, message),
  info: (scope: string, message: string): LogRow => logEntry("info", scope, message),
  warn: (scope: string, message: string): LogRow => logEntry("warn", scope, message),
  error: (scope: string, message: string): LogRow => logEntry("error", scope, message),
};

export function filterLogRows(rows: LogRow[], minimum: LogLevelFilter): LogRow[] {
  const rank: Record<LogLevel, number> = {
    debug: 0,
    info: 1,
    warn: 2,
    error: 3,
  };
  return rows.filter((row) => rank[row.level] >= rank[minimum]);
}

export function limitLogRows(rows: LogRow[]): LogRow[] {
  if (rows.length <= maxStoredLogRows) return rows;
  return rows.slice(-maxStoredLogRows);
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

export function serverPackageLabel(status: ServerPackageCheckStatus, packageStatus: ServerPackageStatus | null): string {
  if (status === "checking") return "Package checking";
  if (status === "updating") return "Package updating";
  if (status === "failed") return "Package check failed";
  if (status === "missing") return "Package missing";
  if (status === "available") {
    return packageStatus?.latestBuildId ? `Server ${packageStatus.latestBuildId} available` : "Server update available";
  }
  if (status === "current") {
    return packageStatus?.installedBuildId ? `Server ${packageStatus.installedBuildId}` : "Server package current";
  }
  return "Server package";
}

export function serverPackageTone(status: ServerPackageCheckStatus): "green" | "amber" | "red" {
  if (status === "failed" || status === "missing") return "red";
  if (status === "current") return "green";
  return "amber";
}

export function networkStatusLabel(status: DetectionState): string {
  if (status === "idle") return "Run local detection";
  if (status === "detecting") return "Detecting adapters...";
  if (status === "failed") return "Detection failed";
  return "Choose adapter";
}

export function zeroTo(max: number): string[] {
  return Array.from({ length: max + 1 }, (_, index) => String(index));
}
