import {
  Activity,
  Database,
  HardDrive,
  Map,
  RadioTower,
  Server,
  SlidersHorizontal,
  Terminal,
  Users,
  Wrench
} from "lucide-react";
import { useMemo } from "react";
import type {
  AppConfig,
  BattleGroupSummary,
  DirectorMapSummary,
  GuestConnection,
  HostStatus,
  ManagerApiStatus,
  NavItem,
  ViewKey,
  VmStatus
} from "../types";

type ManagerSocketState = "disabled" | "connecting" | "connected" | "error";

type DashboardDerivedInput = {
  config: AppConfig;
  host: HostStatus | null;
  vm: VmStatus | null;
  guest: GuestConnection | null;
  battleGroups: BattleGroupSummary[];
  selectedNamespace: string;
  directorMaps: DirectorMapSummary[];
  selectedDirectorMap: string;
  busy: boolean;
  managerStatus: ManagerApiStatus | null;
  managerSocketState: ManagerSocketState;
  activeView: ViewKey;
};

export function useDashboardDerivedState({
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
}: DashboardDerivedInput) {
  const selectedBattleGroup = useMemo(
    () => battleGroups.find((group) => group.namespace === selectedNamespace) ?? battleGroups[0],
    [battleGroups, selectedNamespace]
  );
  const selectedDirectorMapSummary =
    directorMaps.find((map) => map.name === selectedDirectorMap) ?? directorMaps[0] ?? null;
  const vmState = vm?.state.toLowerCase() ?? "";
  const vmExists = Boolean(vm && !["missing", "not found"].includes(vmState));
  const vmIsRunning = vmState === "running";
  const vmIsStarting = vmState === "starting";
  const vmIsChanging = ["starting", "stopping", "pausing", "resuming", "resetting", "saving"].includes(vmState);
  const canControlVm = Boolean(host?.isElevated && host?.hypervAvailable && vmExists);
  const startVmDisabledReason = busy
    ? "An operation is already running"
    : !host?.isElevated
      ? "VM controls require the app to run elevated"
      : !host?.hypervAvailable
        ? "Hyper-V is unavailable"
          : !vmExists
            ? "VM was not detected"
          : vmIsRunning
            ? "VM is already running"
            : vmIsChanging
              ? "VM is changing state"
              : "Start VM";
  const stopVmDisabledReason = busy
    ? "An operation is already running"
    : !host?.isElevated
      ? "VM controls require the app to run elevated"
      : !host?.hypervAvailable
        ? "Hyper-V is unavailable"
        : !vmExists
          ? "VM was not detected"
          : !vmIsRunning
            ? "VM is not running"
            : vmIsChanging
              ? "VM is changing state"
              : "Stop VM";
  const battleGroupIsStopped =
    selectedBattleGroup?.stop === true || selectedBattleGroup?.phase.toLowerCase() === "stopped";
  const battleGroupIsRunning =
    selectedBattleGroup?.stop === false &&
    ["running", "ready", "starting", "healthy"].includes(selectedBattleGroup?.phase.toLowerCase() ?? "");
  const canUseGuest = Boolean(vmIsRunning && guest?.connected && guest?.sudo && guest?.kubectl);
  const canReachGuestForInstall = Boolean((guest?.connected && guest?.sudo) || config.vmIp.trim());
  const managerApiConfigured = config.managerApiUrl.trim().length > 0;
  const managerReadiness = managerStatus ? "Ready" : managerApiConfigured ? "Offline" : "Disabled";
  const managerTelemetryState = managerApiConfigured ? managerSocketState : "disabled";
  const canUseManager = managerApiConfigured && Boolean(managerStatus);
  const managerToolsInstalled = canUseManager;
  const directorAvailable = Boolean(managerToolsInstalled && managerStatus?.directorConfigured);
  const managerInstallNamespace = config.managerApiNamespace.trim() || selectedBattleGroup?.namespace || "";
  const canInstallManagerApi = Boolean(canReachGuestForInstall && config.managerApiBinaryPath.trim());
  const managerRequiredViews = ["battlegroups", "workloads", "config", "logs", "players", "director"];
  const directorRequiredViews = ["players", "director"];
  const activeViewRequiresManager = managerRequiredViews.includes(activeView);
  const activeViewRequiresDirector = directorRequiredViews.includes(activeView);
  const viewLabels: Record<ViewKey, string> = {
    overview: "Overview",
    setup: "Setup",
    host: "Host & VM",
    manager: "Manager API",
    players: "Players",
    battlegroups: "BattleGroups",
    workloads: "Pods & Services",
    director: "Director",
    config: "Config",
    logs: "Logs"
  };
  const pageTitle = activeView === "overview" ? selectedBattleGroup?.title || "Dune Awakening" : viewLabels[activeView];
  const pageSubtitle =
    activeView === "overview"
      ? selectedBattleGroup?.name || "No battlegroup detected"
      : selectedBattleGroup?.title || selectedBattleGroup?.name || "No battlegroup selected";
  const navItems: NavItem[] = [
    { key: "overview", label: "Overview", icon: Server },
    { key: "setup", label: "Setup", icon: Wrench },
    { key: "host", label: "Host & VM", icon: HardDrive },
    { key: "manager", label: "Manager API", icon: RadioTower },
    { key: "players", label: "Players", icon: Users, disabled: !directorAvailable },
    { key: "battlegroups", label: "BattleGroups", icon: Activity, disabled: !managerToolsInstalled },
    { key: "workloads", label: "Pods & Services", icon: Database, disabled: !managerToolsInstalled },
    { key: "director", label: "Director", icon: Map, disabled: !directorAvailable },
    { key: "config", label: "Config", icon: SlidersHorizontal, disabled: !managerToolsInstalled },
    { key: "logs", label: "Logs", icon: Terminal, disabled: !managerToolsInstalled }
  ];

  return {
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
  };
}
