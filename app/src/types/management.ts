export type ManagementInstallRequest = {
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
  commandAuthToken?: string;
};

export type ManagementConnRequest = {
  host: string;
  user: string;
  keyPath?: string;
  port?: number;
};

export type ManagementInstallResult = {
  installed: boolean;
  started: boolean;
  initSystem: string;
  installedVersion: string | null;
  message: string;
};

export type ManagementServiceStatus = {
  installed: boolean;
  active: boolean;
  initSystem: string;
  installedVersion: string | null;
  bundledVersion: string;
  journalTail: string;
};

export type InstallProgressEvent = {
  step: string;
  status: "pending" | "running" | "ok" | "error";
  message: string | null;
};

export const INSTALL_STEPS: ReadonlyArray<{ id: string; label: string }> = [
  { id: "stop-old", label: "Stop existing service" },
  { id: "upload-binary", label: "Upload binary" },
  { id: "write-token", label: "Write command-auth token" },
  { id: "install-init", label: "Install init unit" },
  { id: "start-service", label: "Start service" },
  { id: "verify", label: "Verify" },
];

export type HealthDto = {
  ok: boolean;
  version: string;
  now: string;
};

export type RunDto = {
  id: number;
  taskId: string;
  trigger: "scheduled" | "manual" | "startup";
  dryRun: boolean;
  status: "running" | "success" | "failed" | "skipped";
  startedAt: string;
  finishedAt: string | null;
  durationMs: number | null;
  error: string | null;
};

export type LogDto = {
  id: number;
  createdAt: string;
  level: "info" | "warn" | "error";
  message: string;
  taskId: string | null;
  runId: number | null;
};

export type FieldKind = "string" | "int" | "float" | "bool" | "select" | "text";

export type SelectOption = {
  value: string;
  label: string;
};

export type FieldSpec = {
  key: string;
  label: string;
  kind: FieldKind;
  required?: boolean;
  default?: unknown;
  helper?: string;
  options?: SelectOption[];
};

export type Category =
  | "items"
  | "movement"
  | "broadcast"
  | "progression"
  | "player"
  | "journey"
  | "exec";

export type CommandSpec = {
  id: string;
  label: string;
  category: Category;
  destructive?: boolean;
  needsPlayer: boolean;
  allowAllPlayers: boolean;
  describe: string;
  fields: FieldSpec[];
};

export type ItemDto = {
  id: string;
  name: string;
  category: string;
  source: string;
};

export type VehicleDto = {
  id: string;
  actor_class: string;
  templates: string[];
};

export type SkillModuleDto = {
  id: string;
  name: string;
  category: string;
  maxLevel: number;
};

export type JourneyNodeDto = {
  id: string;
  label: string;
  card: string;
  category: string;
};

export type XpEventTagDto = {
  id: string;
  family: string;
  constant: string;
};

export type ScheduleConfig = {
  restartHour: number;
  restartMinute: number;
  restartWarningFrequencySecs: number;
  restartWarningDurationSecs: number;
  updateLeadSecs: number;
  restartTz: string;
  restartRequired: boolean;
};

export type ScheduleConfigUpdate = Partial<{
  restartHour: number;
  restartMinute: number;
  restartWarningFrequencySecs: number;
  restartWarningDurationSecs: number;
  updateLeadSecs: number;
  restartTz: string;
}>;

export type PlayerLocationDto = {
  x: number;
  y: number;
  z: number;
  dimensionIndex: number | null;
  partitionId: number | null;
  /// Pawn actor class — e.g. "…BP_DunePlayerCharacter_C". Useful sanity.
  source: string;
};

export type RestartNoticeOptions = {
  leadSecs?: number;
  frequencySecs?: number;
  durationSecs?: number;
  title?: string;
  body?: string;
};

export type PlayerDto = {
  flsId: string;
  name: string;
  online: string;
  lastSeen: string;
  level: number | null;
  partitionId: number | null;
};

export type ClusterDto = {
  namespace: string;
  mqPod: string;
  dbPod: string | null;
  serviceVersion: string;
};

export type HistoryDto = {
  id: number;
  createdAt: string;
  command: string;
  payload: Record<string, unknown>;
  ok: boolean;
  message: string | null;
};

export type PublishResultDto = {
  ok: boolean;
  command: string;
  output: string;
  error: string | null;
  inner: Record<string, unknown>;
};
