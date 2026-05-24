import type { RemoteServerKind } from "./server";

export type TunnelService = "director" | "fileBrowser" | "database" | "pgHero";

export type ServerTunnelStatus = {
  tunnelId: string;
  service: TunnelService;
  localPort: number;
  remotePort: number;
  url: string;
};

export type ServerTunnelStartRequest = {
  tunnelId: string;
  serverKind: RemoteServerKind;
  service: TunnelService;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  namespace: string;
};
