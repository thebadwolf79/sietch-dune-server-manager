export type LogLevel = "debug" | "info" | "warn" | "error";
export type LogLevelFilter = LogLevel;

export type LogRow = {
  id: number;
  timestamp: string;
  level: LogLevel;
  scope: string;
  message: string;
};

export type OperationLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
};
