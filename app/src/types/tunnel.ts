import type { RemoteServerKind } from "./server";

export type TunnelService = "director" | "fileBrowser" | "database" | "pgHero";

export type CustomTunnelProtocol = "http" | "https" | "postgresql";

export type CustomTunnelDef = {
  id: string;
  name: string;
  protocol: CustomTunnelProtocol;
  remotePort: number;
  localPort: number;
};

export type CustomTunnelStartRequest = {
  tunnelId: string;
  serverKind: RemoteServerKind;
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  protocol: CustomTunnelProtocol;
  remotePort: number;
  localPort: number;
};

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
