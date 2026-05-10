export type BattleGroupSummary = {
  namespace: string;
  name: string;
  title: string;
  phase: string;
  stop: boolean;
  serverImage: string;
  fileBrowserUrl?: string | null;
  directorUrl?: string | null;
  serverSets: number;
};

export type ServerSetSummary = {
  map: string;
  replicas: number;
  memoryLimit: string;
  dedicatedScaling: boolean;
  image: string;
};

export type BattleGroupDetail = {
  namespace: string;
  name: string;
  title: string;
  phase: string;
  stop: boolean;
  databasePhase: string;
  serverGroupPhase: string;
  gatewayPhase: string;
  directorPhase: string;
  serverImage: string;
  utilityImages: string[];
  serverSets: ServerSetSummary[];
};
