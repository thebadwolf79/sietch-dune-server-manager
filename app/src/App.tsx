import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { AppHeader, AppSidebar, StatusStrip } from "./components/appShell";
import { useDashboardDerivedState } from "./hooks/useDashboardDerivedState";
import { useManagerTelemetry } from "./hooks/useManagerTelemetry";
import { BattleGroupsPanel } from "./views/battlegroups";
import type { BattleGroupLifecycle } from "./views/battlegroups";
import { ConfigView } from "./views/config";
import { DirectorView } from "./views/director";
import { EnvironmentPanel } from "./views/environment";
import { HostVmPanels, VmRequiredNotice } from "./views/hostVm";
import {
  DirectorStartingNotice,
  DirectorUnavailableNotice,
  ManagerApiPanel,
  ManagerToolsRequiredNotice
} from "./views/managerApi";
import { LogsPanel } from "./views/logs";
import { PlayersPanel } from "./views/players";
import { SetupView } from "./views/setup";
import { WorkloadsPanel } from "./views/workloads";
import type {
  AppConfig,
  BattleGroupDetail,
  BattleGroupSummary,
  CommandFailure,
  DirectorMapSummary,
  DirectorPlayerLists,
  DirectorPlayerSummary,
  FlsDraft,
  GuestBootstrapRequest,
  GuestConnection,
  HostStatus,
  ManagerApiInstallResult,
  ManagerApiStatus,
  ManagerLogResponse,
  ManagerSelfStatus,
  ManagerWorkloads,
  MapOverrideDraft,
  SetupCommandResult,
  SetupState,
  TransferDraft,
  ViewKey,
  VmImportOptions,
  VmStatus,
  Workloads
} from "./types";
import {
  asError,
  boolAt,
  defaultConfig,
  delay,
  generateToken,
  managerWorkloadsToUi,
  nullableNumber,
  numberAt,
  valueAt
} from "./utils";
import { isBattleGroupSettled, lifecycleStatusText } from "./domain/lifecycle";

type SetupOutputEvent = {
  stage: string;
  line: string;
};

