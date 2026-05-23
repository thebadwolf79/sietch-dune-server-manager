import { useEffect, useRef, useState } from "react";

import { detectRemoteUbuntuServers } from "../services/tauri";
import {
  mergeRemoteServers,
  persistRemoteServers,
  readRemoteServers,
} from "../services/storage";
import type { LogRow } from "../types/log";
import type { RemoteServerRecord } from "../types/server";
import type { RemoteAttachForm } from "../types/ui";
import { errorMessage } from "../utils/errors";
import { log } from "../utils/logging";

type UseRemoteServersArgs = {
  appendLogRow: (row: LogRow) => void;
};

export function useRemoteServers({ appendLogRow }: UseRemoteServersArgs) {
  const [remoteServers, setRemoteServers] = useState<RemoteServerRecord[]>([]);
  const [remoteAttachOpen, setRemoteAttachOpen] = useState(false);
  const [remoteAttachRunning, setRemoteAttachRunning] = useState(false);
  const [remoteAttachForm, setRemoteAttachForm] = useState<RemoteAttachForm>({ host: "", keyPath: "" });
  const [remoteServerToRemove, setRemoteServerToRemove] = useState<RemoteServerRecord | null>(null);

  const refreshRef = useRef<(server: RemoteServerRecord) => Promise<void> | void>(() => undefined);
  const remoteServerBusyRef = useRef<Record<string, string>>({});
  const clearStatusRef = useRef<(serverId: string) => void>(() => undefined);
  const stopTunnelsRef = useRef<(serverId: string) => void>(() => undefined);

  const bindRefreshRemoteServerStatus = (fn: (server: RemoteServerRecord) => Promise<void> | void) => {
    refreshRef.current = fn;
  };
  const bindRemoteServerBusy = (busy: Record<string, string>) => {
    remoteServerBusyRef.current = busy;
  };
  const bindClearStatusForServer = (fn: (serverId: string) => void) => {
    clearStatusRef.current = fn;
  };
  const bindStopTunnelsForServer = (fn: (serverId: string) => void) => {
    stopTunnelsRef.current = fn;
  };

  const addRemoteServer = async () => {
    const host = remoteAttachForm.host.trim();
    const keyPath = remoteAttachForm.keyPath.trim();
    if (!host || !keyPath) return;
    setRemoteAttachRunning(true);
    appendLogRow(log.info("remote.attach", "Detecting remote Dune battlegroups."));
    try {
      const detected = await detectRemoteUbuntuServers({
        host,
        keyPath,
        serverType: "ubuntu",
        user: "root",
      });
      if (detected.length === 0) {
        throw new Error("No Dune battlegroups were detected on the remote server.");
      }
      const nextServers = mergeRemoteServers(remoteServers, detected);
      setRemoteServers(persistRemoteServers(nextServers));
      setRemoteAttachOpen(false);
      setRemoteAttachForm({ host: "", keyPath: "" });
      appendLogRow(log.info("remote.attach", `Added ${detected.length} remote battlegroup profile(s).`));
      for (const server of detected) {
        void refreshRef.current(server);
      }
    } catch (err) {
      appendLogRow(log.error("remote.attach", errorMessage(err)));
    } finally {
      setRemoteAttachRunning(false);
    }
  };

  const removeRemoteServer = (server: RemoteServerRecord) => {
    stopTunnelsRef.current(server.id);
    setRemoteServers((servers) =>
      persistRemoteServers(servers.filter((candidate) => candidate.id !== server.id)),
    );
    clearStatusRef.current(server.id);
    appendLogRow(log.info("remote.attach", `Forgot remote server ${server.name}.`));
  };

  useEffect(() => {
    setRemoteServers(readRemoteServers());
  }, []);

  useEffect(() => {
    for (const server of remoteServers) {
      if (!server.host || !server.keyPath || remoteServerBusyRef.current[server.id]) continue;
      void refreshRef.current(server);
    }
  }, [remoteServers.map((server) => server.id).join("|")]);

  return {
    remoteServers,
    setRemoteServers,
    remoteAttachOpen,
    setRemoteAttachOpen,
    remoteAttachRunning,
    remoteAttachForm,
    setRemoteAttachForm,
    remoteServerToRemove,
    setRemoteServerToRemove,
    addRemoteServer,
    removeRemoteServer,
    bindRefreshRemoteServerStatus,
    bindRemoteServerBusy,
    bindClearStatusForServer,
    bindStopTunnelsForServer,
  };
}
