import { useEffect, useRef, useState } from "react";

import {
  checkRemoteSudo,
  detectLocalVmConnection,
  detectRemoteUbuntuServers,
  type PreflightCheck,
} from "../services/tauri";
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
  const [remoteAttachError, setRemoteAttachError] = useState<string | null>(null);
  const [remoteAttachPreflight, setRemoteAttachPreflight] = useState<PreflightCheck | null>(null);
  const [remoteAttachForm, setRemoteAttachForm] = useState<RemoteAttachForm>({
    host: "",
    user: "dune",
    keyPath: "",
    port: 22,
  });
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
    const user = remoteAttachForm.user.trim() || "dune";
    const port = remoteAttachForm.port > 0 ? remoteAttachForm.port : 22;
    if (!host || !keyPath) return;
    setRemoteAttachRunning(true);
    setRemoteAttachError(null);
    setRemoteAttachPreflight(null);
    appendLogRow(log.info("remote.attach", `Preflight check for ${user}@${host}:${port}.`));
    try {
      const preflight = await checkRemoteSudo({ host, user, keyPath, port });
      setRemoteAttachPreflight(preflight);
      if (!preflight.sshOk) {
        throw new Error("SSH connection or key authentication failed.");
      }
      if (!preflight.sudoToDuneOk) {
        throw new Error(
          `${user} cannot sudo to dune without a password. ` +
            `Run on the host as root: echo \"${user} ALL=(dune) NOPASSWD: ALL\" | sudo tee /etc/sudoers.d/${user}`,
        );
      }
      if (!preflight.duneNopasswdOk) {
        throw new Error(
          "dune needs passwordless sudo. Run on the host as root: " +
            `echo "dune ALL=(ALL) NOPASSWD: ALL" | sudo tee /etc/sudoers.d/dune`,
        );
      }
      appendLogRow(log.info("remote.attach", "Preflight passed. Detecting remote battlegroups."));
      const detected = await detectRemoteUbuntuServers({
        host,
        keyPath,
        serverType: "alpine",
        user,
        port,
      });
      if (detected.length === 0) {
        throw new Error("No Dune battlegroups were detected on the remote server.");
      }
      const nextServers = mergeRemoteServers(remoteServers, detected);
      setRemoteServers(persistRemoteServers(nextServers));
      setRemoteAttachOpen(false);
      setRemoteAttachForm({ host: "", user: "dune", keyPath: "", port: 22 });
      setRemoteAttachPreflight(null);
      appendLogRow(log.info("remote.attach", `Added ${detected.length} remote battlegroup profile(s).`));
      for (const server of detected) {
        void refreshRef.current(server);
      }
    } catch (err) {
      const message = errorMessage(err);
      setRemoteAttachError(message);
      appendLogRow(log.error("remote.attach", message));
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

  // When the Add Remote Server dialog opens, auto-detect the local Funcom VM and
  // pre-fill empty fields (VM IP + SSH key path). Assumes the operator has run
  // Funcom's setup at least once; on a remote/no-Hyper-V host this is a no-op and
  // the user fills the fields manually. Only fills blanks, so re-opening never
  // clobbers an edit in progress.
  useEffect(() => {
    if (!remoteAttachOpen) return;
    let active = true;
    void (async () => {
      try {
        const defaults = await detectLocalVmConnection();
        if (!active) return;
        setRemoteAttachForm((prev) => ({
          host: prev.host || defaults.host || "",
          user: prev.user || defaults.user || "dune",
          keyPath: prev.keyPath || defaults.keyPath || "",
          port: prev.port || defaults.port || 22,
        }));
        if (defaults.found) {
          const where = defaults.host ? ` at ${defaults.host}` : "";
          appendLogRow(
            log.info("remote.attach", `Auto-detected VM ${defaults.vmName ?? ""}${where}.`.trim()),
          );
        } else if (defaults.note) {
          appendLogRow(log.info("remote.attach", defaults.note));
        }
      } catch {
        // best-effort; the user can still enter details manually
      }
    })();
    return () => {
      active = false;
    };
  }, [remoteAttachOpen]);

  return {
    remoteServers,
    setRemoteServers,
    remoteAttachOpen,
    setRemoteAttachOpen,
    remoteAttachRunning,
    remoteAttachForm,
    setRemoteAttachForm,
    remoteAttachError,
    setRemoteAttachError,
    remoteAttachPreflight,
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
