import {
  getRemoteServerComponents,
  remoteComponentLogTail,
  restartRemoteComponent as restartRemoteComponentCmd,
} from "../services/tauri";
import type { LogRow } from "../types/log";
import type { RemoteServerComponent, RemoteServerRecord } from "../types/server";
import { errorMessage } from "../utils/errors";
import { log, sanitizeLogMessage } from "../utils/logging";
import {
  componentLogStateKey,
  isCriticalRestartComponent,
  omitKey,
  remoteServerActionRequest,
  remoteServerDefaultUser,
} from "../utils/remote-server";

type UseComponentActionsArgs = {
  appendLogRow: (row: LogRow) => void;
  detectRemoteServerDetails: (server: RemoteServerRecord) => Promise<RemoteServerRecord>;
  setRemoteServerComponents: React.Dispatch<
    React.SetStateAction<Record<string, RemoteServerComponent[]>>
  >;
  setRemoteComponentLogs: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  setRemoteComponentLogBusy: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
  setRemoteComponentRestartBusy: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
};

export function useComponentActions({
  appendLogRow,
  detectRemoteServerDetails,
  setRemoteServerComponents,
  setRemoteComponentLogs,
  setRemoteComponentLogBusy,
  setRemoteComponentRestartBusy,
}: UseComponentActionsArgs) {
  const refreshRemoteComponentLog = async (
    server: RemoteServerRecord,
    component: RemoteServerComponent,
  ) => {
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentLogBusy((busy) => ({ ...busy, [key]: true }));
    appendLogRow(log.info("remote.logs", `Refreshing ${component.name} logs.`));
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      const result = await remoteComponentLogTail({
        serverType: liveServer.type,
        host: liveServer.host,
        user: liveServer.user || remoteServerDefaultUser(liveServer.type),
        keyPath: liveServer.keyPath || undefined,
        namespace: liveServer.namespace,
        component: component.logKey,
        tail: 160,
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || "No log output."),
      }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      appendLogRow(log.warn("remote.logs", message));
    } finally {
      setRemoteComponentLogBusy((busy) => omitKey(busy, key));
    }
  };

  const restartRemoteComponent = async (
    server: RemoteServerRecord,
    component: RemoteServerComponent,
  ) => {
    if (isCriticalRestartComponent(component)) {
      const confirmed = window.confirm(
        `Restart ${component.name}? This can temporarily interrupt persistence, messaging, or active players.`,
      );
      if (!confirmed) return;
    }
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentRestartBusy((busy) => ({ ...busy, [key]: true }));
    appendLogRow(log.warn("remote.restart", `Restarting ${component.name}.`));
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      const result = await restartRemoteComponentCmd({
        serverType: liveServer.type,
        host: liveServer.host,
        user: liveServer.user || remoteServerDefaultUser(liveServer.type),
        keyPath: liveServer.keyPath || undefined,
        namespace: liveServer.namespace,
        component: component.logKey,
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || `${component.name} restart requested.`),
      }));
      const components = await getRemoteServerComponents(remoteServerActionRequest(liveServer));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      appendLogRow(log.error("remote.restart", message));
    } finally {
      setRemoteComponentRestartBusy((busy) => omitKey(busy, key));
    }
  };

  return { refreshRemoteComponentLog, restartRemoteComponent };
}
