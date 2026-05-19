import {
  Component,
  type ComponentType,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
  AlertDialog,
  Box,
  Button,
  Card,
  Dialog,
  Flex,
  Grid,
  Heading,
  Separator,
  Select,
  Text,
  TextArea,
  TextField,
  Theme,
} from "@radix-ui/themes";
import { ChevronDownIcon, ChevronUpIcon } from "@radix-ui/react-icons";

// Imports from decoupled modules
import {
  type PageId,
  type SetupForm,
  type LogRow,
  type LogLevel,
  type LogLevelFilter,
  type UpdateStatus,
  type ServerPackageCheckStatus,
  type ServerPackageStatus,
  type DuneVmCandidate,
  type LocalHyperVRuntime,
  type RemoteServerRecord,
  type RemoteServerStatus,
  type RemoteServerComponent,
  type ProxmoxVmStatus,
  type ServerTunnelStatus,
  type ServerTunnelStartRequest,
  type HostReadiness,
  type DriveCandidate,
  type NetworkAdapterCandidate,
  type DetectionState,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type RollbackRequest,
  type GenerateSshKeyResult,
  type RemoteAttachForm,
  type LocalHyperVAttachForm,
  type PendingServerUpdate,
  type RemoteServerKind,
  type EnvironmentGate,
  type EnvironmentDetection,
  type RemoteComponentLogResult,
  type AppLogEntry
} from "./types";

import {
  errorMessage,
  sanitizeLogMessage,
  formatGiB,
  formatBytes,
  limitLogRows,
  filterLogRows,
  logEntry
} from "./utils/helpers";

import {
  localServerKey,
  remoteServerDefaultUser,
  remoteServerKindLabel,
  serverTunnelKey,
  componentLogStateKey,
  isCriticalRestartComponent,
  primaryLocalServerIp,
  copyTextToClipboard,
  tunnelServiceLabel,
  readLocalServers,
  readRemoteServers,
  persistLocalServers,
  persistRemoteServers,
  upsertLocalServer,
  upsertRemoteServer,
  remoteServerPlaceholder,
  remoteServerFromDetected,
  remoteServerDraftFromForm,
  proxmoxProvisionerFromForm,
  proxmoxServerDraftFromForm,
  proxmoxServerRecordFromSetup,
  remoteServerRecordFromSetup,
  remoteServerActionRequest,
  proxmoxConnectionRequest,
  proxmoxVmActionRequest,
  proxmoxSetupRunRequest,
  remoteSetupRunRequest,
  setupRunRequest,
  setupLayoutPreview,
  rollbackRequestFromSetup,
  persistProxmoxProfile,
  localServerPlaceholder,
  mergeLocalServerAddress,
  omitKey,
  omitPrefix,
  uniqueBy
} from "./utils/storage";

import {
  calculateRequiredMemory,
  normalizeSetupForm,
  effectiveVmMemoryGb,
  effectiveProxmoxVmMemoryGb,
  effectiveProcessorCount
} from "./utils/memory";

import { setupEnvironmentGate } from "./utils/validation";

import { Header, AppErrorBoundary } from "./components/Header";
import { InstallControls } from "./components/InstallControls";
import { ToolsPage } from "./components/ToolsPage";
import { ServersPage } from "./components/ServersPage";
import { Metric, InfoRow } from "./components/Common";

const log = {
  debug: (scope: string, message: string): LogRow => logEntry("debug", scope, message),
  info: (scope: string, message: string): LogRow => logEntry("info", scope, message),
  warn: (scope: string, message: string): LogRow => logEntry("warn", scope, message),
  error: (scope: string, message: string): LogRow => logEntry("error", scope, message),
};

const startupUpdateChecksEnabled = import.meta.env.VITE_ENABLE_STARTUP_UPDATE_CHECK === "true";
const defaultHyperVVmName = "dune-awakening";
const defaultHyperVSwitchName = "DuneAwakeningServerSwitch";
const maxStoredLogRows = 2500;
const maxRenderedLogRows = 1200;

type ProxmoxSetupOverrides = {
  temporaryDhcpIp?: string;
  staticIp?: string;
  gateway?: string;
  dns?: string;
};

type ProxmoxNetworkProbeResult = {
  temporaryIp: string;
  interface?: string | null;
  addressCidr?: string | null;
  staticIp?: string | null;
  prefixLength?: number | null;
  gateway?: string | null;
  dns?: string | null;
};

type ProxmoxNetworkPromptValues = {
  temporaryDhcpIp: string;
  staticIp: string;
  gateway: string;
  dns: string;
};

const defaultForm: SetupForm = {
  setupTarget: "hyperv",
  vmDestination: "",
  vmName: defaultHyperVVmName,
  diskGb: "100",
  vmMemoryGb: "",
  processorCount: "4",
  enableSwap: false,
  networkMode: "static",
  switchName: defaultHyperVSwitchName,
  adapterName: "",
  staticIp: "",
  gateway: "",
  dns: "1.1.1.1",
  playerIpMode: "local",
  playerIp: "",
  worldName: "Arrakis",
  region: "Europe Test",
  tokenSource: "",
  survivalInstances: "1",
  includeSocial: true,
  deepDesertPveInstances: "0",
  deepDesertPvpInstances: "0",
  deepDesertWarmServers: "0",
  remoteHost: "",
  remoteUser: "root",
  remoteKeyPath: "",
  proxmoxHostUrl: "",
  proxmoxTokenId: "",
  proxmoxTokenSecret: "",
  proxmoxAcceptedCertificateSha256: "",
  proxmoxNode: "",
  proxmoxVmStorage: "",
  proxmoxImportStorage: "",
  proxmoxBridge: "",
  proxmoxBridgeCidr: "",
  proxmoxVmid: "",
  proxmoxTemporaryDhcpIp: "",
  proxmoxSshKeyPath: "",
  proxmoxInstallQemuGuestAgent: true,
  saveLocalServer: true,
  saveRemoteServer: true,
};

const defaultRemoteAttachForm: RemoteAttachForm = {
  type: "ubuntu",
  host: "",
  keyPath: "",
};

const defaultLocalHyperVAttachForm: LocalHyperVAttachForm = {
  vmName: defaultHyperVVmName,
  staticIp: "",
};

const remoteProfileStorageKey = "dune-manager.remote-ubuntu-profile";
const proxmoxProfileStorageKey = "dune-manager.proxmox-profile";
const remoteServersStorageKey = "dune-manager.remote-servers";

function environmentLogRows(
  status: DetectionState,
  readiness: HostReadiness | null,
  adapters: NetworkAdapterCandidate[],
  drives: DriveCandidate[],
  externalIp: string | null,
  gate: EnvironmentGate,
): LogRow[] {
  if (status === "detecting") {
    return [
      log.debug("env", "Checking administrator privileges..."),
      log.debug("env", "Checking virtualization firmware, Hyper-V, and vmms service..."),
      log.debug("env", "Waiting to detect host networking after host checks complete..."),
    ];
  }
  if (status === "failed") {
    return [log.error("env", "Environment detection failed. Network fields can still be filled manually.")];
  }
  if (status === "idle") {
    return [log.info("env", "Local environment detection has not run yet.")];
  }
  const rows: LogRow[] = [];
  if (readiness) {
    rows.push(
      readiness.elevated
        ? log.info("env", "Administrator privileges detected.")
        : log.warn("env", "This app is not elevated; restart it as administrator to continue setup."),
    );
    rows.push(
      readiness.virtualizationFirmwareEnabled === false
        ? log.warn("env", "Virtualization firmware is disabled or unavailable.")
        : log.info("env", "Hyper-V virtualization support is operational."),
    );
    rows.push(
      readiness.hypervAvailable && readiness.vmmsRunning
        ? log.info("env", "Hyper-V available; vmms service running.")
        : log.warn(
            "env",
            `Hyper-V ${readiness.hypervAvailable ? "available" : "missing"}; vmms service ${
              readiness.vmmsRunning ? "running" : "not running"
            }.`,
          ),
    );
    rows.push(
      log.info(
        "env",
        `Physical memory: ${formatGiB(readiness.availablePhysicalMemoryBytes)} available of ${formatGiB(readiness.totalPhysicalMemoryBytes)} total.`,
      ),
    );
    rows.push(
      log.info(
        "env",
        `CPU cores: ${readiness.logicalProcessorCount || "unknown"} logical processors detected.`,
      ),
    );
  }
  if (drives.length > 0) {
    rows.push(
      log.debug(
        "env",
        `Detected drives: ${drives
          .map((drive) => `${drive.root} ${formatGiB(drive.freeBytes)} free`)
          .join(", ")}.`,
      ),
    );
  }
  rows.push(
    externalIp
      ? log.info("env", "Detected external IP.")
      : log.warn("env", "External IP was not detected; it can be entered manually."),
  );
  if (adapters.length === 0) {
    rows.push(log.warn("env", "No active physical adapters with IPv4 gateway were detected."));
    return rows;
  }

  rows.push(
    ...adapters.map((adapter) =>
      log.info(
        "env",
        `Detected ${adapter.name} with IPv4 gateway and VM IP suggestion.`,
      ),
    ),
  );
  if (!gate.canContinue) {
    rows.push(...gate.reasons.map((reason) => log.error("env", reason)));
  }
  return rows;
}

async function openFileDialog(title: string): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Box>
      <Text as="label" size="2" weight="medium" mb="1" className="field-label">
        {label}
      </Text>
      {children}
    </Box>
  );
}

