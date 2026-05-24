import type { BadgeTone } from "./ui";

export type RemoteServerKind = "ubuntu";

export type RemoteServerPackageStatus = {
  installedBuildId?: string | null;
  battlegroupVersion?: string | null;
  liveBattlegroupVersion?: string | null;
  operatorVersion?: string | null;
};

export type RemoteBattlegroupStatus = {
  stop: boolean;
  phase: string;
  databasePhase?: string;
  /** Gateway phase column from the vendor wrapper. */
  serverGroupPhase: string;
  directorPhase: string;
  uptime?: string;
  serverStats?: RemoteBattlegroupServerStat[];
};

export type RemoteBattlegroupServerStat = {
  map: string;
  phase: string;
  ready: string;
  players: string;
  age: string;
};

export type RemoteServerStatus = {
  battlegroup: RemoteBattlegroupStatus;
  package: RemoteServerPackageStatus;
};

export type RemoteServerComponent = {
  name: string;
  logKey: string;
  category: "system" | "map";
  state: string;
  tone: BadgeTone;
  summary: string;
  details: string[];
};

export type RemoteServerRecord = {
  type: RemoteServerKind;
  id: string;
  name: string;
  host: string;
  user: string;
  keyPath: string;
  port?: number;
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  phase: string;
};
