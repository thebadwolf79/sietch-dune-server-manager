import { invoke } from "@tauri-apps/api/core";

import type {
  ClusterDto,
  CommandSpec,
  HealthDto,
  HistoryDto,
  ItemDto,
  LogDto,
  ManagementConnRequest,
  ManagementInstallRequest,
  ManagementInstallResult,
  ManagementServiceStatus,
  PlayerDto,
  PlayerLocationDto,
  PublishResultDto,
  RunDto,
  JourneyNodeDto,
  ScheduleConfig,
  ScheduleConfigUpdate,
  SkillModuleDto,
  VehicleDto,
  XpEventTagDto,
} from "../types/management";

export const managementService = {
  install: (req: ManagementInstallRequest) =>
    invoke<ManagementInstallResult>("install_management_service", { request: req }),
  uninstall: (req: ManagementConnRequest) =>
    invoke<void>("uninstall_management_service", { request: req }),
  status: (req: ManagementConnRequest) =>
    invoke<ManagementServiceStatus>("management_service_status", { request: req }),
  bundledVersion: () => invoke<string>("management_service_bundled_version"),
  restart: (req: ManagementConnRequest) =>
    invoke<void>("restart_management_service", { request: req }),
};

export const managementApi = {
  health: (tunnelId: string) => invoke<HealthDto>("ms_health", { tunnelId }),
  listRuns: (tunnelId: string, limit?: number, task?: string) =>
    invoke<RunDto[]>("ms_list_runs", { tunnelId, limit, task }),
  listLogs: (tunnelId: string, limit?: number, runId?: number) =>
    invoke<LogDto[]>("ms_list_logs", { tunnelId, limit, runId }),
  triggerRun: (tunnelId: string, task: string, options?: Record<string, unknown>) =>
    invoke<{ ok: boolean; task: string }>("ms_trigger_run", { tunnelId, task, options }),
  listCommands: (tunnelId: string) =>
    invoke<CommandSpec[]>("ms_list_commands", { tunnelId }),
  searchItems: (tunnelId: string, q: string, limit?: number) =>
    invoke<ItemDto[]>("ms_search_items", { tunnelId, q, limit }),
  searchVehicles: (tunnelId: string, q: string, limit?: number) =>
    invoke<VehicleDto[]>("ms_search_vehicles", { tunnelId, q, limit }),
  searchSkillModules: (tunnelId: string, q: string, limit?: number) =>
    invoke<SkillModuleDto[]>("ms_search_skill_modules", { tunnelId, q, limit }),
  searchJourneyNodes: (tunnelId: string, q: string, limit?: number) =>
    invoke<JourneyNodeDto[]>("ms_search_journey_nodes", { tunnelId, q, limit }),
  searchXpEventTags: (tunnelId: string, q: string, limit?: number) =>
    invoke<XpEventTagDto[]>("ms_search_xp_event_tags", { tunnelId, q, limit }),
  getConfig: (tunnelId: string) => invoke<ScheduleConfig>("ms_get_config", { tunnelId }),
  setConfig: (tunnelId: string, config: ScheduleConfigUpdate) =>
    invoke<{ ok: boolean }>("ms_set_config", { tunnelId, config }),
  listTimezones: (tunnelId: string) => invoke<string[]>("ms_list_timezones", { tunnelId }),
  searchPlayers: (tunnelId: string, q: string, limit?: number) =>
    invoke<PlayerDto[]>("ms_search_players", { tunnelId, q, limit }),
  playerLocation: (tunnelId: string, flsId: string) =>
    invoke<PlayerLocationDto>("ms_player_location", { tunnelId, flsId }),
  cluster: (tunnelId: string) => invoke<ClusterDto>("ms_cluster", { tunnelId }),
  history: (tunnelId: string, limit?: number) =>
    invoke<HistoryDto[]>("ms_history", { tunnelId, limit }),
  publish: (tunnelId: string, command: string, fields: Record<string, unknown>) =>
    invoke<PublishResultDto>("ms_publish", { tunnelId, command, fields }),
};
