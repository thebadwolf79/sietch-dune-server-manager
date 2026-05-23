import type { LogLevel, LogLevelFilter, LogRow } from "../types/log";

export const maxStoredLogRows = 2500;
export const maxRenderedLogRows = 1200;

let nextLogRowId = 1;

export function sanitizeLogMessage(message: string): string {
  return message.replace(
    /\b(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?::\d{1,5})?\b/g,
    "IP address",
  );
}

export function logEntry(
  level: LogLevel,
  scope: string,
  message: string,
  serverId?: string,
): LogRow {
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
    serverId,
  };
}

export function filterLogRows(rows: LogRow[], minimum: LogLevelFilter): LogRow[] {
  const rank: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };
  return rows.filter((row) => rank[row.level] >= rank[minimum]);
}

export function limitLogRows(rows: LogRow[]): LogRow[] {
  if (rows.length <= maxStoredLogRows) return rows;
  return rows.slice(-maxStoredLogRows);
}

export const log = {
  debug: (scope: string, message: string, serverId?: string): LogRow =>
    logEntry("debug", scope, message, serverId),
  info: (scope: string, message: string, serverId?: string): LogRow =>
    logEntry("info", scope, message, serverId),
  warn: (scope: string, message: string, serverId?: string): LogRow =>
    logEntry("warn", scope, message, serverId),
  error: (scope: string, message: string, serverId?: string): LogRow =>
    logEntry("error", scope, message, serverId),
};
