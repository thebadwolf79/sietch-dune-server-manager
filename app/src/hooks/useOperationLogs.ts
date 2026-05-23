import { useEffect, useState } from "react";

import { listenToEvent } from "../services/tauri";
import type { LogLevelFilter, LogRow, OperationLogPayload } from "../types/log";
import {
  filterLogRows,
  limitLogRows,
  logEntry,
  maxRenderedLogRows,
} from "../utils/logging";

export function useOperationLogs() {
  const [logRows, setLogRows] = useState<LogRow[]>([]);
  const [logLevelFilter, setLogLevelFilter] = useState<LogLevelFilter>("info");
  const [logPanelCollapsed, setLogPanelCollapsed] = useState(false);

  const appendLogRow = (row: LogRow) => {
    setLogRows((rows) => limitLogRows([...rows, row]));
  };

  const clearLogRows = () => {
    setLogRows([]);
  };

  useEffect(() => {
    const unlisten = listenToEvent<OperationLogPayload>("operation-log", (payload) => {
      appendLogRow(logEntry(payload.level, payload.scope, payload.message));
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
    appendLogRow,
    clearLogRows,
    renderedLogRows,
  };
}
