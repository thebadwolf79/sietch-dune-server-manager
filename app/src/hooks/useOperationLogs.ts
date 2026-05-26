import { useCallback, useEffect, useMemo, useState } from "react";

import { listenToEvent, recordOperationLog } from "../services/tauri";
import { readLogSidebar, writeLogSidebar } from "../services/storage";
import type { LogLevelFilter, LogRow, OperationLogPayload } from "../types/log";
import {
  filterLogRows,
  limitLogRows,
  logEntry,
  maxRenderedLogRows,
} from "../utils/logging";

export function useOperationLogs() {
  const persisted = useMemo(readLogSidebar, []);
  const [logRows, setLogRows] = useState<LogRow[]>([]);
  const [logLevelFilter, setLogLevelFilter] = useState<LogLevelFilter>("info");
  const [logPanelCollapsed, setLogPanelCollapsedState] = useState<boolean>(persisted.collapsed ?? false);
  const [scopeToActiveServer, setScopeToActiveServerState] = useState<boolean>(
    persisted.scopeToActiveServer ?? true,
  );

  const setLogPanelCollapsed = (next: boolean | ((current: boolean) => boolean)) => {
    setLogPanelCollapsedState((current) => {
      const resolved = typeof next === "function" ? next(current) : next;
      writeLogSidebar({ collapsed: resolved, scopeToActiveServer });
      return resolved;
    });
  };

  const setScopeToActiveServer = (next: boolean) => {
    setScopeToActiveServerState(next);
    writeLogSidebar({ collapsed: logPanelCollapsed, scopeToActiveServer: next });
  };

  // Memoized so downstream hooks that take appendLogRow as a dep (e.g.
  // useManagementStatus) don't see a new identity on every render — that was
  // causing a refresh-log-rerender feedback loop that spammed the pane.
  const appendLogRow = useCallback((row: LogRow) => {
    setLogRows((rows) => limitLogRows([...rows, row]));
    void recordOperationLog(row.level, row.scope, row.message).catch(() => undefined);
  }, []);

  const clearLogRows = useCallback(() => {
    setLogRows([]);
  }, []);

  useEffect(() => {
    const unlisten = listenToEvent<OperationLogPayload>("operation-log", (payload) => {
      setLogRows((rows) =>
        limitLogRows([
          ...rows,
          logEntry(payload.level, payload.scope, payload.message, payload.serverId),
        ]),
      );
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const renderedLogRows = filterLogRows(logRows, logLevelFilter).slice(-maxRenderedLogRows);

  return {
    logRows,
    logLevelFilter,
    setLogLevelFilter,
    logPanelCollapsed,
    setLogPanelCollapsed,
    scopeToActiveServer,
    setScopeToActiveServer,
    appendLogRow,
    clearLogRows,
    renderedLogRows,
  };
}
