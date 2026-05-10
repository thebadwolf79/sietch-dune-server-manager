import {
  CheckCircle2,
  CircleDashed,
  Download,
  FolderOpen,
  HardDrive,
  Network,
  PackageCheck,
  RadioTower,
  RefreshCw,
  Server,
  TerminalSquare,
  Wrench,
  type LucideIcon
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState } from "react";
import type { ChangeEvent, ReactNode } from "react";
import { LogOutput } from "../components/logOutput";
import { EmptyState, InfoRow, StatusPill } from "../components/primitives";
import type {
  AppConfig,
  GuestBootstrapRequest,
  SetupCommandResult,
  SetupState,
  VmDestinationStatus,
  VmImportOptions
} from "../types";

const WORLD_REGIONS = ["Europe Test", "North America Test"];

type SetupViewProps = {
  config: AppConfig;
  setupState: SetupState | null;
  vmImportOptions: VmImportOptions | null;
  setupLog: SetupCommandResult | null;
  busy: boolean;
  setupOperation: string;
  canInstallManagerApi: boolean;
  onRefreshSetup: () => void;
  onInstallSteamCmd: (installDir: string) => void;
  onInstallServerApp: (steamcmdPath: string, installDir: string) => void;
  onDetectVmOptions: (installPath: string) => void;
  onImportVm: (
    installPath: string,
    destinationPath: string,
    memoryGb: number,
    switchName: string,
    physicalAdapterName: string,
    clearDestination: boolean
  ) => void;
  onBootstrapGuest: (request: GuestBootstrapRequest) => void;
  onInstallManagerApi: () => void;
};

type StageCardProps = {
  title: string;
  subtitle: string;
  completed: boolean;
  icon: LucideIcon;
  children: ReactNode;
};

function StageCard({ title, subtitle, completed, icon: Icon, children }: StageCardProps) {
  return (
    <section className={`panel setup-stage ${completed ? "completed" : ""}`}>
      <div className="panel-title">
        <div className="setup-stage-title">
          <Icon size={20} />
          <div>
            <h2>{title}</h2>
            <p>{subtitle}</p>
          </div>
        </div>
        <StatusPill value={completed ? "Ready" : "Pending"} />
      </div>
      {children}
    </section>
  );
}

function useField(initial = "") {
  const [value, setValue] = useState(initial);
  return {
    value,
    setValue,
    bind: {
      value,
      onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => setValue(event.target.value)
    }
  };
}

