import { useEffect, useState } from "react";

import {
  openExternal,
  serverTunnelStatus,
  startServerTunnel as startServerTunnelCmd,
  stopAllTunnels,
  stopServerTunnel as stopServerTunnelCmd,
} from "../services/tauri";
import type { LogRow } from "../types/log";
import type { ServerTunnelStartRequest, ServerTunnelStatus } from "../types/tunnel";
import { copyTextToClipboard } from "../utils/clipboard";
import { errorMessage } from "../utils/errors";
import { tunnelServiceLabel } from "../utils/formatting";
import { log } from "../utils/logging";
import { omitKey } from "../utils/remote-server";

type UseServerTunnelsArgs = {
  appendLogRow: (row: LogRow) => void;
};

export function useServerTunnels({ appendLogRow }: UseServerTunnelsArgs) {
  const [serverTunnels, setServerTunnels] = useState<Record<string, ServerTunnelStatus>>({});
  const [serverTunnelBusy, setServerTunnelBusy] = useState<Record<string, boolean>>({});

  const startServerTunnel = async (request: ServerTunnelStartRequest) => {
    setServerTunnelBusy((busy) => ({ ...busy, [request.tunnelId]: true }));
    appendLogRow(log.info("tunnel", `Starting ${tunnelServiceLabel(request.service)} tunnel.`));
    try {
      const status = await startServerTunnelCmd(request);
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      appendLogRow(log.info("tunnel", `${tunnelServiceLabel(request.service)} tunnel is ready at ${status.url}`));
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, request.tunnelId));
    }
  };

  const openServerTunnel = async (tunnel: ServerTunnelStatus) => {
    try {
      const status = await serverTunnelStatus(tunnel.tunnelId);
      if (!status) {
        setServerTunnels((tunnels) => omitKey(tunnels, tunnel.tunnelId));
        appendLogRow(log.warn("tunnel", "The SSH tunnel is no longer running."));
        return;
      }
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      if (status.service === "database") {
        await copyTextToClipboard(status.url);
        appendLogRow(log.info("tunnel", `Copied Postgres connection URI ${status.url}`));
        return;
      }
      await openExternal(status.url);
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    }
  };

  const stopServerTunnel = async (tunnelId: string) => {
    setServerTunnelBusy((busy) => ({ ...busy, [tunnelId]: true }));
    try {
      await stopServerTunnelCmd(tunnelId);
      setServerTunnels((tunnels) => omitKey(tunnels, tunnelId));
      appendLogRow(log.info("tunnel", "SSH tunnel stopped."));
    } catch (err) {
      appendLogRow(log.error("tunnel", errorMessage(err)));
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, tunnelId));
    }
  };

  const stopTunnelsForServer = (serverKey: string) => {
    for (const tunnelId of Object.keys(serverTunnels).filter((id) => id.startsWith(`${serverKey}:tunnel:`))) {
      void stopServerTunnel(tunnelId);
    }
  };

  useEffect(() => {
    return () => {
      void stopAllTunnels();
    };
  }, []);

  return {
    serverTunnels,
    serverTunnelBusy,
    startServerTunnel,
    openServerTunnel,
    stopServerTunnel,
    stopTunnelsForServer,
  };
}