function LogWindow({
  rows,
  level,
  collapsed,
  onLevelChange,
  onClear,
  onOpenLogFile,
  onToggleCollapsed,
}: {
  rows: LogRow[];
  level: LogLevelFilter;
  collapsed: boolean;
  onLevelChange: (level: LogLevelFilter) => void;
  onClear: () => void;
  onOpenLogFile: () => void;
  onToggleCollapsed: () => void;
}) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);

  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    if (stickToBottomRef.current) {
      body.scrollTop = body.scrollHeight;
    }
  }, [rows]);

  return (
    <Card size="3" variant="surface" className={`pane log-pane${collapsed ? " is-collapsed" : ""}`}>
      <Flex direction="column" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3" mb={collapsed ? "0" : "3"}>
          <Box minWidth="0">
            <Text as="div" size="2" weight="medium">
              Logs
            </Text>
            <Text as="div" size="1" color="gray">
              {rows.length} entries
            </Text>
          </Box>
          <Flex align="center" gap="2">
            {collapsed ? null : (
              <>
                <Select.Root value={level} onValueChange={(value) => onLevelChange(value as LogLevelFilter)}>
                  <Select.Trigger aria-label="Minimum log level" />
                  <Select.Content>
                    <Select.Item value="debug">Debug</Select.Item>
                    <Select.Item value="info">Info</Select.Item>
                    <Select.Item value="warn">Warn</Select.Item>
                    <Select.Item value="error">Error</Select.Item>
                  </Select.Content>
                </Select.Root>
                <Button type="button" size="1" variant="surface" disabled={rows.length === 0} onClick={onClear}>
                  Clear
                </Button>
                <Button type="button" size="1" variant="surface" onClick={onOpenLogFile}>
                  Open file
                </Button>
              </>
            )}
            <Button
              type="button"
              size="1"
              variant="surface"
              aria-label={collapsed ? "Expand logs" : "Collapse logs"}
              onClick={onToggleCollapsed}
            >
              {collapsed ? <ChevronUpIcon /> : <ChevronDownIcon />}
            </Button>
          </Flex>
        </Flex>
        {collapsed ? null : (
          <Box
            className="log-body"
            ref={bodyRef}
            onScroll={(event) => {
              const body = event.currentTarget;
              const distanceFromBottom = body.scrollHeight - body.scrollTop - body.clientHeight;
              stickToBottomRef.current = distanceFromBottom < 80;
            }}
          >
            <Flex direction="column" gap="0">
              {rows.map((row) => (
                <Grid
                  key={row.id}
                  columns="96px 44px 1fr"
                  gap="2"
                  align="center"
                  className={`log-line log-${row.level}`}
                >
                  <Text color="gray" className="mono log-meta log-text">
                    {row.timestamp}
                  </Text>
                  <Text className="mono log-meta log-level log-text">
                    {row.level}
                  </Text>
                  <Text className="mono log-text">
                    {row.message}
                  </Text>
                </Grid>
              ))}
            </Flex>
          </Box>
        )}
      </Flex>
    </Card>
  );
}

