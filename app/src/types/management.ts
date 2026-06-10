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
  { id: "prepare-host", label: "Prepare host directories" },
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
  | "currency"
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
  /**
   * Frontend-synthetic grant commands present a dedicated locked form but publish
   * through a real engine command. When set, publish uses `publishAs` as the
   * ServerCommand id and merges `lockedFields` into the payload (e.g. Grant Solari
   * publishes AddItemToInventory with ItemName locked to "solari").
   */
  publishAs?: string;
  lockedFields?: Record<string, unknown>;
  /**
   * "DB grant" commands write directly to the game database through a dedicated
   * management-service endpoint instead of publishing an engine MQ command.
   * `grant_currency` UPSERTs dune.player_virtual_currency_balances (House Scrip);
   * the target currencyId rides in `lockedFields`.
   */
  dbAction?: "grant_currency";
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
  /**
   * Master switches for the daily restart, update loop, and scheduled backups.
   * Optional so older service builds (which omit them) read as undefined;
   * callers should treat undefined as enabled (the default).
   */
  restartEnabled?: boolean;
  updateEnabled?: boolean;
  backupEnabled?: boolean;
  /** null = scheduled backups disabled; otherwise the 5-field cron string. */
  backupCron: string | null;
  welcomeMessageEnabled: boolean;
  welcomePackageEnabled: boolean;
  welcomePackageVersion: string;
  welcomePackageActionsJson: string;
  /** Backward-compatible alias returned by older service builds. */
  welcomePackageItemsJson: string;
  welcomeWhisperSourcePlayer: string;
  welcomeMessage: string;
  restartRequired: boolean;
};

export type ScheduleConfigUpdate = Partial<{
  restartHour: number;
  restartMinute: number;
  restartWarningFrequencySecs: number;
  restartWarningDurationSecs: number;
  updateLeadSecs: number;
  restartTz: string;
  restartEnabled: boolean;
  updateEnabled: boolean;
  backupEnabled: boolean;
  /** Empty string clears the cron (disables); non-empty validated server-side. */
  backupCron: string;
  welcomeMessageEnabled: boolean;
  welcomePackageEnabled: boolean;
  welcomePackageVersion: string;
  welcomePackageActionsJson: string;
  /** Older service builds accept this; newer builds map it to actions. */
  welcomePackageItemsJson: string;
  welcomeWhisperSourcePlayer: string;
  welcomeMessage: string;
}>;

export type CronPreviewResult =
  | { ok: true; tz: string; next: string[] }
  | { ok: false; error: string };

export type DumpPruneItem = {
  namespace: string;
  name: string;
  action: string;
  backup: string | null;
  phase: string;
  createdAt: string;
  ageDays: number;
};

export type DumpPruneTarget = {
  namespace: string;
  name: string;
};

export type DumpPruneResult = {
  deleted: string[];
  skipped: { namespace: string; name: string; reason: string }[];
};

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

export type WelcomeGrantDto = {
  playerId: string;
  packageVersion: string;
  accountId: number;
  characterName: string | null;
  status: "pending" | "granted" | "failed";
  detectedAt: string;
  updatedAt: string;
  grantedAt: string | null;
  attempts: number;
  lastOnlineStatus: string | null;
  firstOnlineAt: string | null;
  lastError: string | null;
};

export type PublishResultDto = {
  ok: boolean;
  command: string;
  output: string;
  error: string | null;
  inner: Record<string, unknown>;
};
