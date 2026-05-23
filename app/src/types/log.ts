export type LogLevel = "debug" | "info" | "warn" | "error";
export type LogLevelFilter = LogLevel;

export type LogRow = {
  id: number;
  timestamp: string;
  level: LogLevel;
  scope: string;
  message: string;
  /** When set, the row is associated with a specific attached server. */
  serverId?: string;
};

export type OperationLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
  serverId?: string;
};
