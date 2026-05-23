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
  serverGroupPhase: string;
  directorPhase: string;
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
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  phase: string;
};
