import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { RemoteServerRecord } from "../../types/server";
import type { ServerTunnelStatus } from "../../types/tunnel";
import { serverTunnelKey } from "../../utils/remote-server";

export type ManagementTunnelState =
  | { kind: "idle" }
  | { kind: "connecting"; tunnelId: string }
  | { kind: "ready"; tunnelId: string; status: ServerTunnelStatus }
  | { kind: "error"; tunnelId: string; message: string };

export function useManagementTunnel(
  server: RemoteServerRecord,
  enabled: boolean,
): ManagementTunnelState {
  const tunnelId = serverTunnelKey(server.id, "managementApi");
  const [state, setState] = useState<ManagementTunnelState>({ kind: "idle" });

  useEffect(() => {
    if (!enabled) {
      setState({ kind: "idle" });
      return;
    }

    let cancelled = false;
    setState({ kind: "connecting", tunnelId });
    invoke<ServerTunnelStatus>("start_server_tunnel", {
      request: {
        tunnelId,
        serverKind: server.type,
        service: "managementApi",
        host: server.host,
        user: server.user,
        keyPath: server.keyPath,
        port: server.port,
        namespace: server.namespace || "",
      },
    })
      .then((status) => {
        if (!cancelled) setState({ kind: "ready", tunnelId, status });
      })
      .catch((err) => {
        if (!cancelled) setState({ kind: "error", tunnelId, message: String(err) });
      });

    return () => {
      cancelled = true;
      invoke("stop_server_tunnel", { request: { tunnelId } }).catch(() => {});
    };
  }, [enabled, tunnelId, server.host, server.keyPath, server.namespace, server.port, server.type, server.user]);

  return state;
}