export function SetupView({
  config,
  setupState,
  vmImportOptions,
  setupLog,
  busy,
  setupOperation,
  canInstallManagerApi,
  onRefreshSetup,
  onInstallSteamCmd,
  onInstallServerApp,
  onDetectVmOptions,
  onImportVm,
  onBootstrapGuest,
  onInstallManagerApi
}: SetupViewProps) {
  const vmIp = useField("");
  const playerIp = useField("");
  const staticIp = useField("");
  const staticCidr = useField("");
  const staticGateway = useField("");
  const staticDns = useField("");
  const worldName = useField("");
  const region = useField(WORLD_REGIONS[0]);
  const selfHostToken = useField("");
  const memoryGb = useField("20");
  const vmDestination = useField("");
  const vmIpMode = useField("");
  const networkAdapter = useField("");
  const [advanced, setAdvanced] = useState(false);

  const completed = useMemo(() => new Set(setupState?.persisted.completedStages ?? []), [setupState]);
  const detectedSteamCmd = setupState?.steamcmd.path || config.steamcmdPath;
  const detectedInstallPath = setupState?.serverInstallPath || config.installPath;
  const detectedVmIp = setupState?.vmIp || config.vmIp;
  const serverInstallPath = detectedInstallPath || setupState?.suggestedServerInstallDir || "dune-server";
  const selectedServerPath = serverInstallPath;
  const defaultVmDestination = vmImportOptions?.suggestedDestination || "vm";
  const selectedVmDestination = vmDestination.value.trim() || defaultVmDestination;
  const selectedSteamCmd = detectedSteamCmd;
  const selectedAdapter = vmImportOptions?.networkAdapters.find((adapter) => adapter.name === networkAdapter.value);
  const preferredAdapter =
    vmImportOptions?.networkAdapters.find((adapter) => adapter.boundSwitchName === "DuneAwakeningServerSwitch") ??
    vmImportOptions?.networkAdapters.find((adapter) => adapter.boundSwitchName) ??
    vmImportOptions?.networkAdapters[0];
  const steamCmdInstallDir = setupState?.suggestedSteamcmdInstallDir || "steamcmd";
  const steamCmdStatus = selectedSteamCmd ? "SteamCMD installed" : "SteamCMD not installed";
  const serverPackageStatus = detectedInstallPath || completed.has("server-app")
    ? "Server package installed"
    : "Server package not installed";
  const detectedVmState = setupState?.vmState?.toLowerCase() ?? "";
  const vmDetectedButStopped = Boolean(setupState?.vmExists && detectedVmState && detectedVmState !== "running");
  const missingBootstrapFields = [
    !selectedServerPath.trim() && "server package path",
    vmDetectedButStopped && "running VM",
    !vmIp.value.trim() && "VM IP",
    !playerIp.value.trim() && "player-facing IP",
    !worldName.value.trim() && "world name",
    !region.value.trim() && "region",
    !selfHostToken.value.trim() && "self-host token"
  ].filter(Boolean);

  const setupLogLines = useMemo(
    () => (setupLog?.stdout ? setupLog.stdout.split(/\r?\n/).filter((line) => line.length > 0) : []),
    [setupLog?.stdout]
  );
  const canBootstrap =
    selectedServerPath.trim().length > 0 &&
    !vmDetectedButStopped &&
    vmIp.value.trim().length > 0 &&
    playerIp.value.trim().length > 0 &&
    worldName.value.trim().length > 0 &&
    region.value.trim().length > 0 &&
    selfHostToken.value.trim().length > 0;

  async function installServerWithDialog() {
    onInstallServerApp(selectedSteamCmd, selectedServerPath);
  }

  async function chooseVmDestination() {
    const selected = await open({ directory: true, multiple: false, defaultPath: selectedVmDestination });
    if (typeof selected === "string") {
      vmDestination.setValue(selected);
    }
  }

  useEffect(() => {
    if (!networkAdapter.value && preferredAdapter?.name) {
      networkAdapter.setValue(preferredAdapter.name);
    }
  }, [networkAdapter, preferredAdapter]);

  useEffect(() => {
    const selections = setupState?.persisted.selections;
    if (!vmIp.value && detectedVmIp) {
      vmIp.setValue(detectedVmIp);
    }
    if (!playerIp.value) {
      playerIp.setValue(selections?.manualPlayerIp || detectedVmIp || "");
    }
    if (!worldName.value && selections?.worldName) {
      worldName.setValue(selections.worldName);
    }
    if (selections?.worldRegion && region.value === WORLD_REGIONS[0]) {
      region.setValue(selections.worldRegion);
    }
    if (!staticIp.value && selections?.staticIp) {
      staticIp.setValue(selections.staticIp);
    }
    if (!staticCidr.value && selections?.staticCidr) {
      staticCidr.setValue(selections.staticCidr);
    }
    if (!staticGateway.value && selections?.staticGateway) {
      staticGateway.setValue(selections.staticGateway);
    }
    if (!staticDns.value && selections?.staticDns) {
      staticDns.setValue(selections.staticDns);
    }
  }, [detectedVmIp, playerIp, region, setupState?.persisted.selections, staticCidr, staticDns, staticGateway, staticIp, vmIp, worldName]);

  async function createVm() {
    if (!vmImportOptions) {
      onDetectVmOptions(selectedServerPath);
      return;
    }
    if (!selectedAdapter) return;
    const destinationStatus = await invoke<VmDestinationStatus>("inspect_vm_destination", {
      destinationPath: selectedVmDestination
    });
    let clearDestination = false;
    if (destinationStatus.exists && !destinationStatus.isEmpty) {
      clearDestination = await ask(
        `The VM files destination already exists and is not empty:\n\n${selectedVmDestination}\n\nAll files and folders inside it will be deleted before importing the VM. Continue?`,
        {
          title: "Clear VM destination?",
          kind: "warning",
          okLabel: "Delete and continue",
          cancelLabel: "Cancel"
        }
      );
      if (!clearDestination) return;
    }
    const switchName =
      vmImportOptions.switches.find(
        (item) =>
          item.switchType === "External" &&
          item.netAdapterInterfaceDescription === selectedAdapter.interfaceDescription
      )?.name ?? "DuneAwakeningServerSwitch";
    const adapterName = selectedAdapter.name;
    onImportVm(
      selectedServerPath,
      selectedVmDestination,
      Number(memoryGb.value),
      switchName,
      adapterName,
      clearDestination
    );
  }

  const canCreateVm =
    Boolean(vmImportOptions) &&
    selectedServerPath.trim().length > 0 &&
    selectedVmDestination.trim().length > 0 &&
    vmIpMode.value.trim().length > 0 &&
    Boolean(selectedAdapter) &&
    !setupState?.vmExists;

  return (
    <div className="setup-workspace">
      <section className="panel setup-hero">
        <div>
          <div className="setup-kicker">
            <Wrench size={18} />
            End-to-end setup
          </div>
          <h2>Dune Dedicated Server Manager</h2>
          <p>
            Install the Steam app, reuse or import the Hyper-V VM, bootstrap the guest, then hand control to the
            Manager API.
          </p>
        </div>
        <div className="button-row">
          <button onClick={onRefreshSetup} disabled={busy}>
            <RefreshCw size={16} />
            Refresh
          </button>
          <button type="button" onClick={() => setAdvanced((value) => !value)}>
            <Wrench size={16} />
            {advanced ? "Basic" : "Advanced"}
          </button>
        </div>
      </section>

      <div className="setup-console">
        <div className="setup-steps">
        <StageCard
          title="Host Readiness"
          subtitle="Elevation, Hyper-V, and vmms status"
          completed={Boolean(setupState?.elevated && setupState.hypervAvailable && setupState.vmmsRunning)}
          icon={HardDrive}
        >
          <div className="config-summary">
            <InfoRow label="Elevated" value={setupState?.elevated ? "Yes" : "No"} />
            <InfoRow label="Hyper-V" value={setupState?.hypervAvailable ? "Available" : "Unavailable"} />
            <InfoRow label="vmms service" value={setupState?.vmmsRunning ? "Running" : "Stopped"} />
            <InfoRow label="Existing VM" value={setupState?.vmExists ? `${setupState.vmState || "Detected"}` : "None"} />
          </div>
        </StageCard>

        <StageCard
          title="SteamCMD"
          subtitle="Install Valve's package tool into the app folder"
          completed={Boolean(setupState?.steamcmd.found || config.steamcmdPath)}
          icon={Download}
        >
          <div className="setup-choice single">
            <div>
              <strong>Install destination</strong>
              <p className="mono">{steamCmdInstallDir}</p>
            </div>
            <button onClick={() => onInstallSteamCmd(steamCmdInstallDir)} disabled={busy}>
              <Download size={16} />
              {setupOperation === "Installing SteamCMD" ? "Installing..." : "Install SteamCMD"}
            </button>
          </div>
          <div className={`setup-detected ${selectedSteamCmd ? "ready" : "missing"}`}>
            <div>
              <strong>{steamCmdStatus}</strong>
              <span className="mono">{selectedSteamCmd || "steamcmd.exe will appear here after installation."}</span>
            </div>
          </div>
        </StageCard>

        <StageCard
          title="Server Package"
          subtitle="app_update 3104830 validate"
          completed={Boolean(setupState?.serverInstalled || completed.has("server-app"))}
          icon={PackageCheck}
        >
          <div className="setup-choice single">
            <div>
              <strong>Install destination</strong>
              <p className="mono">{serverInstallPath}</p>
            </div>
            <button className="primary" onClick={() => void installServerWithDialog()} disabled={busy || !selectedSteamCmd}>
              <PackageCheck size={16} />
              {setupOperation === "Installing server package" ? "Installing..." : "Install / Update"}
            </button>
          </div>
          <div className={`setup-detected ${detectedInstallPath || completed.has("server-app") ? "ready" : "missing"}`}>
            <div>
              <strong>{serverPackageStatus}</strong>
              <span className="mono">{detectedInstallPath || "The server package will appear here after installation."}</span>
            </div>
          </div>
        </StageCard>

        <StageCard
          title="Virtual Machine"
          subtitle="Create the packaged Hyper-V VM"
          completed={Boolean(setupState?.vmExists)}
          icon={Server}
        >
          <div className="setup-choice single">
            <div>
              <strong>{setupState?.vmExists ? "VM detected" : "No VM detected"}</strong>
              <p className="mono">
                {setupState?.vmExists
                  ? `${setupState.vmState || "Detected"}${setupState.vmIp ? ` at ${setupState.vmIp}` : ""}`
                  : "Choose networking before creating and starting the VM."}
              </p>
            </div>
            <button
              className="primary"
              onClick={() => void createVm()}
              disabled={busy || !selectedServerPath || Boolean(setupState?.vmExists) || (Boolean(vmImportOptions) && !canCreateVm)}
            >
              <Server size={16} />
              {!vmImportOptions ? "Detect VM Package" : setupOperation === "Importing VM" ? "Creating..." : "Create VM"}
            </button>
          </div>
          <div className="form-grid">
            <label>
              VM files destination
              <div className="path-picker">
                <input value={selectedVmDestination} onChange={(event) => vmDestination.setValue(event.target.value)} />
                <button type="button" onClick={() => void chooseVmDestination()} disabled={busy}>
                  <FolderOpen size={16} />
                </button>
              </div>
            </label>
            <label>
              VM memory
              <select {...memoryGb.bind}>
                <option value="20">20 GB - Sietch</option>
                <option value="30">30 GB - Sietch + Story/Social</option>
                <option value="40">40 GB - Deep Desert ready</option>
              </select>
            </label>
            <label>
              Initial network mode
              <select {...vmIpMode.bind}>
                <option value="">Choose before creating</option>
                <option value="dhcp">DHCP from router</option>
                <option value="static">Static IP after first boot</option>
              </select>
            </label>
            <label>
              Host network adapter
              <select {...networkAdapter.bind} disabled={!vmImportOptions?.networkAdapters.length}>
                <option value="">Choose adapter</option>
                {vmImportOptions?.networkAdapters.map((adapter) => (
                  <option key={adapter.name} value={adapter.name}>
                    {adapter.name} {adapter.cidr ? `- ${adapter.cidr}` : ""}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {vmImportOptions && (
            <div className="setup-adapter-list">
              {vmImportOptions.networkAdapters.length === 0 && (
                <p>No supported active physical IPv4 adapters were detected for a Hyper-V external switch.</p>
              )}
              {vmImportOptions.networkAdapters.map((adapter) => (
                <button
                  type="button"
                  key={adapter.name}
                  className={adapter.name === selectedAdapter?.name ? "selected" : ""}
                  onClick={() => networkAdapter.setValue(adapter.name)}
                  disabled={busy}
                >
                  <strong>{adapter.name}</strong>
                  <span>
                    {adapter.interfaceDescription}
                    {adapter.boundSwitchName ? ` via existing switch ${adapter.boundSwitchName}` : ""}
                  </span>
                  <span className="mono">
                    {adapter.cidr || "No IPv4 range"} {adapter.gateway ? `gateway ${adapter.gateway}` : ""}
                  </span>
                </button>
              ))}
            </div>
          )}
          {vmIpMode.value === "dhcp" && (
            <p className="setup-hint">
              The VM will start with DHCP. After it appears, use the detected IP for bootstrap.
            </p>
          )}
          {vmIpMode.value === "static" && (
            <p className="setup-hint">
              The VM still boots once with DHCP so SSH can connect, then Guest Bootstrap applies the static IP fields.
            </p>
          )}
          {!setupState?.elevated && <p className="setup-hint">Creating the VM requires running the app elevated.</p>}
        </StageCard>

        <StageCard
          title="Guest Bootstrap"
          subtitle="Use the VM DHCP IP, or optionally switch it to static"
          completed={completed.has("guest-bootstrap")}
          icon={Network}
        >
          <div className="form-grid">
            <label>
              VM IP
              <input placeholder="Detected or manual guest IP" {...vmIp.bind} />
            </label>
            <label>
              Player-facing IP
              <input placeholder="Public or LAN IP players use" {...playerIp.bind} />
            </label>
          </div>
          {detectedVmIp && (
            <div className="setup-detected">
              <span className="mono">{detectedVmIp}</span>
              <button
                type="button"
                onClick={() => {
                  vmIp.setValue(detectedVmIp);
                  if (!playerIp.value.trim()) {
                    playerIp.setValue(detectedVmIp);
                  }
                }}
              >
                Use for VM and LAN players
              </button>
            </div>
          )}
          {advanced && (
            <div className="form-grid">
              <label>
                Static VM IP
                <input placeholder="Leave blank for DHCP" {...staticIp.bind} />
              </label>
              <label>
                CIDR
                <input placeholder="/24" {...staticCidr.bind} />
              </label>
              <label>
                Gateway
                <input placeholder="Static gateway" {...staticGateway.bind} />
              </label>
              <label>
                DNS
                <input placeholder="1.1.1.1" {...staticDns.bind} />
              </label>
            </div>
          )}
          <div className="form-grid">
            <label>
              World name
              <input placeholder="Name shown for your self-hosted world" maxLength={50} {...worldName.bind} />
            </label>
            <label>
              Region
              <select {...region.bind}>
                {WORLD_REGIONS.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Self-host token
              <input type="password" placeholder="One-time setup token" {...selfHostToken.bind} />
            </label>
            <label>
              Bootstrap profile
              <select value="vendor-default" disabled>
                <option value="vendor-default">Vendor default</option>
              </select>
            </label>
          </div>
          <button
            className="primary"
            onClick={() =>
              onBootstrapGuest({
                installPath: selectedServerPath,
                ip: vmIp.value,
                playerIp: playerIp.value,
                staticIp: staticIp.value,
                staticCidr: staticCidr.value,
                staticGateway: staticGateway.value,
                staticDns: staticDns.value,
                worldName: worldName.value,
                region: region.value,
                selfHostToken: selfHostToken.value,
                profileId: "vendor-default"
              })
            }
            disabled={busy || !canBootstrap}
          >
            <TerminalSquare size={16} />
            {setupOperation === "Bootstrapping guest VM" ? "Bootstrapping..." : "Run Guest Bootstrap"}
          </button>
          {missingBootstrapFields.length > 0 && (
            <p className="setup-hint">
              Required before bootstrap: {missingBootstrapFields.join(", ")}. Player-facing IP can be the LAN VM IP for
              local testing; public IP is only needed for internet players.
            </p>
          )}
        </StageCard>

        <StageCard
          title="Manager API"
          subtitle="Install the authenticated control plane"
          completed={Boolean(config.managerApiUrl)}
          icon={RadioTower}
        >
          <div className="config-summary">
            <InfoRow label="API URL" value={config.managerApiUrl || "Not configured"} />
            <InfoRow label="Install ready" value={canInstallManagerApi ? "Yes" : "No"} />
          </div>
          <button className="primary" onClick={onInstallManagerApi} disabled={busy || !canInstallManagerApi}>
            <RadioTower size={16} />
            {setupOperation === "Installing Manager API" ? "Installing..." : "Install Tool"}
          </button>
        </StageCard>
        </div>

        <aside className="setup-side">
          <section className={`panel setup-progress ${setupOperation ? "active" : ""}`} role="status" aria-live="polite">
            <CircleDashed size={20} />
            <div>
              <strong>{setupOperation || "Setup idle"}</strong>
              <span>
                {setupOperation
                  ? "Keep this window open while the setup command runs."
                  : "Run a setup action to see progress here."}
              </span>
            </div>
          </section>

          <section className="panel setup-output">
            <div className="panel-title">
              <div className="setup-stage-title">
                {setupLog?.ok ? <CheckCircle2 size={20} /> : <CircleDashed size={20} />}
                <h2>Setup Output</h2>
              </div>
              {setupLog && <StatusPill value={setupLog.ok ? "OK" : "Pending"} />}
            </div>
            {!setupLog && <EmptyState text="Run a setup action to see its output here." />}
            {setupLog && <LogOutput lines={[setupLog.message, ...setupLogLines]} emptyText="Setup output is empty." />}
          </section>
        </aside>
      </div>
    </div>
  );
}