function RollbackDialog({
  open,
  rollbackRunning,
  onOpenChange,
  onRollback,
}: {
  open: boolean;
  rollbackRunning: boolean;
  onOpenChange: (open: boolean) => void;
  onRollback: () => void;
}) {
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="460px">
        <AlertDialog.Title>Rollback setup artifacts?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          Setup failed after creating or touching host resources. Rollback removes the selected VM,
          removes VM files when they look like manager-created VM files, and removes the Hyper-V
          switch only if no other VMs use it.
        </AlertDialog.Description>
        <Flex gap="3" mt="4" justify="end">
          <AlertDialog.Cancel disabled={rollbackRunning}>
            <Button variant="soft" color="gray" disabled={rollbackRunning}>
              Keep artifacts
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action disabled={rollbackRunning}>
            <Button color="red" disabled={rollbackRunning} onClick={onRollback}>
              {rollbackRunning ? "Rolling back..." : "Rollback"}
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

function ServerUpdateConfirmDialog({
  pending,
  onOpenChange,
  onConfirm,
}: {
  pending: PendingServerUpdate | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  const serverName =
    pending?.type === "remote"
      ? pending.server.name
      : pending?.type === "local"
        ? pending.server.vm.name
        : "";
  return (
    <AlertDialog.Root open={!!pending} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Update server?</AlertDialog.Title>
        <AlertDialog.Description size="2" color="gray">
          This operation will stop the BattleGroup, verify it is fully stopped, update the server,
          and start the BattleGroup again. Are you sure?
        </AlertDialog.Description>
        {serverName ? (
          <Text as="p" size="2" color="gray" mt="3">
            Target: <Text weight="medium">{serverName}</Text>
          </Text>
        ) : null}
        <Flex gap="3" justify="end" mt="5">
          <AlertDialog.Cancel>
            <Button variant="soft" color="gray">
              Cancel
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action>
            <Button color="amber" onClick={onConfirm}>
              Update Server
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

function RemoteAttachDialog({
  open,
  form,
  running,
  onOpenChange,
  onChange,
  onAttach,
}: {
  open: boolean;
  form: RemoteAttachForm;
  running: boolean;
  onOpenChange: (open: boolean) => void;
  onChange: (form: RemoteAttachForm) => void;
  onAttach: () => void;
}) {
  const canAttach =
    form.host.trim().length > 0 &&
    form.keyPath.trim().length > 0 &&
    !running;
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="520px">
        <Dialog.Title>Add Remote Server</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Connect over SSH and detect existing Dune battlegroups. This does not provision or modify the server.
        </Dialog.Description>
        <Flex direction="column" gap="3" mt="4">
          <Field label="Server type">
            <Select.Root
              value={form.type}
              onValueChange={(value) => onChange({ ...form, type: value as RemoteServerKind })}
              disabled={running}
            >
              <Select.Trigger />
              <Select.Content>
                <Select.Item value="ubuntu">Remote Ubuntu over SSH</Select.Item>
                <Select.Item value="alpine">Remote Alpine VM over SSH</Select.Item>
              </Select.Content>
            </Select.Root>
          </Field>
          <Field label="Host or IP">
            <TextField.Root
              placeholder="203.0.113.10"
              disabled={running}
              value={form.host}
              onChange={(event) => onChange({ ...form, host: event.target.value })}
            />
          </Field>
          <Field label="Private Key">
            <Grid columns="1fr auto" gap="2">
              <TextField.Root
                placeholder="Choose SSH private key"
                value={form.keyPath}
                disabled={running}
                onChange={(event) => onChange({ ...form, keyPath: event.target.value })}
              />
              <Button
                type="button"
                variant="surface"
                disabled={running}
                onClick={async () => {
                  const selected = await openFileDialog("Choose SSH private key");
                  if (selected) onChange({ ...form, keyPath: selected });
                }}
              >
                Choose
              </Button>
            </Grid>
          </Field>
        </Flex>
        <Flex gap="3" justify="end" mt="5">
          <Dialog.Close>
            <Button variant="soft" color="gray" disabled={running}>
              Cancel
            </Button>
          </Dialog.Close>
          <Button disabled={!canAttach} onClick={onAttach}>
            {running ? "Detecting..." : "Detect and Add"}
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function LocalHyperVAttachDialog({
  open,
  form,
  running,
  onOpenChange,
  onChange,
  onAttach,
}: {
  open: boolean;
  form: LocalHyperVAttachForm;
  running: boolean;
  onOpenChange: (open: boolean) => void;
  onChange: (form: LocalHyperVAttachForm) => void;
  onAttach: () => void;
}) {
  const canAttach = form.vmName.trim().length > 0 && !running;
  const update = <K extends keyof LocalHyperVAttachForm>(key: K, value: LocalHyperVAttachForm[K]) => {
    onChange({ ...form, [key]: value });
  };
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Add Local Hyper-V Server</Dialog.Title>
        <Dialog.Description size="2" color="gray">
          Detect the vendor Hyper-V VM and read its host and in-guest server details.
        </Dialog.Description>
        <Flex direction="column" gap="3" mt="4">
          <label>
            <Text as="div" size="2" weight="medium" mb="1">
              VM name
            </Text>
            <TextField.Root
              value={form.vmName}
              disabled={running}
              onChange={(event) => update("vmName", event.target.value)}
              placeholder={defaultHyperVVmName}
            />
          </label>
          <label>
            <Text as="div" size="2" weight="medium" mb="1">
              Static IP
            </Text>
            <TextField.Root
              value={form.staticIp}
              disabled={running}
              onChange={(event) => update("staticIp", event.target.value)}
              placeholder="Only needed if Hyper-V does not report the guest IP"
            />
          </label>
        </Flex>
        <Flex gap="3" justify="end" mt="5">
          <Dialog.Close>
            <Button variant="soft" color="gray" disabled={running}>
              Cancel
            </Button>
          </Dialog.Close>
          <Button disabled={!canAttach} onClick={onAttach}>
            {running ? "Registering..." : "Register Server"}
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function RemoveRemoteServerDialog({
  server,
  onOpenChange,
  onRemove,
}: {
  server: RemoteServerRecord | null;
  onOpenChange: (open: boolean) => void;
  onRemove: (server: RemoteServerRecord) => void;
}) {
  return (
    <AlertDialog.Root open={!!server} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Forget Remote Server</AlertDialog.Title>
        <AlertDialog.Description size="2" color="gray">
          This only removes the saved server entry from this app. The remote host and Dune battlegroup will not be changed.
        </AlertDialog.Description>
        {server ? (
          <Box className="info-card" mt="4">
            <InfoRow label="Host" value={server.host} tone="amber" />
            <InfoRow label="Battlegroup" value={server.battlegroupName || "Setup pending"} tone="amber" />
          </Box>
        ) : null}
        <Flex gap="3" justify="end" mt="5">
          <AlertDialog.Cancel>
            <Button variant="soft" color="gray">
              Cancel
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action>
            <Button color="red" onClick={() => server && onRemove(server)}>
              Forget Server
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

function ProxmoxIpPromptDialog({
  open,
  onOpenChange,
  currentTemporaryIp,
  currentStaticIp,
  currentGateway,
  currentDns,
  onCheck,
  onSubmit,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  currentTemporaryIp: string;
  currentStaticIp: string;
  currentGateway: string;
  currentDns: string;
  onCheck: (ip: string) => Promise<ProxmoxNetworkProbeResult>;
  onSubmit: (values: ProxmoxNetworkPromptValues) => void;
}) {
  const [ip, setIp] = useState(currentTemporaryIp);
  const [staticIp, setStaticIp] = useState(currentStaticIp);
  const [gateway, setGateway] = useState(currentGateway);
  const [dns, setDns] = useState(currentDns || "1.1.1.1");
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [probe, setProbe] = useState<ProxmoxNetworkProbeResult | null>(null);
  const [showNetworkControls, setShowNetworkControls] = useState(false);

  useEffect(() => {
    if (!open) return;
    setIp(currentTemporaryIp);
    setStaticIp(currentStaticIp);
    setGateway(currentGateway);
    setDns(currentDns || "1.1.1.1");
    setChecking(false);
    setCheckError(null);
    setProbe(null);
    setShowNetworkControls(false);
  }, [open, currentTemporaryIp, currentStaticIp, currentGateway, currentDns]);

  const cleanedIp = ip.trim().replace(/,/g, ".");
  const canCheck = isDialogIpv4(cleanedIp) && !checking;
  const canSubmit =
    showNetworkControls &&
    isDialogIpv4(cleanedIp) &&
    isDialogIpv4(staticIp.trim().replace(/,/g, ".")) &&
    isDialogIpv4(gateway.trim().replace(/,/g, ".")) &&
    !!dns.trim() &&
    !checking;

  const runCheck = async () => {
    const candidateIp = cleanedIp;
    if (!isDialogIpv4(candidateIp)) {
      setCheckError("Enter a valid temporary IPv4 address first.");
      setShowNetworkControls(false);
      return;
    }
    setChecking(true);
    setCheckError(null);
    setProbe(null);
    try {
      const result = await onCheck(candidateIp);
      setProbe(result);
      setIp(result.temporaryIp || candidateIp);
      setStaticIp((value) => value.trim() || result.staticIp || candidateIp);
      setGateway((value) => value.trim() || result.gateway || "");
      setDns((value) => value.trim() || result.dns || "1.1.1.1");
      setShowNetworkControls(true);
    } catch (err) {
      setCheckError(errorMessage(err));
      setShowNetworkControls(false);
    } finally {
      setChecking(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="560px">
        <Dialog.Title>Proxmox Temporary IP Required</Dialog.Title>
        <Dialog.Description size="2" mb="3">
          The VM shell was created and started on Proxmox, but setup needs the temporary DHCP address before it can bootstrap the guest over SSH.
          <br /><br />
          Open the <strong>Proxmox Web Console</strong> for this VM, copy the temporary DHCP IP address displayed on the boot screen or from your router leases, then check it here. Once SSH works, the static network fields appear with the guest route details pre-filled where available.
        </Dialog.Description>
        <Box mb="4">
          <Text as="div" size="2" weight="medium" mb="2">
            Temporary IP address (DHCP)
          </Text>
          <Grid columns="1fr auto" gap="2">
            <TextField.Root
              placeholder="e.g. 192.168.1.150"
              value={ip}
              onChange={(e) => {
                setIp(e.target.value);
                setCheckError(null);
                setProbe(null);
                setShowNetworkControls(false);
              }}
            />
            <Button variant="soft" disabled={!canCheck} onClick={() => void runCheck()}>
              {checking ? "Checking..." : "Check"}
            </Button>
          </Grid>
          {checkError ? (
            <Text as="p" size="2" color="red" mt="2">
              {checkError}
            </Text>
          ) : null}
          {probe ? (
            <Text as="p" size="2" color="green" mt="2">
              SSH connected{probe.addressCidr ? `, guest address ${probe.addressCidr}` : ""}{probe.gateway ? `, gateway ${probe.gateway}` : ""}.
            </Text>
          ) : null}
        </Box>
        {showNetworkControls ? (
          <Box className="info-card" mb="4">
            <Text as="div" size="2" weight="medium" mb="3">
              Static guest network
            </Text>
            <Grid columns="3" gap="3">
              <Box>
                <Text as="div" size="1" color="gray" mb="1">
                  Static IP
                </Text>
                <TextField.Root
                  placeholder="Guest static IP"
                  value={staticIp}
                  onChange={(event) => setStaticIp(event.target.value)}
                />
              </Box>
              <Box>
                <Text as="div" size="1" color="gray" mb="1">
                  Gateway
                </Text>
                <TextField.Root
                  placeholder="Gateway"
                  value={gateway}
                  onChange={(event) => setGateway(event.target.value)}
                />
              </Box>
              <Box>
                <Text as="div" size="1" color="gray" mb="1">
                  DNS
                </Text>
                <TextField.Root
                  placeholder="DNS"
                  value={dns}
                  onChange={(event) => setDns(event.target.value)}
                />
              </Box>
            </Grid>
          </Box>
        ) : null}
        <Flex gap="3" justify="end" mt="4">
          <Dialog.Close>
            <Button variant="soft" color="gray">
              Cancel
            </Button>
          </Dialog.Close>
          <Button
            disabled={!canSubmit}
            onClick={() => {
              onSubmit({
                temporaryDhcpIp: cleanedIp,
                staticIp: staticIp.trim().replace(/,/g, "."),
                gateway: gateway.trim().replace(/,/g, "."),
                dns: dns.trim(),
              });
              onOpenChange(false);
            }}
          >
            Resume Setup
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function isDialogIpv4(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 4 && parts.every((part) => {
    if (!/^\d{1,3}$/.test(part)) return false;
    const number = Number(part);
    return number >= 0 && number <= 255;
  });
}

function UpdateDialog({
  open,
  update,
  status,
  progress,
  onOpenChange,
  onInstall,
}: {
  open: boolean;
  update: Update | null;
  status: UpdateStatus;
  progress: string | null;
  onOpenChange: (open: boolean) => void;
  onInstall: () => void;
}) {
  const busy = status === "installing" || status === "relaunching";

  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Content maxWidth="520px">
        <AlertDialog.Title>Install app update?</AlertDialog.Title>
        <AlertDialog.Description size="2">
          {update
            ? `Version ${update.version} is available. The app will download the signed installer, install it, and relaunch.`
            : "No update is currently selected."}
        </AlertDialog.Description>
        {update?.body ? (
          <TextArea mt="3" value={update.body} readOnly rows={7} />
        ) : null}
        {progress ? (
          <Text as="p" size="2" color="gray" mt="3" className="mono">
            {progress}
          </Text>
        ) : null}
        <Flex gap="3" mt="4" justify="end">
          <AlertDialog.Cancel disabled={busy}>
            <Button variant="soft" color="gray" disabled={busy}>
              Later
            </Button>
          </AlertDialog.Cancel>
          <AlertDialog.Action disabled={!update || busy}>
            <Button disabled={!update || busy} onClick={onInstall}>
              {busy ? "Installing..." : "Install update"}
            </Button>
          </AlertDialog.Action>
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}

export function App() {
  const [activePage, setActivePage] = useState<PageId>("servers");
  const [form, setForm] = useState<SetupForm>(defaultForm);
  const [started, setStarted] = useState(false);
  const [setupRunning, setSetupRunning] = useState(false);
  const [setupRows, setSetupRows] = useState<LogRow[]>([]);
  const [initRows, setInitRows] = useState<LogRow[]>([]);
  const [logLevelFilter, setLogLevelFilter] = useState<LogLevelFilter>("info");
  const [logPanelCollapsed, setLogPanelCollapsed] = useState(false);
  const [rollbackOpen, setRollbackOpen] = useState(false);
  const [rollbackRunning, setRollbackRunning] = useState(false);
  const [proxmoxIpPromptOpen, setProxmoxIpPromptOpen] = useState(false);
  const [failedRollbackRequest, setFailedRollbackRequest] = useState<RollbackRequest | null>(null);
  const [pendingServerUpdate, setPendingServerUpdate] = useState<PendingServerUpdate | null>(null);
  const [localAttachOpen, setLocalAttachOpen] = useState(false);
  const [localAttachRunning, setLocalAttachRunning] = useState(false);
  const [localAttachForm, setLocalAttachForm] = useState<LocalHyperVAttachForm>(defaultLocalHyperVAttachForm);
  const [remoteAttachOpen, setRemoteAttachOpen] = useState(false);
  const [remoteAttachRunning, setRemoteAttachRunning] = useState(false);
  const [remoteAttachForm, setRemoteAttachForm] = useState<RemoteAttachForm>(defaultRemoteAttachForm);
  const [remoteServerToRemove, setRemoteServerToRemove] = useState<RemoteServerRecord | null>(null);
  const [generatedSshKey, setGeneratedSshKey] = useState<GenerateSshKeyResult | null>(null);
  const [sshKeyGenerationRunning, setSshKeyGenerationRunning] = useState(false);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<string | null>(null);
  const [serverPackageStatus, setServerPackageStatus] = useState<ServerPackageStatus | null>(null);
  const [serverPackageCheckStatus, setServerPackageCheckStatus] = useState<ServerPackageCheckStatus>("idle");
  const [hostReadiness, setHostReadiness] = useState<HostReadiness | null>(null);
  const [driveCandidates, setDriveCandidates] = useState<DriveCandidate[]>([]);
  const [networkAdapters, setNetworkAdapters] = useState<NetworkAdapterCandidate[]>([]);
  const [externalIp, setExternalIp] = useState<string | null>(null);
  const [networkDetection, setNetworkDetection] = useState<DetectionState>("idle");
  const [duneVms, setDuneVms] = useState<DuneVmCandidate[]>([]);
  const [localHyperVRuntimes, setLocalHyperVRuntimes] = useState<Record<string, LocalHyperVRuntime>>({});
  const [localHyperVRuntimeErrors, setLocalHyperVRuntimeErrors] = useState<Record<string, string>>({});
  const [vmDestinationHasVm, setVmDestinationHasVm] = useState(false);
  const [remoteServers, setRemoteServers] = useState<RemoteServerRecord[]>([]);
  const [remoteServerStatuses, setRemoteServerStatuses] = useState<Record<string, RemoteServerStatus>>({});
  const [remoteServerComponents, setRemoteServerComponents] = useState<Record<string, RemoteServerComponent[]>>({});
  const [remoteComponentLogs, setRemoteComponentLogs] = useState<Record<string, string>>({});
  const [remoteComponentLogBusy, setRemoteComponentLogBusy] = useState<Record<string, boolean>>({});
  const [remoteComponentRestartBusy, setRemoteComponentRestartBusy] = useState<Record<string, boolean>>({});
  const [remoteServerStatusErrors, setRemoteServerStatusErrors] = useState<Record<string, string>>({});
  const [remoteServerBusy, setRemoteServerBusy] = useState<Record<string, string>>({});
  const [serverTunnels, setServerTunnels] = useState<Record<string, ServerTunnelStatus>>({});
  const [serverTunnelBusy, setServerTunnelBusy] = useState<Record<string, boolean>>({});
  const [remotePreflight, setRemotePreflight] = useState<UbuntuSshPreflight | null>(null);
  const [remotePreflightStatus, setRemotePreflightStatus] = useState<DetectionState>("idle");
  const [proxmoxDetection, setProxmoxDetection] = useState<ProxmoxDetection | null>(null);
  const [proxmoxDetectionStatus, setProxmoxDetectionStatus] = useState<DetectionState>("idle");
  const [proxmoxVmStatuses, setProxmoxVmStatuses] = useState<Record<string, ProxmoxVmStatus>>({});
  const calculatedMemory = useMemo(() => calculateRequiredMemory(form), [form]);
  const environmentGate = useMemo(
    () => setupEnvironmentGate(networkDetection, hostReadiness, networkAdapters),
    [hostReadiness, networkAdapters, networkDetection],
  );
  const layoutPreview = useMemo(() => setupLayoutPreview(form), [form]);
  const updateCheckInFlight = useRef(false);
  const lastPersistedLogRowId = useRef(0);
  const update = <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => {
    setForm((current) => normalizeSetupForm({ ...current, [key]: value }));
  };
  const appendInitRow = (row: LogRow) => {
    setInitRows((rows) => limitLogRows([...rows, row]));
  };
  const appendSetupRow = (row: LogRow) => {
    setSetupRows((rows) => limitLogRows([...rows, row]));
  };
  const clearLogRows = () => {
    setInitRows([]);
    setSetupRows([]);
  };
  const checkForAppUpdate = async (source: "startup" | "manual") => {
    if (updateCheckInFlight.current) return;
    updateCheckInFlight.current = true;
    setUpdateStatus("checking");
    setUpdateProgress(null);
    appendInitRow(log.info("updates", "Checking for app updates."));
    try {
      const nextUpdate = await check({ timeout: 15_000 });
      setAvailableUpdate(nextUpdate);
      if (nextUpdate) {
        setUpdateStatus("available");
        appendInitRow(
          log.info(
            "updates",
            `Update ${nextUpdate.version} is available; current version is ${nextUpdate.currentVersion}.`,
          ),
        );
        setUpdateDialogOpen(true);
      } else {
        setUpdateStatus("current");
        appendInitRow(log.info("updates", "The app is up to date."));
      }
    } catch (err) {
      setUpdateStatus("failed");
      appendInitRow(log.warn("updates", `Update check failed: ${errorMessage(err)}`));
    } finally {
      updateCheckInFlight.current = false;
    }
  };
  const refreshServerPackageStatus = async () => {
    setServerPackageCheckStatus("checking");
    appendInitRow(log.info("server-package", "Checking Dune server package status."));
    try {
      const status = await invoke<ServerPackageStatus>("server_package_status");
      setServerPackageStatus(status);
      setServerPackageCheckStatus(
        !status.complete ? "missing" : status.updateAvailable ? "available" : "current",
      );
      appendInitRow(log.info("server-package", status.message));
    } catch (err) {
      setServerPackageCheckStatus("failed");
      appendInitRow(log.warn("server-package", `Package status check failed: ${errorMessage(err)}`));
    }
  };
  const updateServerPackage = async () => {
    setServerPackageCheckStatus("updating");
    try {
      const status = await invoke<ServerPackageStatus>("update_server_package");
      setServerPackageStatus(status);
      setServerPackageCheckStatus(
        !status.complete ? "missing" : status.updateAvailable ? "available" : "current",
      );
    } catch (err) {
      setServerPackageCheckStatus("failed");
      appendSetupRow(log.error("server-package", errorMessage(err)));
    }
  };
  const installAppUpdate = async () => {
    if (!availableUpdate) return;
    let downloaded = 0;
    let total: number | null = null;
    setUpdateStatus("installing");
    setUpdateProgress("Preparing download...");
    appendInitRow(log.info("updates", `Installing update ${availableUpdate.version}.`));
    try {
      await availableUpdate.downloadAndInstall(
        (event: DownloadEvent) => {
          if (event.event === "Started") {
            total = event.data.contentLength ?? null;
            downloaded = 0;
            setUpdateProgress(total ? `Downloading 0 of ${formatBytes(total)}` : "Downloading update...");
          }
          if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
            setUpdateProgress(
              total
                ? `Downloading ${formatBytes(downloaded)} of ${formatBytes(total)}`
                : `Downloading ${formatBytes(downloaded)}`,
            );
          }
          if (event.event === "Finished") {
            setUpdateProgress("Installing update...");
          }
        },
        { timeout: 120_000 },
      );
      setUpdateStatus("relaunching");
      setUpdateProgress("Relaunching...");
      appendInitRow(log.info("updates", "Update installed; relaunching the app."));
      await relaunch();
    } catch (err) {
      setUpdateStatus("failed");
      setUpdateProgress(null);
      appendInitRow(log.error("updates", errorMessage(err)));
    }
  };

  const runLocalDetection = async () => {
    setNetworkDetection("detecting");
    setSetupRows((rows) => [...rows, log.info("capabilities", "Detecting local host capabilities.")]);
    try {
      const [location, environment] = await Promise.all([
        invoke<string>("default_vm_location").catch(() => ""),
        invoke<EnvironmentDetection>("detect_environment"),
      ]);
      setHostReadiness(environment.readiness);
      setDriveCandidates(environment.drives);
      setNetworkAdapters(environment.networkAdapters);
      setExternalIp(environment.externalIp);
      setNetworkDetection("ready");
      if (location) {
        setForm((current) => (current.vmDestination ? current : { ...current, vmDestination: location }));
      }
      const first = environment.networkAdapters[0];
      if (first) {
        setForm((current) => ({
          ...current,
          adapterName: current.adapterName || first.interfaceDescription,
          switchName: current.switchName || first.existingExternalSwitch || defaultHyperVSwitchName,
          staticIp: current.staticIp || first.suggestedIpv4Address,
          playerIp: current.playerIp || (current.playerIpMode === "external" && environment.externalIp
            ? environment.externalIp
            : first.suggestedIpv4Address),
          gateway: current.gateway || first.gateway,
        }));
      }
      const gate = setupEnvironmentGate("ready", environment.readiness, environment.networkAdapters);
      setSetupRows((rows) => [
        ...rows,
        log.info("capabilities", "Local host capability detection completed."),
        ...environmentLogRows(
          "ready",
          environment.readiness,
          environment.networkAdapters,
          environment.drives,
          environment.externalIp,
          gate,
        ),
      ]);
    } catch (err) {
      setNetworkDetection("failed");
      setSetupRows((rows) => [...rows, log.error("capabilities", errorMessage(err))]);
    }
  };

  const runRemotePreflight = async () => {
    setRemotePreflightStatus("detecting");
    setRemotePreflight(null);
    setSetupRows((rows) => [...rows, log.info("ubuntu.preflight", "Checking remote Ubuntu host resources.")]);
    try {
      const preflight = await invoke<UbuntuSshPreflight>("preflight_remote_ubuntu", {
        request: remoteSetupRunRequest(form),
      });
      setRemotePreflight(preflight);
      setRemotePreflightStatus("ready");
      const accessMessage = preflight.uid === 0
        ? `Remote access: ${preflight.user} is root.`
        : preflight.passwordlessSudo
          ? `Remote access: ${preflight.user} can run passwordless sudo.`
          : `Remote access: ${preflight.user} needs a sudo password; setup requires root or passwordless sudo. ${preflight.sudoCheck}`;
      setSetupRows((rows) => [
        ...rows,
        log.info(
          "ubuntu.preflight",
          `Remote resources: ${formatGiB(preflight.availableMemoryBytes)} available memory, ${preflight.logicalProcessorCount} logical CPUs, ${formatGiB(preflight.rootDiskAvailableBytes)} disk free.`,
        ),
        preflight.uid === 0 || preflight.passwordlessSudo
          ? log.info("ubuntu.preflight", accessMessage)
          : log.error("ubuntu.preflight", accessMessage),
      ]);
      const publicIp = preflight.publicIp;
      if (publicIp && form.playerIpMode === "external" && form.playerIp !== publicIp) {
        update("playerIp", publicIp);
      } else if (publicIp && form.setupTarget === "ubuntu" && !form.playerIp.trim()) {
        setForm((current) => normalizeSetupForm({ ...current, playerIpMode: "external", playerIp: publicIp }));
      }
    } catch (err) {
      setRemotePreflightStatus("failed");
      setSetupRows((rows) => [...rows, log.error("ubuntu.preflight", errorMessage(err))]);
    }
  };

  const runProxmoxDetection = async () => {
    setProxmoxDetectionStatus("detecting");
    setProxmoxDetection(null);
    setSetupRows((rows) => [...rows, log.info("proxmox.detect", "Detecting Proxmox resources.")]);
    try {
      const detection = await invoke<ProxmoxDetection>("detect_proxmox", {
        request: proxmoxConnectionRequest(form),
      });
      setProxmoxDetection(detection);
      setProxmoxDetectionStatus("ready");
      const firstNode = detection.nodes[0]?.node || "";
      const vmStorage = detection.storages.find((storage) => storage.content.includes("images"))?.storage || detection.storages[0]?.storage || "";
      const importStorage = detection.storages.find((storage) => storage.content.includes("import"))?.storage || detection.storages[0]?.storage || "";
      const bridge = detection.bridges[0];
      setForm((current) => ({
        ...current,
        proxmoxNode: current.proxmoxNode || firstNode,
        proxmoxVmStorage: current.proxmoxVmStorage || vmStorage,
        proxmoxImportStorage: current.proxmoxImportStorage || importStorage,
        proxmoxBridge: current.proxmoxBridge || bridge?.iface || "",
        proxmoxBridgeCidr: current.proxmoxBridgeCidr || bridge?.cidr || "",
        proxmoxVmid: current.proxmoxVmid || String(detection.nextVmid || ""),
        proxmoxAcceptedCertificateSha256:
          current.proxmoxAcceptedCertificateSha256 || detection.certificateSha256,
      }));
      setSetupRows((rows) => [
        ...rows,
        log.info(
          "proxmox.detect",
          `Detected Proxmox ${detection.version.version}; ${detection.nodes.length} node(s), ${detection.storages.length} storage target(s), ${detection.bridges.length} bridge(s).`,
        ),
      ]);
      persistProxmoxProfile({
        hostUrl: form.proxmoxHostUrl.trim(),
        tokenId: form.proxmoxTokenId.trim(),
        acceptedCertificateSha256: detection.certificateSha256,
      });
    } catch (err) {
      setProxmoxDetectionStatus("failed");
      setSetupRows((rows) => [...rows, log.error("proxmox.detect", errorMessage(err))]);
    }
  };

  const generateUbuntuSshKey = async () => {
    setSshKeyGenerationRunning(true);
    setSetupRows((rows) => [...rows, log.info("ssh-key", "Generating an Ubuntu setup SSH key pair.")]);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose where to create the SSH key pair",
      });
      if (typeof selected !== "string") return;
      const result = await invoke<GenerateSshKeyResult>("generate_ubuntu_ssh_key", {
        request: {
          directory: selected,
          fileName: "dune_ubuntu_setup_ed25519",
        },
      });
      setGeneratedSshKey(result);
      update("remoteKeyPath", result.privateKeyPath);
      setSetupRows((rows) => [...rows, log.info("ssh-key", `Generated SSH key pair at ${result.privateKeyPath}.`)]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("ssh-key", errorMessage(err))]);
    } finally {
      setSshKeyGenerationRunning(false);
    }
  };

  const generateProxmoxSshKey = async () => {
    setSshKeyGenerationRunning(true);
    setSetupRows((rows) => [...rows, log.info("ssh-key", "Generating a Proxmox guest SSH key pair.")]);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Choose where to create the Proxmox guest SSH key pair",
      });
      if (typeof selected !== "string") return;
      const result = await invoke<GenerateSshKeyResult>("generate_ubuntu_ssh_key", {
        request: {
          directory: selected,
          fileName: "dune_proxmox_guest_ed25519",
        },
      });
      setGeneratedSshKey(result);
      update("proxmoxSshKeyPath", result.privateKeyPath);
      setSetupRows((rows) => [...rows, log.info("ssh-key", `Generated Proxmox guest SSH key pair at ${result.privateKeyPath}.`)]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("ssh-key", errorMessage(err))]);
    } finally {
      setSshKeyGenerationRunning(false);
    }
  };

  const attachRemoteServer = async () => {
    setRemoteAttachRunning(true);
    setSetupRows((rows) => [...rows, log.info("remote.attach", "Adding remote server profile.")]);
    try {
      const record = remoteServerPlaceholder({
        type: remoteAttachForm.type,
        host: remoteAttachForm.host.trim(),
        keyPath: remoteAttachForm.keyPath.trim(),
        createdAt: new Date().toISOString(),
      });
      setRemoteServerStatuses((statuses) => omitKey(statuses, record.id));
      setRemoteServerComponents((components) => omitKey(components, record.id));
      setRemoteComponentLogs((logs) => omitPrefix(logs, `${record.id}:`));
      setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${record.id}:`));
      setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${record.id}:`));
      setRemoteServerStatusErrors((errors) => omitKey(errors, record.id));
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, record)));
      setActivePage("servers");
      setRemoteAttachOpen(false);
      setRemoteAttachForm(defaultRemoteAttachForm);
      setSetupRows((rows) => [
        ...rows,
        log.info("remote.attach", "Added remote server profile."),
      ]);
      void refreshRemoteServerStatus(record);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("remote.attach", errorMessage(err))]);
    } finally {
      setRemoteAttachRunning(false);
    }
  };

  const attachLocalHyperVServer = async () => {
    if (!localAttachForm.vmName.trim()) return;
    setLocalAttachRunning(true);
    setSetupRows((rows) => [...rows, log.info("local.attach", `Registering Hyper-V VM ${localAttachForm.vmName.trim()}.`)]);
    try {
      const candidate = await invoke<DuneVmCandidate>("register_local_hyperv_server", {
        request: { vmName: localAttachForm.vmName.trim() },
      });
      const record = mergeLocalServerAddress(localServerPlaceholder(candidate.vm.name, localAttachForm.staticIp), candidate);
      setDuneVms((servers) => persistLocalServers(upsertLocalServer(servers, record)));
      setLocalAttachOpen(false);
      setLocalAttachForm(defaultLocalHyperVAttachForm);
      setSetupRows((rows) => [...rows, log.info("local.attach", `Added local Hyper-V VM ${candidate.vm.name}.`)]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("local.attach", errorMessage(err))]);
    } finally {
      setLocalAttachRunning(false);
    }
  };

  const removeRemoteServer = (server: RemoteServerRecord) => {
    stopTunnelsForServer(server.id);
    setRemoteServers((servers) => {
      const next = persistRemoteServers(servers.filter((candidate) => candidate.id !== server.id));
      return next;
    });
    setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
    setRemoteServerComponents((components) => omitKey(components, server.id));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    setSetupRows((rows) => [...rows, log.info("remote.attach", "Forgot remote server profile.")]);
    setRemoteServerToRemove(null);
  };

  const removeLocalHyperVServer = (server: DuneVmCandidate) => {
    stopTunnelsForServer(localServerKey(server));
    setDuneVms((servers) => persistLocalServers(servers.filter((candidate) => candidate.vm.name !== server.vm.name)));
    setLocalHyperVRuntimes((runtimes) => omitKey(runtimes, localServerKey(server)));
    setLocalHyperVRuntimeErrors((errors) => omitKey(errors, localServerKey(server)));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${localServerKey(server)}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${localServerKey(server)}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${localServerKey(server)}:`));
    setSetupRows((rows) => [...rows, log.info("local.attach", `Forgot local Hyper-V VM ${server.vm.name}.`)]);
  };

  const stopTunnelsForServer = (serverKey: string) => {
    for (const tunnelId of Object.keys(serverTunnels).filter((id) => id.startsWith(`${serverKey}:tunnel:`))) {
      void stopServerTunnel(tunnelId);
    }
  };

  const refreshLocalHyperVServer = async (server: DuneVmCandidate) => {
    const serverKey = localServerKey(server);
    setRemoteServerBusy((busy) => ({ ...busy, [serverKey]: "Retrieving server information" }));
    setLocalHyperVRuntimeErrors((errors) => omitKey(errors, serverKey));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${serverKey}:`));
    try {
      const candidate = await invoke<DuneVmCandidate>("register_local_hyperv_server", {
        request: { vmName: server.vm.name },
      });
      const mergedCandidate = mergeLocalServerAddress(server, candidate);
      setDuneVms((servers) => persistLocalServers(upsertLocalServer(servers, mergedCandidate)));
      if (mergedCandidate.vm.state === "running") {
        const runtime = await invoke<LocalHyperVRuntime>("local_hyperv_runtime", {
          request: { vmName: mergedCandidate.vm.name, host: primaryLocalServerIp(mergedCandidate) },
        });
        setLocalHyperVRuntimes((runtimes) => ({ ...runtimes, [localServerKey(mergedCandidate)]: runtime }));
      } else {
        setLocalHyperVRuntimes((runtimes) => omitKey(runtimes, localServerKey(mergedCandidate)));
      }
    } catch (err) {
      const message = errorMessage(err);
      setLocalHyperVRuntimeErrors((errors) => ({ ...errors, [serverKey]: message }));
      setSetupRows((rows) => [...rows, log.warn("local.status", message)]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, serverKey));
    }
  };

  const runLocalHyperVAction = async (server: DuneVmCandidate, action: "start" | "stop") => {
    const serverKey = localServerKey(server);
    setRemoteServerBusy((busy) => ({ ...busy, [serverKey]: action === "start" ? "Starting VM" : "Stopping VM" }));
    try {
      const candidate = await invoke<DuneVmCandidate>(
        action === "start" ? "start_local_hyperv_server" : "stop_local_hyperv_server",
        { request: { vmName: server.vm.name } },
      );
      setDuneVms((servers) => persistLocalServers(upsertLocalServer(servers, candidate)));
      setLocalHyperVRuntimes((runtimes) => omitKey(runtimes, serverKey));
      setLocalHyperVRuntimeErrors((errors) => omitKey(errors, serverKey));
      if (candidate.vm.state === "running") {
        void refreshLocalHyperVServer(candidate);
      }
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("local.vm", errorMessage(err))]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, serverKey));
    }
  };

  const detectRemoteServerDetails = async (server: RemoteServerRecord): Promise<RemoteServerRecord> => {
    const command = server.type === "alpine" ? "detect_remote_alpine_servers" : "detect_remote_ubuntu_servers";
    const detected = await invoke<RemoteServerRecord[]>(command, {
      request:
        server.type === "alpine"
          ? { host: server.host, keyPath: server.keyPath, serverType: "alpine", user: "dune" }
          : { host: server.host, keyPath: server.keyPath, serverType: "ubuntu", user: "root" },
    });
    if (detected.length === 0) {
      throw new Error("No Dune battlegroups were detected on the remote server.");
    }
    const selected =
      detected.find((candidate) => candidate.battlegroupName === server.battlegroupName) ?? detected[0];
    return remoteServerFromDetected(server, selected);
  };

  const refreshRemoteServerStatus = async (server: RemoteServerRecord) => {
    if (!server.host || !server.keyPath) return;
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: "Retrieving server information" }));
    setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
    setRemoteServerComponents((components) => omitKey(components, server.id));
    setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
    setRemoteComponentLogBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteComponentRestartBusy((busy) => omitPrefix(busy, `${server.id}:`));
    setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
    setSetupRows((rows) => [...rows, log.info("remote.status", "Retrieving remote server information.")]);
    try {
      const liveServer = await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const status = await invoke<RemoteServerStatus>("remote_server_status", {
        request: remoteServerActionRequest(liveServer),
      });
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerStatuses((statuses) => ({ ...statuses, [server.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [server.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === server.id ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase } : candidate,
          ),
        ),
      );
      setSetupRows((rows) => [
        ...rows,
        log.info(
          "remote.status",
          `${liveServer.battlegroupName}: ${status.battlegroup.phase || "unknown"}, server group ${status.battlegroup.serverGroupPhase || "unknown"}, Director ${status.battlegroup.directorPhase || "unknown"}.`,
        ),
      ]);
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatuses((statuses) => omitKey(statuses, server.id));
      setRemoteServerComponents((components) => omitKey(components, server.id));
      setRemoteComponentLogs((logs) => omitPrefix(logs, `${server.id}:`));
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      setSetupRows((rows) => [...rows, log.warn("remote.status", message)]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const runRemoteBattlegroupAction = async (server: RemoteServerRecord, action: "start" | "stop" | "update") => {
    const busyText =
      action === "start" ? "Starting battlegroup" : action === "stop" ? "Stopping battlegroup" : "Updating battlegroup";
    const verb = action === "start" ? "Starting" : action === "stop" ? "Stopping" : "Updating";
    setRemoteServerBusy((busy) => ({ ...busy, [server.id]: busyText }));
    setSetupRows((rows) => [...rows, log.info("bg", `${verb} remote battlegroup.`)]);
    try {
      const liveServer = server.namespace && server.battlegroupName ? server : await detectRemoteServerDetails(server);
      setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      const command =
        action === "start"
          ? "start_remote_battlegroup"
          : action === "stop"
            ? "stop_remote_battlegroup"
            : "update_remote_battlegroup";
      const status = await invoke<RemoteServerStatus>(command, { request: remoteServerActionRequest(liveServer) });
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerStatuses((statuses) => ({ ...statuses, [server.id]: status }));
      setRemoteServerComponents((current) => ({ ...current, [server.id]: components }));
      setRemoteServerStatusErrors((errors) => omitKey(errors, server.id));
      setRemoteServers((servers) =>
        persistRemoteServers(
          servers.map((candidate) =>
            candidate.id === server.id ? { ...liveServer, phase: status.battlegroup.phase || liveServer.phase } : candidate,
          ),
        ),
      );
    } catch (err) {
      const message = errorMessage(err);
      setRemoteServerStatusErrors((errors) => ({ ...errors, [server.id]: message }));
      setSetupRows((rows) => [...rows, log.error("bg", message)]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, server.id));
    }
  };

  const runProxmoxVmAction = async (server: RemoteServerRecord, action: "start" | "stop" | "status") => {
    if (!server.provisioner) return;
    const key = server.id;
    setRemoteServerBusy((busy) => ({ ...busy, [key]: action === "status" ? "Checking Proxmox VM" : `${action === "start" ? "Starting" : "Stopping"} Proxmox VM` }));
    try {
      const command =
        action === "start" ? "start_proxmox_vm" : action === "stop" ? "stop_proxmox_vm" : "proxmox_vm_status";
      const status = await invoke<ProxmoxVmStatus>(command, {
        request: proxmoxVmActionRequest(server.provisioner),
      });
      setProxmoxVmStatuses((statuses) => ({ ...statuses, [key]: status }));
      setSetupRows((rows) => [...rows, log.info("proxmox.vm", `VM ${server.provisioner?.vmid} is ${status.status || "unknown"}.`)]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("proxmox.vm", errorMessage(err))]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, key));
    }
  };

  const runLocalHyperVBattlegroupAction = async (server: DuneVmCandidate, action: "start" | "stop" | "update") => {
    const serverKey = localServerKey(server);
    const runtime = localHyperVRuntimes[serverKey];
    if (!runtime) return;
    const busyText =
      action === "start" ? "Starting battlegroup" : action === "stop" ? "Stopping battlegroup" : "Updating battlegroup";
    const verb = action === "start" ? "Starting" : action === "stop" ? "Stopping" : "Updating";
    setRemoteServerBusy((busy) => ({
      ...busy,
      [serverKey]: busyText,
    }));
    setSetupRows((rows) => [...rows, log.info("bg", `${verb} local battlegroup.`)]);
    try {
      const command =
        action === "start"
          ? "start_local_hyperv_battlegroup"
          : action === "stop"
            ? "stop_local_hyperv_battlegroup"
            : "update_local_hyperv_battlegroup";
      const status = await invoke<RemoteServerStatus>(command, {
        request: {
          vmName: server.vm.name,
          host: primaryLocalServerIp(server),
          namespace: runtime.namespace,
          battlegroupName: runtime.battlegroupName,
        },
      });
      setLocalHyperVRuntimes((runtimes) => ({
        ...runtimes,
        [serverKey]: { ...runtime, status },
      }));
      void refreshLocalHyperVServer(server);
    } catch (err) {
      const message = errorMessage(err);
      setLocalHyperVRuntimeErrors((errors) => ({ ...errors, [serverKey]: message }));
      setSetupRows((rows) => [...rows, log.error("bg", message)]);
    } finally {
      setRemoteServerBusy((busy) => omitKey(busy, serverKey));
    }
  };

  const startServerTunnel = async (request: ServerTunnelStartRequest) => {
    setServerTunnelBusy((busy) => ({ ...busy, [request.tunnelId]: true }));
    setSetupRows((rows) => [...rows, log.info("tunnel", `Starting ${tunnelServiceLabel(request.service)} tunnel.`)]);
    try {
      const status = await invoke<ServerTunnelStatus>("start_server_tunnel", { request });
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      setSetupRows((rows) => [
        ...rows,
        log.info("tunnel", `${tunnelServiceLabel(request.service)} tunnel is ready at ${status.url}`),
      ]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("tunnel", errorMessage(err))]);
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, request.tunnelId));
    }
  };

  const openServerTunnel = async (tunnel: ServerTunnelStatus) => {
    try {
      const status = await invoke<ServerTunnelStatus | null>("server_tunnel_status", {
        request: { tunnelId: tunnel.tunnelId },
      });
      if (!status) {
        setServerTunnels((tunnels) => omitKey(tunnels, tunnel.tunnelId));
        setSetupRows((rows) => [...rows, log.warn("tunnel", "The SSH tunnel is no longer running.")]);
        return;
      }
      setServerTunnels((tunnels) => ({ ...tunnels, [status.tunnelId]: status }));
      if (status.service === "database") {
        await copyTextToClipboard(status.url);
        setSetupRows((rows) => [...rows, log.info("tunnel", `Copied Postgres connection URI ${status.url}`)]);
        return;
      }
      await openExternal(status.url);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("tunnel", errorMessage(err))]);
    }
  };

  const stopServerTunnel = async (tunnelId: string) => {
    setServerTunnelBusy((busy) => ({ ...busy, [tunnelId]: true }));
    try {
      await invoke("stop_server_tunnel", { request: { tunnelId } });
      setServerTunnels((tunnels) => omitKey(tunnels, tunnelId));
      setSetupRows((rows) => [...rows, log.info("tunnel", "SSH tunnel stopped.")]);
    } catch (err) {
      setSetupRows((rows) => [...rows, log.error("tunnel", errorMessage(err))]);
    } finally {
      setServerTunnelBusy((busy) => omitKey(busy, tunnelId));
    }
  };

  const refreshRemoteComponentLog = async (server: RemoteServerRecord, component: RemoteServerComponent) => {
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentLogBusy((busy) => ({ ...busy, [key]: true }));
    setSetupRows((rows) => [...rows, log.info("remote.logs", `Refreshing ${component.name} logs.`)]);
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      if (!server.namespace) {
        setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      }
      const result = await invoke<RemoteComponentLogResult>("remote_component_log_tail", {
        request: {
          serverType: liveServer.type,
          host: liveServer.host,
          user: liveServer.user || remoteServerDefaultUser(liveServer.type),
          keyPath: liveServer.keyPath || undefined,
          namespace: liveServer.namespace,
          component: component.logKey,
          tail: 160,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || "No log output."),
      }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      setSetupRows((rows) => [...rows, log.warn("remote.logs", message)]);
    } finally {
      setRemoteComponentLogBusy((busy) => omitKey(busy, key));
    }
  };

  const refreshLocalHyperVComponentLog = async (server: DuneVmCandidate, component: RemoteServerComponent) => {
    const serverKey = localServerKey(server);
    const runtime = localHyperVRuntimes[serverKey];
    if (!runtime) return;
    const key = componentLogStateKey(serverKey, component);
    setRemoteComponentLogBusy((busy) => ({ ...busy, [key]: true }));
    setSetupRows((rows) => [...rows, log.info("local.logs", `Refreshing ${component.name} logs.`)]);
    try {
      const result = await invoke<RemoteComponentLogResult>("local_hyperv_component_log_tail", {
        request: {
          vmName: server.vm.name,
          host: primaryLocalServerIp(server),
          namespace: runtime.namespace,
          component: component.logKey,
          tail: 160,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || "No log output."),
      }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      setSetupRows((rows) => [...rows, log.warn("local.logs", message)]);
    } finally {
      setRemoteComponentLogBusy((busy) => omitKey(busy, key));
    }
  };

  const restartRemoteComponent = async (server: RemoteServerRecord, component: RemoteServerComponent) => {
    if (isCriticalRestartComponent(component)) {
      const confirmed = window.confirm(
        `Restart ${component.name}? This can temporarily interrupt persistence, messaging, or active players.`,
      );
      if (!confirmed) return;
    }
    const key = componentLogStateKey(server.id, component);
    setRemoteComponentRestartBusy((busy) => ({ ...busy, [key]: true }));
    setSetupRows((rows) => [...rows, log.warn("remote.restart", `Restarting ${component.name}.`)]);
    try {
      const liveServer = server.namespace ? server : await detectRemoteServerDetails(server);
      if (!server.namespace) {
        setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, liveServer)));
      }
      const result = await invoke<{ output: string }>("restart_remote_component", {
        request: {
          serverType: liveServer.type,
          host: liveServer.host,
          user: liveServer.user || remoteServerDefaultUser(liveServer.type),
          keyPath: liveServer.keyPath || undefined,
          namespace: liveServer.namespace,
          component: component.logKey,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || `${component.name} restart requested.`),
      }));
      const components = await invoke<RemoteServerComponent[]>("remote_server_components", {
        request: remoteServerActionRequest(liveServer),
      });
      setRemoteServerComponents((current) => ({ ...current, [server.id]: components }));
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      setSetupRows((rows) => [...rows, log.error("remote.restart", message)]);
    } finally {
      setRemoteComponentRestartBusy((busy) => omitKey(busy, key));
    }
  };

  const restartLocalHyperVComponent = async (server: DuneVmCandidate, component: RemoteServerComponent) => {
    if (isCriticalRestartComponent(component)) {
      const confirmed = window.confirm(
        `Restart ${component.name}? This can temporarily interrupt persistence, messaging, or active players.`,
      );
      if (!confirmed) return;
    }
    const serverKey = localServerKey(server);
    const runtime = localHyperVRuntimes[serverKey];
    if (!runtime) return;
    const key = componentLogStateKey(serverKey, component);
    setRemoteComponentRestartBusy((busy) => ({ ...busy, [key]: true }));
    setSetupRows((rows) => [...rows, log.warn("local.restart", `Restarting ${component.name}.`)]);
    try {
      const result = await invoke<{ output: string }>("restart_local_hyperv_component", {
        request: {
          vmName: server.vm.name,
          host: primaryLocalServerIp(server),
          namespace: runtime.namespace,
          component: component.logKey,
          tail: 160,
        },
      });
      setRemoteComponentLogs((logs) => ({
        ...logs,
        [key]: sanitizeLogMessage(result.output || `${component.name} restart requested.`),
      }));
      void refreshLocalHyperVServer(server);
    } catch (err) {
      const message = errorMessage(err);
      setRemoteComponentLogs((logs) => ({ ...logs, [key]: sanitizeLogMessage(message) }));
      setSetupRows((rows) => [...rows, log.error("local.restart", message)]);
    } finally {
      setRemoteComponentRestartBusy((busy) => omitKey(busy, key));
    }
  };

  useEffect(() => {
    const text = window.localStorage.getItem(remoteProfileStorageKey);
    if (!text) return;
    try {
      const profile = JSON.parse(text) as Partial<Pick<SetupForm, "remoteHost" | "remoteUser" | "remoteKeyPath">>;
      setForm((current) =>
        normalizeSetupForm({
          ...current,
          remoteHost: profile.remoteHost || current.remoteHost,
          remoteUser: profile.remoteUser || current.remoteUser,
          remoteKeyPath: profile.remoteKeyPath || current.remoteKeyPath,
        }),
      );
    } catch {
      window.localStorage.removeItem(remoteProfileStorageKey);
    }
  }, []);

  useEffect(() => {
    const text = window.localStorage.getItem(proxmoxProfileStorageKey);
    if (!text) return;
    try {
      const profile = JSON.parse(text) as Partial<
        Pick<SetupForm, "proxmoxHostUrl" | "proxmoxTokenId" | "proxmoxAcceptedCertificateSha256">
      >;
      setForm((current) =>
        normalizeSetupForm({
          ...current,
          proxmoxHostUrl: profile.proxmoxHostUrl || current.proxmoxHostUrl,
          proxmoxTokenId: profile.proxmoxTokenId || current.proxmoxTokenId,
          proxmoxAcceptedCertificateSha256:
            profile.proxmoxAcceptedCertificateSha256 || current.proxmoxAcceptedCertificateSha256,
        }),
      );
    } catch {
      window.localStorage.removeItem(proxmoxProfileStorageKey);
    }
  }, []);

  useEffect(() => {
    setDuneVms(readLocalServers());
    setRemoteServers(readRemoteServers());
    void refreshServerPackageStatus();
  }, []);

  useEffect(() => {
    let cancelled = false;
    for (const server of remoteServers) {
      if (!server.host || !server.keyPath || remoteServerBusy[server.id]) continue;
      void refreshRemoteServerStatus(server);
    }
    return () => {
      cancelled = true;
    };
  }, [remoteServers.map((server) => server.id).join("|")]);

  useEffect(() => {
    for (const server of duneVms) {
      if (remoteServerBusy[localServerKey(server)]) continue;
      void refreshLocalHyperVServer(server);
    }
  }, [duneVms.map((server) => server.vm.name).join("|")]);

  useEffect(() => {
    const profile = {
      remoteHost: form.remoteHost,
      remoteUser: form.remoteUser,
      remoteKeyPath: form.remoteKeyPath,
    };
    window.localStorage.setItem(remoteProfileStorageKey, JSON.stringify(profile));
    setRemotePreflight(null);
    setRemotePreflightStatus("idle");
  }, [form.remoteHost, form.remoteKeyPath, form.remoteUser]);

  useEffect(() => {
    persistProxmoxProfile({
      hostUrl: form.proxmoxHostUrl,
      tokenId: form.proxmoxTokenId,
      acceptedCertificateSha256: form.proxmoxAcceptedCertificateSha256,
    });
  }, [form.proxmoxHostUrl, form.proxmoxTokenId, form.proxmoxAcceptedCertificateSha256]);

  useEffect(() => {
    setProxmoxDetection(null);
    setProxmoxDetectionStatus("idle");
  }, [form.proxmoxHostUrl, form.proxmoxTokenId]);

  useEffect(() => {
    if (!startupUpdateChecksEnabled) {
      appendInitRow(log.debug("updates", "Automatic update checks are disabled for this local build."));
      return;
    }

    const timer = window.setTimeout(() => {
      void checkForAppUpdate("startup");
    }, 1_500);

    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    return () => {
      void invoke("stop_all_tunnels");
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      for (const tunnel of Object.values(serverTunnels)) {
        invoke<ServerTunnelStatus | null>("server_tunnel_status", {
          request: { tunnelId: tunnel.tunnelId },
        })
          .then((status) => {
            if (!status) {
              setServerTunnels((tunnels) => omitKey(tunnels, tunnel.tunnelId));
            }
          })
          .catch(() => {
            setServerTunnels((tunnels) => omitKey(tunnels, tunnel.tunnelId));
          });
      }
    }, 5000);
    return () => window.clearInterval(timer);
  }, [serverTunnels]);

  useEffect(() => {
    const onError = (event: ErrorEvent) => {
      appendSetupRow(log.error("ui", event.message || "Unhandled browser error."));
    };
    const onRejection = (event: PromiseRejectionEvent) => {
      appendSetupRow(log.error("ui", errorMessage(event.reason)));
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<SetupLogPayload>("setup-log", (event) => {
      if (cancelled) return;
      appendSetupRow(logEntry(event.payload.level, event.payload.scope, event.payload.message));
    }).then((handler) => {
      unlisten = handler;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const path = form.vmDestination.trim();
    if (!/^[A-Za-z]:[\\/]/.test(path)) {
      setVmDestinationHasVm(false);
      return () => {
        cancelled = true;
      };
    }
    const timer = window.setTimeout(() => {
      invoke<boolean>("vm_destination_has_vm", { path })
        .then((hasVm) => {
          if (!cancelled) setVmDestinationHasVm(hasVm);
        })
        .catch(() => {
          if (!cancelled) setVmDestinationHasVm(false);
        });
    }, 150);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [form.vmDestination]);

  const logRows = useMemo(() => limitLogRows([...initRows, ...setupRows]), [initRows, setupRows]);
  const visibleLogRows = useMemo(
    () => filterLogRows(logRows, logLevelFilter).slice(-maxRenderedLogRows),
    [logLevelFilter, logRows],
  );

  useEffect(() => {
    const entries: AppLogEntry[] = logRows.filter((row) => row.id > lastPersistedLogRowId.current);
    if (entries.length === 0) return;
    lastPersistedLogRowId.current = entries.reduce(
      (highest, row) => Math.max(highest, row.id),
      lastPersistedLogRowId.current,
    );
    invoke<string>("append_app_log_entries", { entries }).catch((err) => {
      console.error("Failed to append app log entries", err);
    });
  }, [logRows]);

  const openAppLogFile = async () => {
    try {
      const path = await invoke<string>("open_app_log_file");
      appendInitRow(log.info("logs", `Opened app log file at ${path}.`));
    } catch (err) {
      appendSetupRow(log.error("logs", errorMessage(err)));
    }
  };

  const startSetup = async (proxmoxOverrides?: ProxmoxSetupOverrides) => {
    const formForRun: SetupForm =
      form.setupTarget === "proxmox" && proxmoxOverrides
        ? {
            ...form,
            proxmoxTemporaryDhcpIp: proxmoxOverrides.temporaryDhcpIp ?? form.proxmoxTemporaryDhcpIp,
            staticIp: proxmoxOverrides.staticIp ?? form.staticIp,
            gateway: proxmoxOverrides.gateway ?? form.gateway,
            dns: proxmoxOverrides.dns ?? form.dns,
            playerIp:
              form.playerIpMode === "local" && proxmoxOverrides.staticIp
                ? proxmoxOverrides.staticIp
                : form.playerIp,
          }
        : form;
    const setupMemoryGb =
      formForRun.setupTarget === "proxmox"
        ? effectiveProxmoxVmMemoryGb(formForRun, calculatedMemory, proxmoxDetection)
        : effectiveVmMemoryGb(formForRun, calculatedMemory);
    const request = setupRunRequest(formForRun, setupMemoryGb);
    setStarted(true);
    setSetupRunning(true);
    setFailedRollbackRequest(null);
    try {
      if (formForRun.setupTarget === "ubuntu") {
        const pendingRecord = formForRun.saveRemoteServer ? remoteServerDraftFromForm(formForRun) : null;
        if (pendingRecord) {
          setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, pendingRecord)));
        }
        const result = await invoke<{ namespace: string; battlegroupName: string; worldUniqueName: string }>("start_remote_ubuntu_setup", {
          request: remoteSetupRunRequest(formForRun),
        });
        setSetupRows((rows) => [
          ...rows,
          log.info("ubuntu", "Server provisioning completed. It can take some time before the server appears in-game."),
        ]);
        if (formForRun.saveRemoteServer) {
          const record = remoteServerRecordFromSetup(formForRun, result, pendingRecord?.id);
          setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, record)));
        }
      } else if (formForRun.setupTarget === "proxmox") {
        const pendingRecord =
          formForRun.saveRemoteServer &&
          (formForRun.staticIp.trim() || formForRun.proxmoxTemporaryDhcpIp.trim())
            ? proxmoxServerDraftFromForm(formForRun)
            : null;
        if (pendingRecord) {
          setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, pendingRecord)));
        }
        const runRequest = {
          ...proxmoxSetupRunRequest(formForRun, setupMemoryGb),
          temporaryDhcpIp: formForRun.proxmoxTemporaryDhcpIp.trim() || undefined,
        };
        const result = await invoke<{ host: string; user: string; keyPath: string; namespace: string; battlegroupName: string; worldUniqueName: string; node: string; vmid: number; vmName: string }>("start_proxmox_alpine_setup", {
          request: runRequest,
        });
        setSetupRows((rows) => [
          ...rows,
          log.info("proxmox", "Proxmox Alpine provisioning completed. The server is starting through the guest bootstrap flow."),
        ]);
        if (formForRun.saveRemoteServer) {
          const record = proxmoxServerRecordFromSetup(formForRun, result, pendingRecord?.id);
          setRemoteServers((servers) => persistRemoteServers(upsertRemoteServer(servers, record)));
        }
      } else {
        if (formForRun.saveLocalServer) {
          const pending = localServerPlaceholder(request.vmName, request.staticIp);
          setDuneVms((servers) => persistLocalServers(upsertLocalServer(servers, pending)));
          setLocalHyperVRuntimeErrors((errors) => omitKey(errors, localServerKey(pending)));
        }
        const result = await invoke<{ vmName: string; namespace: string; battlegroupName: string; worldUniqueName: string; directorNodePort: number | null }>("start_full_setup", {
          request,
        });
        if (formForRun.saveLocalServer) {
          try {
            const candidate = await invoke<DuneVmCandidate>("register_local_hyperv_server", {
              request: { vmName: result.vmName || request.vmName },
            });
            setDuneVms((servers) => persistLocalServers(upsertLocalServer(servers, candidate)));
          } catch (err) {
            setSetupRows((rows) => [...rows, log.warn("local.attach", `Setup completed but server registration failed: ${errorMessage(err)}`)]);
          }
        }
      }
    } catch (err) {
      console.error(err);
      const errorMsg = errorMessage(err);
      appendSetupRow(log.error("setup", errorMsg));
      if ((formForRun.setupTarget === "ubuntu" || formForRun.setupTarget === "proxmox") && formForRun.saveRemoteServer) {
        const pending =
          formForRun.setupTarget === "proxmox"
            ? formForRun.staticIp.trim() || formForRun.proxmoxTemporaryDhcpIp.trim()
              ? proxmoxServerDraftFromForm(formForRun)
              : null
            : remoteServerDraftFromForm(formForRun);
        if (pending) {
          setRemoteServers((servers) =>
            persistRemoteServers(upsertRemoteServer(servers, { ...pending, phase: "Setup failed" })),
          );
        }
      }
      if (
        formForRun.setupTarget === "proxmox" &&
        (errorMsg.includes("Temporary Proxmox DHCP IP") ||
          errorMsg.includes("temporary IP") ||
          errorMsg.includes("MAC") ||
          errorMsg.includes("ARP"))
      ) {
        setProxmoxIpPromptOpen(true);
      }
      if (formForRun.setupTarget === "hyperv") {
        setFailedRollbackRequest(rollbackRequestFromSetup(request));
        setRollbackOpen(true);
      }
    } finally {
      setSetupRunning(false);
    }
  };

  const rollback = async () => {
    if (!failedRollbackRequest) return;
    setRollbackRunning(true);
    try {
      await invoke("rollback_setup", { request: failedRollbackRequest });
      setRollbackOpen(false);
      setFailedRollbackRequest(null);
    } finally {
      setRollbackRunning(false);
    }
  };

  const confirmPendingServerUpdate = () => {
    const pending = pendingServerUpdate;
    if (!pending) return;
    setPendingServerUpdate(null);
    if (pending.type === "remote") {
      void runRemoteBattlegroupAction(pending.server, "update");
    } else {
      void runLocalHyperVBattlegroupAction(pending.server, "update");
    }
  };

  return (
    <Theme
      appearance="dark"
      accentColor="bronze"
      grayColor="sand"
      panelBackground="solid"
      radius="medium"
      scaling="95%"
    >
      <Flex direction="column" className="app-root">
        <Header
          activePage={activePage}
          onNavigate={setActivePage}
          serverCount={duneVms.length + remoteServers.length}
          updateStatus={updateStatus}
          update={availableUpdate}
          updateProgress={updateProgress}
          serverPackageStatus={serverPackageStatus}
          serverPackageCheckStatus={serverPackageCheckStatus}
          onCheckUpdate={() => void checkForAppUpdate("manual")}
          onOpenUpdate={() => setUpdateDialogOpen(true)}
          onCheckServerPackage={() => void refreshServerPackageStatus()}
          onUpdateServerPackage={() => void updateServerPackage()}
        />
        <Separator size="4" />
        <Box className={logPanelCollapsed ? "app-main log-collapsed" : "app-main has-log"}>
          <AppErrorBoundary
            onError={(message) => setSetupRows((rows) => [...rows, log.error("ui", message)])}
          >
            {activePage === "servers" ? (
              <ServersPage
                duneVms={duneVms}
                remoteServers={remoteServers}
                remoteStatuses={remoteServerStatuses}
                remoteComponents={remoteServerComponents}
                localRuntimes={localHyperVRuntimes}
                localRuntimeErrors={localHyperVRuntimeErrors}
                remoteComponentLogs={remoteComponentLogs}
                remoteComponentLogBusy={remoteComponentLogBusy}
                remoteComponentRestartBusy={remoteComponentRestartBusy}
                remoteStatusErrors={remoteServerStatusErrors}
                remoteBusy={remoteServerBusy}
                serverPackageStatus={serverPackageStatus}
                proxmoxVmStatuses={proxmoxVmStatuses}
                tunnels={serverTunnels}
                tunnelBusy={serverTunnelBusy}
                onAddLocalServer={() => setLocalAttachOpen(true)}
                onAddRemoteServer={() => {
                  setRemoteAttachForm({
                    type: "ubuntu",
                    host: form.remoteHost,
                    keyPath: form.remoteKeyPath,
                  });
                  setRemoteAttachOpen(true);
                }}
                onRemoveLocalServer={removeLocalHyperVServer}
                onRefreshLocalServer={(server) => void refreshLocalHyperVServer(server)}
                onStartLocalServer={(server) => void runLocalHyperVAction(server, "start")}
                onStopLocalServer={(server) => void runLocalHyperVAction(server, "stop")}
                onRemoveRemoteServer={setRemoteServerToRemove}
                onRefreshRemoteStatus={(server) => void refreshRemoteServerStatus(server)}
                onStartRemoteBattlegroup={(server) => void runRemoteBattlegroupAction(server, "start")}
                onStopRemoteBattlegroup={(server) => void runRemoteBattlegroupAction(server, "stop")}
                onUpdateRemoteBattlegroup={(server) => setPendingServerUpdate({ type: "remote", server })}
                onRefreshProxmoxVm={(server) => void runProxmoxVmAction(server, "status")}
                onStartProxmoxVm={(server) => void runProxmoxVmAction(server, "start")}
                onStopProxmoxVm={(server) => void runProxmoxVmAction(server, "stop")}
                onStartLocalBattlegroup={(server) => void runLocalHyperVBattlegroupAction(server, "start")}
                onStopLocalBattlegroup={(server) => void runLocalHyperVBattlegroupAction(server, "stop")}
                onUpdateLocalBattlegroup={(server) => setPendingServerUpdate({ type: "local", server })}
                onStartTunnel={(request) => void startServerTunnel(request)}
                onStopTunnel={(tunnelId) => void stopServerTunnel(tunnelId)}
                onOpenTunnel={(tunnel) => void openServerTunnel(tunnel)}
                onRefreshRemoteComponentLog={(server, component) =>
                  void refreshRemoteComponentLog(server, component)
                }
                onRestartRemoteComponent={(server, component) =>
                  void restartRemoteComponent(server, component)
                }
                onRefreshLocalComponentLog={(server, component) =>
                  void refreshLocalHyperVComponentLog(server, component)
                }
                onRestartLocalComponent={(server, component) =>
                  void restartLocalHyperVComponent(server, component)
                }
              />
            ) : null}
            {activePage === "install" ? (
              <InstallControls
                form={form}
                calculatedMemory={calculatedMemory}
                layoutPreview={layoutPreview}
                hostReadiness={hostReadiness}
                driveCandidates={driveCandidates}
                networkAdapters={networkAdapters}
                networkDetection={networkDetection}
                externalIp={externalIp}
                environmentGate={environmentGate}
                setupRunning={setupRunning}
                vmDestinationHasVm={vmDestinationHasVm}
                remotePreflight={remotePreflight}
                remotePreflightStatus={remotePreflightStatus}
                proxmoxDetection={proxmoxDetection}
                proxmoxDetectionStatus={proxmoxDetectionStatus}
                serverPackageStatus={serverPackageStatus}
                serverPackageCheckStatus={serverPackageCheckStatus}
                update={update}
                onUpdateServerPackage={() => void updateServerPackage()}
                onLocalDetection={() => void runLocalDetection()}
                onRemotePreflight={() => void runRemotePreflight()}
                onProxmoxDetection={() => void runProxmoxDetection()}
                onGenerateProxmoxSshKey={() => void generateProxmoxSshKey()}
                onStart={startSetup}
              />
            ) : null}
            {activePage === "tools" ? (
              <ToolsPage
                generatedSshKey={generatedSshKey}
                sshKeyGenerationRunning={sshKeyGenerationRunning}
                onGenerateUbuntuSshKey={() => void generateUbuntuSshKey()}
              />
            ) : null}
            <LogWindow
              rows={visibleLogRows}
              level={logLevelFilter}
              collapsed={logPanelCollapsed}
              onLevelChange={setLogLevelFilter}
              onClear={clearLogRows}
              onOpenLogFile={() => void openAppLogFile()}
              onToggleCollapsed={() => setLogPanelCollapsed((collapsed) => !collapsed)}
            />
          </AppErrorBoundary>
        </Box>
        <RollbackDialog
          open={rollbackOpen}
          rollbackRunning={rollbackRunning}
          onOpenChange={setRollbackOpen}
          onRollback={rollback}
        />
        <ProxmoxIpPromptDialog
          open={proxmoxIpPromptOpen}
          onOpenChange={setProxmoxIpPromptOpen}
          currentTemporaryIp={form.proxmoxTemporaryDhcpIp}
          currentStaticIp={form.staticIp}
          currentGateway={form.gateway}
          currentDns={form.dns}
          onCheck={(ip) =>
            invoke<ProxmoxNetworkProbeResult>("probe_proxmox_guest_network", {
              request: { temporaryDhcpIp: ip, sshKeyPath: form.proxmoxSshKeyPath },
            })
          }
          onSubmit={(values) => {
            setForm((current) => ({
              ...current,
              proxmoxTemporaryDhcpIp: values.temporaryDhcpIp,
              staticIp: values.staticIp,
              gateway: values.gateway,
              dns: values.dns,
              playerIp: current.playerIpMode === "local" ? values.staticIp : current.playerIp,
            }));
            void startSetup(values);
          }}
        />
        <UpdateDialog
          open={updateDialogOpen}
          update={availableUpdate}
          status={updateStatus}
          progress={updateProgress}
          onOpenChange={setUpdateDialogOpen}
          onInstall={() => void installAppUpdate()}
        />
        <ServerUpdateConfirmDialog
          pending={pendingServerUpdate}
          onOpenChange={(open) => {
            if (!open) setPendingServerUpdate(null);
          }}
          onConfirm={confirmPendingServerUpdate}
        />
        <RemoteAttachDialog
          open={remoteAttachOpen}
          form={remoteAttachForm}
          running={remoteAttachRunning}
          onOpenChange={setRemoteAttachOpen}
          onChange={setRemoteAttachForm}
          onAttach={() => void attachRemoteServer()}
        />
        <LocalHyperVAttachDialog
          open={localAttachOpen}
          form={localAttachForm}
          running={localAttachRunning}
          onOpenChange={setLocalAttachOpen}
          onChange={setLocalAttachForm}
          onAttach={() => void attachLocalHyperVServer()}
        />
        <RemoveRemoteServerDialog
          server={remoteServerToRemove}
          onOpenChange={(open) => {
            if (!open) setRemoteServerToRemove(null);
          }}
          onRemove={removeRemoteServer}
        />
      </Flex>
    </Theme>
  );
}

type SetupLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
};
