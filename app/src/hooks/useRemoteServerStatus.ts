import { useState } from "react";

import {
  detectRemoteUbuntuServers,
  getRemoteServerComponents,
  getRemoteServerStatus,
  startRemoteBattlegroup,
  stopRemoteBattlegroup,
  updateRemoteBattlegroup,
} from "../services/tauri";
import { persistRemoteServers, upsertRemoteServer } from "../services/storage";
import type { LogRow } from "../types/log";
import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../types/server";
import { errorMessage } from "../utils/errors";
import { log } from "../utils/logging";
import {
  omitKey,
  omitPrefix,
  remoteServerActionRequest,
  remoteServerDefaultUser,
} from "../utils/remote-server";

type UseRemoteServerStatusArgs = {
  appendLogRow: (row: LogRow) => void;
  setRemoteServers: React.Dispatch<React.SetStateAction<RemoteServerRecord[]>>;
};

export function useRemoteServerStatus({ appendLogRow, setRemoteServers }: UseRemoteServerStatusArgs) {
  const [remoteServerStatuses, setRemoteServerStatuses] = useState<Record<string, RemoteServerStatus>>({});
  const [remoteServerComponents, setRemoteServerComponents] = useState<Record<string, RemoteServerComponent[]>>({});
  const [remoteServerStatusErrors, setRemoteServerStatusErrors] = useState<Record<string, string>>({});
  const [remoteServerBusy, setRemoteServerBusy] = useState<Record<string, string>>({});
  const [remoteComponentLogs, setRemoteComponentLogs] = useState<Record<string, string>>({});
  const [remoteComponentLogBusy, setRemoteComponentLogBusy] = useState<Record<string, boolean>>({});
  const [remoteComponentRestartBusy, setRemoteComponentRestartBusy] = useState<Record<string, boolean>>({});

  const detectRemoteServerDetails = async (server: RemoteServerRecord): Promise<RemoteServerRecord> => {
    const detected = await detectRemoteUbuntuServers({
      host: server.host,
      keyPath: server.keyPath,
      serverType: "ubuntu",
      user: server.user || remoteServerDefaultUser(server.type),
    });
    if (detected.length === 0) {
      throw new Error("No Dune battlegroups were detected on the remote server.");
    }
    return detected.find((candidate) => candidate.battlegroupName === server.battlegroupName) ?? detected[0];
  };

  const refreshRemoteServerStatus = async (server: RemoteServerRecord) => {
    if (!server.host || !server.keyPath) return;
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: "Retrieving server information" }));
    setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
    setRemoteServerComponents((components) => omitKey(components, server.id));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    try {
      const liveServer = await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const status = await getRemoteServerStatus(remoteServerActionRequest(liveServer));
      const components = await getRemoteServerComponents(remoteServerActionRequest(liveServer));
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id
              ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase }
              : candidate,
          ),
        ),
      );
      appendLogRow(
        log.info(
          "remote.status",
          `${liveServer.battlegroupName}: ${status.battlegroup.phase || "unknown"}, server group ${
            status.battlegroup.serverGroupPhase || "unknown"
          }, Director ${status.battlegroup.directorPhase || "unknown"}.`,
        ),
      );
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
      setRemoteServerComponents((components) => omitKey(components, server.id));
      setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.warn("remote.status", message));
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const runRemoteBattlegroupAction = async (
    server: RemoteServerRecord,
    action: "start" | "stop" | "update",
  ) => {
    const busyText =
      action === "start" ? "Starting battlegroup" : action === "stop" ? "Stopping battlegroup" : "Updating battlegroup";
    const verb = action === "start" ? "Starting" : action === "stop" ? "Stopping" : "Updating";
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: busyText }));
    appendLogRow(log.info("bg", `${verb} remote battlegroup.`));
    try {
      const liveServer =
        server.namespace && server.battlegroupName ? server : await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const request = remoteServerActionRequest(liveServer);
      const status =
        action === "start"
          ? await startRemoteBattlegroup(request)
          : action === "stop"
            ? await stopRemoteBattlegroup(request)
            : await updateRemoteBattlegroup(request);
      const components = await getRemoteServerComponents(request);
      setRemoteServerStatuses((statuses) => ({ ...statuses, [liveServer.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [liveServer.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, liveServer.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === liveServer.id
              ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase }
              : candidate,
          ),
        ),
      );
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      appendLogRow(log.error("bg", message));
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const clearStatusForServer = (serverId: string) => {
    setRemoteServerStatuses((statuses) => omitKey(statuses, serverId));
    setRemoteServerComponents((components) => omitKey(components, serverId));
    setRemoteServerStatusErrors((errors) => omitKey(errors, serverId));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${serverId}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${serverId}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${serverId}:`));
  };

  return {
    remoteServerStatuses,
    remoteServerComponents,
    setRemoteServerComponents,
    remoteServerStatusErrors,
    remoteServerBusy,
    remoteComponentLogs,
    setRemoteComponentLogs,
    remoteComponentLogBusy,
    setRemoteComponentLogBusy,
    remoteComponentRestartBusy,
    setRemoteComponentRestartBusy,
    detectRemoteServerDetails,
    refreshRemoteServerStatus,
    runRemoteBattlegroupAction,
    clearStatusForServer,
  };
}
