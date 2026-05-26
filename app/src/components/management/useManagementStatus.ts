import { useCallback, useEffect, useState } from "react";

import { managementService } from "../../services/management";
import type { RemoteServerRecord } from "../../types/server";
import type { ManagementServiceStatus } from "../../types/management";
import type { LogRow } from "../../types/log";
import { log } from "../../utils/logging";

export type ManagementStatusState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ok"; value: ManagementServiceStatus }
  | { kind: "error"; message: string };

export type UseManagementStatus = {
  state: ManagementStatusState;
  refresh: () => Promise<void>;
};

export function useManagementStatus(
  server: RemoteServerRecord,
  appendLogRow?: (row: LogRow) => void,
): UseManagementStatus {
  const [state, setState] = useState<ManagementStatusState>({ kind: "idle" });

  const refresh = useCallback(async () => {
    setState({ kind: "loading" });
    appendLogRow?.(
      log.info("mgmt.status", `Checking management service on ${server.host}…`, server.id),
    );
    try {
      const result = await managementService.status({
        host: server.host,
        user: server.user,
        keyPath: server.keyPath,
        port: server.port,
      });
      setState({ kind: "ok", value: result });
      const summary = !result.installed
        ? "not installed"
        : result.active
          ? `active${result.installedVersion ? ` v${result.installedVersion}` : ""} (${result.initSystem})`
          : `installed but not running${result.installedVersion ? ` v${result.installedVersion}` : ""}`;
      appendLogRow?.(
        log.info("mgmt.status", `Management service on ${server.host}: ${summary}.`, server.id),
      );
    } catch (err) {
      const message = String(err);
      setState({ kind: "error", message });
      appendLogRow?.(
        log.error(
          "mgmt.status",
          `Failed to read management status on ${server.host}: ${message}`,
          server.id,
        ),
      );
    }
  }, [server.host, server.id, server.keyPath, server.port, server.user, appendLogRow]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { state, refresh };
}

export function isManagementReady(state: ManagementStatusState): boolean {
  return state.kind === "ok" && state.value.installed && state.value.active;
}