export default function App() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [host, setHost] = useState<HostStatus | null>(null);
  const [vm, setVm] = useState<VmStatus | null>(null);
  const [guest, setGuest] = useState<GuestConnection | null>(null);
  const [battleGroups, setBattleGroups] = useState<BattleGroupSummary[]>([]);
  const [battleGroupDetail, setBattleGroupDetail] = useState<BattleGroupDetail | null>(null);
  const [battleGroupLifecycle, setBattleGroupLifecycle] = useState<BattleGroupLifecycle | null>(null);
  const [selectedNamespace, setSelectedNamespace] = useState<string>("");
  const [workloads, setWorkloads] = useState<Workloads | null>(null);
  const [errors, setErrors] = useState<CommandFailure[]>([]);
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [snapshotPath, setSnapshotPath] = useState<string>("");
  const [configSaved, setConfigSaved] = useState(false);
  const [configLoaded, setConfigLoaded] = useState(false);
  const [managerInstall, setManagerInstall] = useState<ManagerApiInstallResult | null>(null);
  const [managerSelf, setManagerSelf] = useState<ManagerSelfStatus | null>(null);
  const [directorPlayers, setDirectorPlayers] = useState<DirectorPlayerSummary | null>(null);
  const [directorPlayerLists, setDirectorPlayerLists] = useState<DirectorPlayerLists | null>(null);
  const [directorMaps, setDirectorMaps] = useState<DirectorMapSummary[]>([]);
  const [directorFlsConfig, setDirectorFlsConfig] = useState<Record<string, unknown> | null>(null);
  const [directorTransferConfig, setDirectorTransferConfig] = useState<Record<string, unknown> | null>(null);
  const [directorLoading, setDirectorLoading] = useState(false);
  const [setupState, setSetupState] = useState<SetupState | null>(null);
  const [vmImportOptions, setVmImportOptions] = useState<VmImportOptions | null>(null);
  const [setupLog, setSetupLog] = useState<SetupCommandResult | null>(null);
  const [setupOperation, setSetupOperation] = useState("");
  const [selectedDirectorMap, setSelectedDirectorMap] = useState("");
  const [flsDraft, setFlsDraft] = useState<FlsDraft>({ heartbeatSeconds: "", settingsSeconds: "" });
  const [transferDraft, setTransferDraft] = useState<TransferDraft>({
    deleteOrigin: true,
    incoming: "0",
    outgoing: false,
    exportTimeout: "",
    importTimeout: "",
    freeFrom: false,
    freeTo: false,
    validateTimeout: "",
    worldClosed: false,
    worldClosingSoon: false
  });
  const [mapOverrideDraft, setMapOverrideDraft] = useState<MapOverrideDraft>({
    playerHardCap: "",
    updatePlayerCountOnFls: false,
    enforceSameHomeDimension: false,
    automaticScaling: false,
    throttlingSeconds: "",
    minServers: "",
    extraServers: ""
  });
  const [activeView, setActiveView] = useState<ViewKey>("overview");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const {
    managerStatus,
    setManagerStatus,
    managerTelemetry,
    setManagerTelemetry,
    managerSocketState,
    managerError
  } = useManagerTelemetry(configLoaded, config);

  const {
    selectedBattleGroup,
    selectedDirectorMapSummary,
    vmIsRunning,
    vmIsStarting,
    vmIsChanging,
    canControlVm,
    startVmDisabledReason,
    stopVmDisabledReason,
    battleGroupIsStopped,
    battleGroupIsRunning,
    managerApiConfigured,
    managerReadiness,
    managerTelemetryState,
    canUseManager,
    managerToolsInstalled,
    directorAvailable,
    managerInstallNamespace,
    canInstallManagerApi,
    activeViewRequiresManager,
    activeViewRequiresDirector,
    pageTitle,
    pageSubtitle,
    navItems
  } = useDashboardDerivedState({
    config,
    host,
    vm,
    guest,
    battleGroups,
    selectedNamespace,
    directorMaps,
    selectedDirectorMap,
    busy,
    managerStatus,
    managerSocketState,
    activeView
  });

  async function capture<T>(label: string, fn: () => Promise<T>): Promise<T | null> {
    try {
      return await fn();
    } catch (error) {
      const commandError = asError(error);
      if (isSetupLabel(label)) {
        appendSetupLog({
          ok: false,
          stage: label.toLowerCase().replace(/\s+/g, "-"),
          message: `${label}: ${commandError.message}`,
          stdout: [commandError.stdout, commandError.stderr].filter(Boolean).join("\n")
        });
        return null;
      }
      setErrors((current) => [{ ...commandError, message: `${label}: ${commandError.message}` }, ...current]);
      return null;
    }
  }

  function isSetupLabel(label: string) {
    return [
      "Install SteamCMD",
      "Install server package",
      "Detect VM import options",
      "Import VM",
      "Guest bootstrap",
      "Install Manager API"
    ].includes(label);
  }

  function appendSetupLog(entry: SetupCommandResult) {
    setSetupLog((current) => {
      if (!current) return entry;
      const header = `--- ${entry.message} ---`;
      const nextStdout = [current.stdout, header, entry.stdout].filter(Boolean).join("\n");
      return {
        ...entry,
        stdout: nextStdout
      };
    });
  }

  async function managerRequest<T>(path: string, init?: RequestInit): Promise<T> {
    const baseUrl = config.managerApiUrl.trim().replace(/\/$/, "");
    if (!baseUrl) throw new Error("Manager API URL is not configured");
    const headers = new Headers(init?.headers);
    if (config.managerApiToken) {
      headers.set("Authorization", `Bearer ${config.managerApiToken}`);
    }
    const response = await fetch(`${baseUrl}${path}`, { ...init, headers });
    if (!response.ok) {
      const body = await response.text();
      throw new Error(body || `Manager API returned ${response.status}`);
    }
    return (await response.json()) as T;
  }

  async function refresh() {
    setRefreshing(true);
    setErrors([]);
    setSnapshotPath("");

    const nextHost = await capture("Host status", () => invoke<HostStatus>("get_host_status"));
    setHost(nextHost);

    const nextVm = await capture("VM status", () => invoke<VmStatus>("get_vm_status", { vmName: config.vmName }));
    setVm(nextVm);

    const nextVmState = nextVm?.state.toLowerCase() ?? "";
    if (nextVmState !== "running") {
      setGuest(null);
      setManagerStatus(null);
      setManagerSelf(null);
      setManagerTelemetry(null);
      setBattleGroups([]);
      setBattleGroupDetail(null);
      setBattleGroupLifecycle(null);
      setWorkloads(null);
      setDirectorPlayers(null);
      setDirectorPlayerLists(null);
      setDirectorMaps([]);
      setDirectorFlsConfig(null);
      setDirectorTransferConfig(null);
      setRefreshing(false);
      return;
    }

    const ip = nextVm?.ipAddresses?.[0] ?? guest?.ip ?? config.vmIp;
    if (!ip) {
      setGuest(null);
      setManagerStatus(null);
      setRefreshing(false);
      return;
    }

    const nextGuest = await capture("Guest connection", () =>
      invoke<GuestConnection>("connect_guest", { installPath: config.installPath, ip, sshUser: config.sshUser })
    );
    setGuest(nextGuest);
    if (nextGuest?.ip && nextGuest.ip !== config.vmIp) {
      const updatedConfig = { ...config, vmIp: nextGuest.ip };
      setConfig(updatedConfig);
      void invoke<AppConfig>("save_app_config", { config: updatedConfig });
    }

    const nextManagerStatus = managerApiConfigured
      ? await capture("Manager API status", () => managerRequest<ManagerApiStatus>("/api/status"))
      : null;
    setManagerStatus(nextManagerStatus);
    const nextManagerSelf = managerApiConfigured
      ? await capture("Manager API self", () => managerRequest<ManagerSelfStatus>("/api/manager/self"))
      : null;
    setManagerSelf(nextManagerSelf);

    let nextBattleGroups = managerApiConfigured
      ? await capture("Manager BattleGroups", () => managerRequest<BattleGroupSummary[]>("/api/battlegroups"))
      : null;
    if (!nextBattleGroups && nextGuest?.kubectl && nextGuest.sudo) {
      nextBattleGroups = await capture("Initial BattleGroup discovery", () =>
        invoke<BattleGroupSummary[]>("get_battlegroups", {
          installPath: config.installPath,
          ip: nextGuest.ip ?? ip,
          sshUser: config.sshUser
        })
      );
    }
    if (nextBattleGroups) {
      setBattleGroups(nextBattleGroups);
      const nextSelected = nextBattleGroups.some((group) => group.namespace === selectedNamespace)
        ? selectedNamespace
        : nextBattleGroups[0]?.namespace ?? "";
      setSelectedNamespace(nextSelected);
      const group = nextBattleGroups.find((candidate) => candidate.namespace === nextSelected);
      if (group) {
        await Promise.all([loadWorkloads(group.namespace), loadBattleGroupDetail(group)]);
      }
    }
    if (managerApiConfigured) {
      await loadDirectorData();
    }
    setRefreshing(false);
  }

  async function loadWorkloads(_namespace: string) {
    const nextWorkloads = await capture("Manager workloads", () => managerRequest<ManagerWorkloads>("/api/workloads"));
    setWorkloads(nextWorkloads ? managerWorkloadsToUi(nextWorkloads) : null);
  }

  async function loadBattleGroupDetail(group: BattleGroupSummary) {
    const detail = await capture("BattleGroup detail", () =>
      managerRequest<BattleGroupDetail>(
        `/api/battlegroups/${encodeURIComponent(group.namespace)}/${encodeURIComponent(group.name)}`
      )
    );
    setBattleGroupDetail(detail);
  }

  async function loadDirectorData() {
    setDirectorLoading(true);
    const [players, playerLists, maps, flsConfig, transferConfig] = await Promise.all([
      capture("Director players", () => managerRequest<DirectorPlayerSummary>("/api/director/players/summary")),
      capture("Director player lists", () => managerRequest<DirectorPlayerLists>("/api/director/players")),
      capture("Director maps", () => managerRequest<DirectorMapSummary[]>("/api/director/maps")),
      capture("Director FLS config", () => managerRequest<Record<string, unknown>>("/api/director/config/fls")),
      capture("Director character transfer config", () =>
        managerRequest<Record<string, unknown>>("/api/director/config/character-transfer")
      )
    ]);
    if (players) setDirectorPlayers(players);
    if (playerLists) setDirectorPlayerLists(playerLists);
    if (maps) setDirectorMaps(maps);
    if (flsConfig) setDirectorFlsConfig(flsConfig);
    if (transferConfig) setDirectorTransferConfig(transferConfig);
    setDirectorLoading(false);
  }

  function buildFlsPayload() {
    if (!flsDraft.heartbeatSeconds || !flsDraft.settingsSeconds) return null;
    return {
      FlsServerHeartbeatUpdateFrequencySeconds: Number(flsDraft.heartbeatSeconds),
      FlsServerSettingsUpdateFrequencySeconds: Number(flsDraft.settingsSeconds)
    };
  }

  function buildCurrentFlsPayload() {
    if (!directorFlsConfig) return null;
    return {
      FlsServerHeartbeatUpdateFrequencySeconds: Number(
        valueAt(directorFlsConfig, ["config", "flsServerHeartbeatUpdateFrequencySeconds"])
      ),
      FlsServerSettingsUpdateFrequencySeconds: Number(
        valueAt(directorFlsConfig, ["config", "flsServerSettingsUpdateFrequencySeconds"])
      )
    };
  }

  function buildTransferPayload() {
    if (!transferDraft.exportTimeout || !transferDraft.importTimeout || !transferDraft.validateTimeout) return null;
    return {
      ShouldDeleteOriginCharactersDuringTransfers: transferDraft.deleteOrigin,
      IncomingCharacterTransfers: Number(transferDraft.incoming),
      AcceptOutgoingCharacterTransfers: transferDraft.outgoing,
      ExportCharacterTimeout: Number(transferDraft.exportTimeout),
      ImportCharacterTimeout: Number(transferDraft.importTimeout),
      FreeToTransferCharactersFrom: transferDraft.freeFrom,
      FreeToTransferCharactersTo: transferDraft.freeTo,
      ValidateBeforeImportCharacterTimeout: Number(transferDraft.validateTimeout),
      ForceIsWorldClosed: transferDraft.worldClosed,
      ForceIsWorldClosingSoon: transferDraft.worldClosingSoon
    };
  }

  function buildCurrentTransferPayload() {
    if (!directorTransferConfig) return null;
    return {
      ShouldDeleteOriginCharactersDuringTransfers: boolAt(
        directorTransferConfig,
        ["config", "shouldDeleteOriginCharactersDuringTransfers"],
        true
      ),
      IncomingCharacterTransfers: Number(valueAt(directorTransferConfig, ["config", "incomingCharacterTransfers"])),
      AcceptOutgoingCharacterTransfers: boolAt(directorTransferConfig, ["config", "acceptOutgoingCharacterTransfers"]),
      ExportCharacterTimeout: Number(valueAt(directorTransferConfig, ["config", "exportCharacterTimeout"])),
      ImportCharacterTimeout: Number(valueAt(directorTransferConfig, ["config", "importCharacterTimeout"])),
      FreeToTransferCharactersFrom: boolAt(directorTransferConfig, ["config", "freeToTransferCharactersFrom"]),
      FreeToTransferCharactersTo: boolAt(directorTransferConfig, ["config", "freeToTransferCharactersTo"]),
      ValidateBeforeImportCharacterTimeout: Number(
        valueAt(directorTransferConfig, ["config", "validateBeforeImportCharacterTimeout"])
      ),
      ForceIsWorldClosed: boolAt(directorTransferConfig, ["config", "forceIsWorldClosed"]),
      ForceIsWorldClosingSoon: boolAt(directorTransferConfig, ["config", "forceIsWorldClosingSoon"])
    };
  }

  function buildMapOverridePayload() {
    if (!selectedDirectorMapSummary) return null;
    const mapName = selectedDirectorMapSummary.name;
    return selectedDirectorMapSummary.kind === "Dimension"
      ? {
          MapName: mapName,
          DimensionServerGroupConfig: {
            EnforceSameHomeDimensionForAll: mapOverrideDraft.enforceSameHomeDimension,
            PlayerHardCap: nullableNumber(mapOverrideDraft.playerHardCap),
            ShouldUpdatePlayerCountOnFls: mapOverrideDraft.updatePlayerCountOnFls,
            DimensionOverrides: null
          }
        }
      : selectedDirectorMapSummary.kind === "Instanced"
        ? {
            MapName: mapName,
            ClassicalInstancingGroupConfig: {
              PlayerHardCap: nullableNumber(mapOverrideDraft.playerHardCap),
              ShouldUpdatePlayerCountOnFls: mapOverrideDraft.updatePlayerCountOnFls,
              EnableAutomaticInstanceScaling: mapOverrideDraft.automaticScaling,
              InstanceScalingThrottlingSeconds: nullableNumber(mapOverrideDraft.throttlingSeconds),
              MinServers: nullableNumber(mapOverrideDraft.minServers),
              NumExtraServers: nullableNumber(mapOverrideDraft.extraServers)
            }
          }
        : {
            MapName: mapName,
            SingleServerConfig: {
              PlayerHardCap: nullableNumber(mapOverrideDraft.playerHardCap),
              ShouldUpdatePlayerCountOnFls: mapOverrideDraft.updatePlayerCountOnFls
            }
          };
  }

  async function saveFlsConfig() {
    setBusy(true);
    await capture("Update Director FLS config", () =>
      managerRequest<Record<string, unknown>>("/api/director/config/fls", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(buildFlsPayload())
      })
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function clearFlsConfig() {
    setBusy(true);
    await capture("Clear Director FLS overrides", () =>
      managerRequest<Record<string, unknown>>("/api/director/config/fls", { method: "DELETE" })
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function saveTransferConfig() {
    setBusy(true);
    await capture("Update Director character transfer config", () =>
      managerRequest<Record<string, unknown>>("/api/director/config/character-transfer", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(buildTransferPayload())
      })
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function clearTransferConfig() {
    setBusy(true);
    await capture("Clear Director character transfer overrides", () =>
      managerRequest<Record<string, unknown>>("/api/director/config/character-transfer", { method: "DELETE" })
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function saveMapOverride() {
    if (!selectedDirectorMapSummary) return;
    const mapName = selectedDirectorMapSummary.name;
    const config = buildMapOverridePayload();

    setBusy(true);
    await capture("Update Director map override", () =>
      managerRequest<Record<string, unknown>>(
        `/api/director/config/maps/${encodeURIComponent(mapName)}/override`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(config)
        }
      )
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function clearMapOverride(mapName = selectedDirectorMapSummary?.name) {
    if (!mapName) return;
    setBusy(true);
    await capture("Clear Director map override", () =>
      managerRequest<Record<string, unknown>>(
        `/api/director/config/maps/${encodeURIComponent(mapName)}/override`,
        { method: "DELETE" }
      )
    );
    await loadDirectorData();
    setBusy(false);
  }

  async function startVm() {
    setBusy(true);
    if (vm) {
      setVm({ ...vm, state: "Starting", status: "Starting" });
    }
    const nextVm = await capture("Start VM", () => invoke<VmStatus>("start_vm", { vmName: config.vmName }));
    if (nextVm && nextVm.state.toLowerCase() !== "off") {
      setVm(nextVm);
    }
    await pollVmLifecycle("running", "Starting", "off");
    setBusy(false);
  }

  async function stopVm() {
    setBusy(true);
    if (vm) {
      setVm({ ...vm, state: "Stopping", status: "Stopping" });
    }
    const nextVm = await capture("Stop VM", () => invoke<VmStatus>("stop_vm", { vmName: config.vmName }));
    if (nextVm && nextVm.state.toLowerCase() !== "running") {
      setVm(nextVm);
    }
    await pollVmLifecycle("off", "Stopping", "running");
    setBusy(false);
  }

  async function pollVmLifecycle(targetState: string, transitionState: string, staleState: string) {
    let lastVm: VmStatus | null = null;
    for (let index = 0; index < 20; index += 1) {
      await delay(1500);
      const nextVm = await capture("VM lifecycle", () => invoke<VmStatus>("get_vm_status", { vmName: config.vmName }));
      if (!nextVm) return;
      lastVm = nextVm;
      const nextState = nextVm.state.toLowerCase();
      if (nextState === targetState) {
        setVm(nextVm);
        return;
      }
      setVm(nextState === staleState ? { ...nextVm, state: transitionState, status: transitionState } : nextVm);
    }
    if (lastVm) setVm(lastVm);
  }

  async function setBattleGroupRunning(running: boolean) {
    if (!selectedBattleGroup) return;
    const action = running ? "start" : "stop";
    const target = running ? "running" : "stopped";
    const requestedAt = Date.now();
    setBattleGroupLifecycle({
      action,
      target,
      requestedAt,
      status: running ? "Start requested" : "Stop requested",
      lastPhase: selectedBattleGroup.phase
    });
    const detail = await capture(running ? "Start battlegroup" : "Stop battlegroup", () =>
      managerRequest<BattleGroupDetail>(
        `/api/battlegroups/${encodeURIComponent(selectedBattleGroup.namespace)}/${encodeURIComponent(
          selectedBattleGroup.name
        )}/${running ? "start" : "stop"}`,
        { method: "POST" }
      )
    );
    if (detail) setBattleGroupDetail(detail);
    await pollBattleGroupLifecycle(selectedBattleGroup, target, requestedAt, action);
  }

  async function restartBattleGroup() {
    if (!selectedBattleGroup) return;
    const requestedAt = Date.now();
    setBattleGroupLifecycle({
      action: "restart",
      target: "running",
      requestedAt,
      status: "Restart requested",
      lastPhase: selectedBattleGroup.phase
    });
    const detail = await capture("Restart battlegroup", () =>
      managerRequest<BattleGroupDetail>(
        `/api/battlegroups/${encodeURIComponent(selectedBattleGroup.namespace)}/${encodeURIComponent(
          selectedBattleGroup.name
        )}/restart`,
        { method: "POST" }
      )
    );
    if (detail) setBattleGroupDetail(detail);
    await pollBattleGroupLifecycle(selectedBattleGroup, "running", requestedAt, "restart");
  }

  async function pollBattleGroupLifecycle(
    group: BattleGroupSummary,
    target: "running" | "stopped",
    requestedAt: number,
    action: BattleGroupLifecycle["action"]
  ) {
    for (let index = 0; index < 80; index += 1) {
      await delay(index === 0 ? 750 : 3000);
      const detail = await capture("BattleGroup lifecycle", () =>
        managerRequest<BattleGroupDetail>(
          `/api/battlegroups/${encodeURIComponent(group.namespace)}/${encodeURIComponent(group.name)}`
        )
      );
      const groups = await capture("BattleGroup list", () => managerRequest<BattleGroupSummary[]>("/api/battlegroups"));
      if (groups) setBattleGroups(groups);
      if (detail) {
        setBattleGroupDetail(detail);
        setBattleGroupLifecycle({
          action,
          target,
          requestedAt,
          status: lifecycleStatusText(action, detail),
          lastPhase: detail.phase
        });
        await loadWorkloads(group.namespace);
        if (isBattleGroupSettled(detail, target)) {
          setBattleGroupLifecycle(null);
          if (target === "running") {
            await loadDirectorData();
          }
          return;
        }
      }
    }
    setBattleGroupLifecycle((current) =>
      current
        ? {
            ...current,
            status: "Still waiting for a settled phase"
          }
        : current
    );
  }

  async function exportLiveConfig() {
    if (!selectedBattleGroup) return;
    setBusy(true);
    const snapshot = await capture("Export live config", () =>
      managerRequest<Record<string, unknown>>(
        `/api/battlegroups/${encodeURIComponent(selectedBattleGroup.namespace)}/${encodeURIComponent(
          selectedBattleGroup.name
        )}/raw`
      )
    );
    if (snapshot) {
      const blob = new Blob([JSON.stringify(snapshot, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `${selectedBattleGroup.name}-live.json`;
      anchor.click();
      URL.revokeObjectURL(url);
      setSnapshotPath(`${selectedBattleGroup.name}-live.json`);
    }
    setBusy(false);
  }

  async function loadLogs(pod: string, container: string, tail: number) {
    const query = new URLSearchParams({ pod, tail: String(tail) });
    if (container) query.set("container", container);
    return capture("Load logs", () => managerRequest<ManagerLogResponse>(`/api/logs?${query.toString()}`));
  }

  function backupConfig(name: string, value: unknown) {
    if (!value) return;
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    const blob = new Blob([JSON.stringify(value, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${name}-${stamp}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  async function saveConfig(nextConfig = config) {
    setBusy(true);
    const saved = await capture("Save config", () => invoke<AppConfig>("save_app_config", { config: nextConfig }));
    if (saved) {
      setConfig(saved);
      setConfigSaved(true);
      window.setTimeout(() => setConfigSaved(false), 2200);
    }
    setBusy(false);
  }

  async function detectEnvironment() {
    setBusy(true);
    const detected = await capture("Detect environment", () => invoke<AppConfig>("detect_app_config"));
    if (detected) {
      setConfig(detected);
    }
    setBusy(false);
  }

  async function installManagerApi() {
    const namespace = managerInstallNamespace;
    const token = config.managerApiToken || generateToken();
    const nextConfig = {
      ...config,
      managerApiNamespace: namespace,
      managerApiToken: token
    };

    setBusy(true);
    setSetupOperation("Installing Manager API");
    appendSetupLog({
      ok: false,
      stage: "manager-api",
      message: "Installing Manager API",
      stdout: `Namespace: ${namespace}`
    });
    setConfig(nextConfig);
    const result = await capture("Install Manager API", () =>
      invoke<ManagerApiInstallResult>("install_manager_api", {
        namespace,
        binaryPath: nextConfig.managerApiBinaryPath,
        token,
        directorBaseUrl: nextConfig.managerApiDirectorUrl,
        installPath: nextConfig.installPath,
        ip: guest?.ip ?? nextConfig.vmIp,
        sshUser: nextConfig.sshUser
      })
    );
    if (result) {
      setManagerInstall(result);
      appendSetupLog({
        ok: true,
        stage: "manager-api",
        message: "Manager API installed",
        stdout: `URL: ${result.url}`
      });
      const savedConfig = { ...nextConfig, managerApiNamespace: result.namespace, managerApiUrl: result.url };
      setConfig(savedConfig);
      await capture("Save Manager API config", () => invoke<AppConfig>("save_app_config", { config: savedConfig }));
    }
    setSetupOperation("");
    setBusy(false);
  }

  async function refreshSetupState() {
    const state = await capture("Setup state", () => invoke<SetupState>("detect_setup_state"));
    if (state) setSetupState(state);
  }

  async function detectSteamCmd() {
    setBusy(true);
    const state = await capture("SteamCMD detection", () => invoke<SetupState>("detect_setup_state"));
    if (state) setSetupState(state);
    setBusy(false);
  }

  async function installSteamCmd(installDir: string) {
    setBusy(true);
    setSetupOperation("Installing SteamCMD");
    appendSetupLog({
      ok: false,
      stage: "steamcmd",
      message: "Installing SteamCMD",
      stdout: `Destination: ${installDir}`
    });
    const detected = await capture("Install SteamCMD", () => invoke("install_steamcmd", { installDir }));
    if (detected) {
      appendSetupLog({
        ok: true,
        stage: "steamcmd",
        message: "SteamCMD installed",
        stdout: `SteamCMD is ready at ${installDir}`
      });
      await refreshSetupState();
      const loaded = await capture("Reload config", () => invoke<AppConfig>("get_app_config"));
      if (loaded) setConfig(loaded);
    }
    setSetupOperation("");
    setBusy(false);
  }

  async function installServerApp(steamcmdPath: string, installDir: string) {
    setBusy(true);
    setSetupOperation("Installing server package");
    appendSetupLog({
      ok: false,
      stage: "server-app",
      message: "Installing server package",
      stdout: `Destination: ${installDir}`
    });
    const result = await capture("Install server package", () =>
      invoke<SetupCommandResult>("install_server_app", { steamcmdPath, installDir })
    );
    if (result) appendSetupLog(result);
    const loaded = await capture("Reload config", () => invoke<AppConfig>("get_app_config"));
    if (loaded) setConfig(loaded);
    await refreshSetupState();
    setSetupOperation("");
    setBusy(false);
  }

  async function detectVmOptions(installPath: string) {
    setBusy(true);
    appendSetupLog({
      ok: false,
      stage: "vm-import",
      message: "Detecting VM import options",
      stdout: `Server package: ${installPath}`
    });
    const options = await capture("Detect VM import options", () =>
      invoke<VmImportOptions>("detect_vm_import_options", { installPath: installPath || null })
    );
    if (options) {
      setVmImportOptions(options);
      appendSetupLog({
        ok: true,
        stage: "vm-import",
        message: "VM import options detected",
        stdout: options.existingVm
          ? `Existing VM detected: ${options.existingVmState || "state unknown"}`
          : `Suggested destination: ${options.suggestedDestination || "none"}`
      });
    }
    setBusy(false);
  }

  async function importVm(
    installPath: string,
    destinationPath: string,
    memoryGb: number,
    switchName: string,
    physicalAdapterName: string,
    clearDestination: boolean
  ) {
    setBusy(true);
    setSetupOperation("Importing VM");
    appendSetupLog({
      ok: false,
      stage: "vm-import",
      message: "Importing VM",
      stdout: `Destination: ${destinationPath}`
    });
    const result = await capture("Import VM", () =>
      invoke<SetupCommandResult>("run_vm_import_stage", {
        installPath,
        destinationPath,
        memoryGb,
        switchName,
        physicalAdapterName,
        clearDestination
      })
    );
    if (result) appendSetupLog(result);
    const loaded = await capture("Reload config", () => invoke<AppConfig>("get_app_config"));
    if (loaded) setConfig(loaded);
    await refreshSetupState();
    await refresh();
    setSetupOperation("");
    setBusy(false);
  }

  async function bootstrapGuest(request: GuestBootstrapRequest) {
    setBusy(true);
    setSetupOperation("Bootstrapping guest VM");
    if (setupState) {
      void invoke("save_setup_state", {
        state: {
          ...setupState.persisted,
          selections: {
            ...setupState.persisted.selections,
            serverInstallDir: request.installPath,
            staticIp: request.staticIp,
            staticCidr: request.staticCidr,
            staticGateway: request.staticGateway,
            staticDns: request.staticDns,
            manualPlayerIp: request.playerIp,
            worldName: request.worldName,
            worldRegion: request.region,
            bootstrapProfileId: request.profileId
          }
        }
      });
    }
    appendSetupLog({
      ok: false,
      stage: "guest-bootstrap",
      message: "Bootstrapping guest VM",
      stdout: `Guest IP: ${request.ip}\nPlayer-facing IP: ${request.playerIp}\nWorld: ${request.worldName}\nRegion: ${request.region}\nProfile: ${request.profileId}`
    });
    const result = await capture("Guest bootstrap", () =>
      invoke<SetupCommandResult>("run_guest_bootstrap_stage", {
        request
      })
    );
    if (result) {
      appendSetupLog(result);
      const loaded = await capture("Reload config", () => invoke<AppConfig>("get_app_config"));
      if (loaded) setConfig(loaded);
      await refreshSetupState();
      setSetupOperation("");
      setBusy(false);
      void refresh();
      return;
    } else {
      setSetupOperation("");
      setBusy(false);
      void refreshSetupState();
      return;
    }
    setSetupOperation("");
    setBusy(false);
  }

  useEffect(() => {
    void (async () => {
      const loaded = await capture("Detect environment", () => invoke<AppConfig>("detect_app_config"));
      if (loaded) {
        setConfig(loaded);
      }
      setConfigLoaded(true);
    })();
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlisten = listen<SetupOutputEvent>("setup-output", ({ payload }) => {
      if (disposed) return;
      setSetupLog((current) => {
        const rawLine = payload.line.trimEnd();
        if (!rawLine) return current;
        if (!current) {
          return {
            ok: false,
            stage: payload.stage,
            message: "Running setup command",
            stdout: rawLine
          };
        }
        const nextLine = current.stage === payload.stage ? rawLine : `[${payload.stage}] ${rawLine}`;
        const stdout = current.stdout ? `${current.stdout}\n${nextLine}` : nextLine;
        return { ...current, stdout };
      });
    });

    return () => {
      disposed = true;
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    if (configLoaded) {
      void refresh();
      void refreshSetupState();
    }
  }, [
    configLoaded,
    config.vmName,
    config.installPath,
    config.sshUser,
    config.vmIp,
    config.managerApiUrl,
    config.managerApiToken,
    config.managerApiNamespace
  ]);

  useEffect(() => {
    if (activeViewRequiresManager && !managerToolsInstalled) {
      setActiveView("manager");
    } else if (activeViewRequiresDirector && !directorAvailable) {
      setActiveView("manager");
    }
  }, [activeViewRequiresDirector, activeViewRequiresManager, directorAvailable, managerToolsInstalled]);

  useEffect(() => {
    if (!directorFlsConfig) return;
    setFlsDraft({
      heartbeatSeconds: numberAt(directorFlsConfig, ["config", "flsServerHeartbeatUpdateFrequencySeconds"]),
      settingsSeconds: numberAt(directorFlsConfig, ["config", "flsServerSettingsUpdateFrequencySeconds"])
    });
  }, [directorFlsConfig]);

  useEffect(() => {
    if (!directorTransferConfig) return;
    setTransferDraft({
      deleteOrigin: boolAt(directorTransferConfig, ["config", "shouldDeleteOriginCharactersDuringTransfers"], true),
      incoming: numberAt(directorTransferConfig, ["config", "incomingCharacterTransfers"], "0"),
      outgoing: boolAt(directorTransferConfig, ["config", "acceptOutgoingCharacterTransfers"]),
      exportTimeout: numberAt(directorTransferConfig, ["config", "exportCharacterTimeout"]),
      importTimeout: numberAt(directorTransferConfig, ["config", "importCharacterTimeout"]),
      freeFrom: boolAt(directorTransferConfig, ["config", "freeToTransferCharactersFrom"]),
      freeTo: boolAt(directorTransferConfig, ["config", "freeToTransferCharactersTo"]),
      validateTimeout: numberAt(directorTransferConfig, ["config", "validateBeforeImportCharacterTimeout"]),
      worldClosed: boolAt(directorTransferConfig, ["config", "forceIsWorldClosed"]),
      worldClosingSoon: boolAt(directorTransferConfig, ["config", "forceIsWorldClosingSoon"])
    });
  }, [directorTransferConfig]);

  useEffect(() => {
    if (!selectedDirectorMap && directorMaps.length > 0) {
      setSelectedDirectorMap(directorMaps[0].name);
    }
  }, [directorMaps, selectedDirectorMap]);

  const pods = workloads?.pods.items ?? [];
  const services = workloads?.services.items ?? [];
  const flsPreview = useMemo(() => buildFlsPayload(), [flsDraft]);
  const transferPreview = useMemo(() => buildTransferPayload(), [transferDraft]);
  const currentFlsPayload = useMemo(() => buildCurrentFlsPayload(), [directorFlsConfig]);
  const currentTransferPayload = useMemo(() => buildCurrentTransferPayload(), [directorTransferConfig]);
  const flsChanged = Boolean(flsPreview && JSON.stringify(flsPreview) !== JSON.stringify(currentFlsPayload));
  const transferChanged = Boolean(
    transferPreview && JSON.stringify(transferPreview) !== JSON.stringify(currentTransferPayload)
  );
  const mapOverridePreview = useMemo(
    () => buildMapOverridePayload(),
    [selectedDirectorMapSummary, mapOverrideDraft]
  );

  return (
    <main className={`app-shell ${sidebarCollapsed ? "sidebar-collapsed" : ""}`}>
      <AppSidebar
        navItems={navItems}
        activeView={activeView}
        collapsed={sidebarCollapsed}
        onToggleCollapsed={() => setSidebarCollapsed((value) => !value)}
        onSelect={setActiveView}
      />

      <section className="content">
        <AppHeader title={pageTitle} subtitle={pageSubtitle} busy={busy || refreshing} onRefresh={refresh} />

        <StatusStrip
          admin={host?.isElevated ?? false}
          vmState={vm?.state}
          sshConnected={guest?.connected ?? false}
          kubectlReady={guest?.kubectl ?? false}
          battleGroupPhase={selectedBattleGroup?.phase}
          managerReadiness={managerReadiness}
        />

        {activeView === "setup" && (
          <SetupView
            config={config}
            setupState={setupState}
            vmImportOptions={vmImportOptions}
            setupLog={setupLog}
            busy={busy}
            setupOperation={setupOperation}
            canInstallManagerApi={canInstallManagerApi}
            onRefreshSetup={refreshSetupState}
            onInstallSteamCmd={installSteamCmd}
            onInstallServerApp={installServerApp}
            onDetectVmOptions={detectVmOptions}
            onImportVm={importVm}
            onBootstrapGuest={bootstrapGuest}
            onInstallManagerApi={installManagerApi}
          />
        )}

        {(activeView === "overview" || activeView === "config") && (
          <EnvironmentPanel
            config={config}
            vm={vm}
            managerInstallNamespace={managerInstallNamespace}
            configSaved={configSaved}
            busy={busy}
            onDetect={detectEnvironment}
          />
        )}

        {errors.length > 0 && (
          <section className="error-list">
            {errors.map((error, index) => (
              <div key={`${error.message}-${index}`}>
                <strong>{error.message}</strong>
                {(error.stdout || error.stderr) && (
                  <div className="error-output">
                    {(error.stdout || error.stderr || "")
                      .split(/\r?\n/)
                      .filter(Boolean)
                      .map((line, lineIndex) => (
                        <span key={`${error.message}-${index}-${lineIndex}`}>{line}</span>
                      ))}
                  </div>
                )}
              </div>
            ))}
          </section>
        )}

        {activeView !== "setup" && vm && !vmIsRunning && (
          <VmRequiredNotice
            vm={vm}
            busy={busy}
            canControlVm={canControlVm}
            vmIsRunning={vmIsRunning}
            vmIsChanging={vmIsChanging}
            vmIsStarting={vmIsStarting}
            onStart={startVm}
          />
        )}

        {(activeView === "overview" || activeView === "host") && (
          <HostVmPanels
            host={host}
            vm={vm}
            guest={guest}
            busy={busy}
            canControlVm={canControlVm}
            vmIsRunning={vmIsRunning}
            vmIsChanging={vmIsChanging}
            startVmDisabledReason={startVmDisabledReason}
            stopVmDisabledReason={stopVmDisabledReason}
            onStart={startVm}
            onStop={stopVm}
          />
        )}

        {(activeView === "overview" || activeView === "manager") && (
          <ManagerApiPanel
            config={config}
            managerInstallNamespace={managerInstallNamespace}
            managerReadiness={managerReadiness}
            managerTelemetryState={managerTelemetryState}
            managerStatus={managerStatus}
            managerSelf={managerSelf}
            managerTelemetry={managerTelemetry}
            managerInstall={managerInstall}
            managerError={managerError}
            busy={busy}
            canInstallManagerApi={canInstallManagerApi}
            onInstall={installManagerApi}
          />
        )}

        {!managerToolsInstalled && (activeView === "overview" || activeView === "manager" || activeViewRequiresManager) && (
          <ManagerToolsRequiredNotice
            busy={busy}
            canInstallManagerApi={canInstallManagerApi}
            onInstall={installManagerApi}
          />
        )}

        {directorLoading && (activeView === "overview" || activeView === "manager" || activeViewRequiresDirector) && (
          <DirectorStartingNotice />
        )}

        {managerToolsInstalled && !directorAvailable && !directorLoading && (activeView === "overview" || activeView === "manager") && (
          <DirectorUnavailableNotice busy={busy} onRefresh={refresh} />
        )}

        {directorAvailable && (activeView === "overview" || activeView === "players") && (
          <PlayersPanel players={directorPlayers} playerLists={directorPlayerLists} busy={busy} onReload={loadDirectorData} />
        )}

        {managerToolsInstalled && (activeView === "overview" || activeView === "battlegroups") && (
          <BattleGroupsPanel
            battleGroups={battleGroups}
            selectedBattleGroup={selectedBattleGroup}
            battleGroupDetail={battleGroupDetail}
            lifecycle={battleGroupLifecycle}
            busy={busy}
            canUseManager={canUseManager}
            battleGroupIsStopped={battleGroupIsStopped}
            battleGroupIsRunning={battleGroupIsRunning}
            snapshotPath={snapshotPath}
            onStart={() => setBattleGroupRunning(true)}
            onStop={() => setBattleGroupRunning(false)}
            onRestart={restartBattleGroup}
            onExport={exportLiveConfig}
            onSelect={(group) => {
              setSelectedNamespace(group.namespace);
              void loadBattleGroupDetail(group);
              void loadWorkloads(group.namespace);
            }}
          />
        )}

        {managerToolsInstalled && activeView === "config" && (
          <ConfigView
            battleGroupDetail={battleGroupDetail}
            directorAvailable={directorAvailable}
            directorFlsConfig={directorFlsConfig}
            directorTransferConfig={directorTransferConfig}
            directorMaps={directorMaps}
            selectedDirectorMapSummary={selectedDirectorMapSummary}
            flsDraft={flsDraft}
            transferDraft={transferDraft}
            mapOverrideDraft={mapOverrideDraft}
            busy={busy}
            onFlsDraftChange={setFlsDraft}
            onTransferDraftChange={setTransferDraft}
            onMapOverrideDraftChange={setMapOverrideDraft}
            onSaveFlsConfig={saveFlsConfig}
            onClearFlsConfig={clearFlsConfig}
            onSaveTransferConfig={saveTransferConfig}
            onClearTransferConfig={clearTransferConfig}
            onSelectMap={setSelectedDirectorMap}
            onSaveMapOverride={saveMapOverride}
            onClearMapOverride={clearMapOverride}
            flsPreview={flsPreview}
            transferPreview={transferPreview}
            flsChanged={flsChanged}
            transferChanged={transferChanged}
            mapOverridePreview={mapOverridePreview}
            onBackupConfig={backupConfig}
          />
        )}

        {directorAvailable && activeView === "director" && (
          <DirectorView
            directorMaps={directorMaps}
            busy={busy}
            onReload={loadDirectorData}
            onEditMap={(mapName) => {
              setSelectedDirectorMap(mapName);
              setActiveView("config");
            }}
            onClearMapOverride={clearMapOverride}
          />
        )}

        {managerToolsInstalled && (activeView === "overview" || activeView === "workloads") && (
          <WorkloadsPanel pods={pods} services={services} />
        )}

        {managerToolsInstalled && activeView === "logs" && (
          <LogsPanel pods={pods} busy={busy} onLoadLogs={loadLogs} />
        )}
      </section>
    </main>
  );
}
