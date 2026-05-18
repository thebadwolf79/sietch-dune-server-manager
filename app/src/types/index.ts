import { type ReactNode } from "react";
import { type Update } from "@tauri-apps/plugin-updater";

export const pages = [
  { id: "servers", label: "Servers" },
  { id: "install", label: "Create New Server" },
  { id: "tools", label: "Tools" },
] as const;

export type PageId = (typeof pages)[number]["id"];

export type NetworkMode = "static" | "dhcp";
export type PlayerIpMode = "local" | "external";
export type SetupTarget = "hyperv" | "ubuntu" | "proxmox";
export type RemoteServerKind = "ubuntu" | "alpine";

export type NetworkAdapterCandidate = {
  name: string;
  interfaceDescription: string;
  ipv4Address: string;
  prefixLength: number;
  gateway: string;
  suggestedIpv4Address: string;
  existingExternalSwitch: string;
};

export type HostReadiness = {
  elevated: boolean;
  hypervAvailable: boolean;
  vmmsRunning: boolean;
  virtualizationFirmwareEnabled: boolean | null;
  totalPhysicalMemoryBytes: number;
  availablePhysicalMemoryBytes: number;
  logicalProcessorCount: number;
};

export type DriveCandidate = {
  name: string;
  root: string;
  freeBytes: number;
};

export type EnvironmentDetection = {
  readiness: HostReadiness;
  drives: DriveCandidate[];
  networkAdapters: NetworkAdapterCandidate[];
  externalIp: string | null;
};

export type VmPowerState =
  | "missing"
  | "off"
  | "starting"
  | "running"
  | "stopping"
  | "saved"
  | "paused"
  | "other";

export type VmInventoryRecord = {
  name: string;
  state: VmPowerState;
  rawState: string;
  configurationLocation: string;
  path: string;
  memoryAssignedBytes: number;
  processorCount: number;
  uptimeSeconds: number;
  ipv4Addresses: string[];
  hardDiskPaths: string[];
  diskSizeBytes: number;
  diskFileSizeBytes: number;
  switchNames: string[];
};

export type DuneVmCandidate = {
  vm: VmInventoryRecord;
  confidence: "high" | "medium" | "low";
  reasons: string[];
};

export type DetectionState = "idle" | "detecting" | "ready" | "failed";
export type LogLevel = "debug" | "info" | "warn" | "error";
export type LogLevelFilter = LogLevel;
export type UpdateStatus = "idle" | "checking" | "available" | "current" | "installing" | "relaunching" | "failed";
export type ServerPackageCheckStatus = "idle" | "checking" | "current" | "available" | "missing" | "updating" | "failed";

export type LogRow = {
  id: number;
  timestamp: string;
  level: LogLevel;
  scope: string;
  message: string;
};

export type AppErrorBoundaryProps = {
  onError: (message: string) => void;
  children: ReactNode;
};

export type AppErrorBoundaryState = {
  error: string | null;
};

export type EnvironmentGate = {
  canContinue: boolean;
  reasons: string[];
};

export type SetupLogPayload = {
  level: LogLevel;
  scope: string;
  message: string;
};

export type ServerPackageStatus = {
  packageDir: string;
  appId: string;
  installedBuildId?: string | null;
  latestBuildId?: string | null;
  updateAvailable: boolean;
  complete: boolean;
  layout?: "legacyInternalScripts" | "battlegroupManagement" | null;
  message: string;
};

export type SetupRunRequest = {
  vmDestination: string;
  vmName: string;
  diskGb: number;
  memoryGb: number;
  processorCount: number;
  enableSwap: boolean;
  networkMode: NetworkMode;
  switchName: string;
  adapterName: string;
  staticIp: string;
  gateway: string;
  dns: string;
  playerIp: string;
  worldName: string;
  region: string;
  selfHostToken: string;
  survivalInstances: number;
  deepDesertPveInstances: number;
  deepDesertPvpInstances: number;
  deepDesertWarmServers: number;
};

export type RemoteSetupRunRequest = {
  host: string;
  user: string;
  keyPath: string;
  playerIp: string;
  worldName: string;
  region: string;
  selfHostToken: string;
  survivalInstances: number;
  deepDesertPveInstances: number;
  deepDesertPvpInstances: number;
  deepDesertWarmServers: number;
  enableSwap: boolean;
};

export type RemoteSetupRunResult = {
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  preflight: UbuntuSshPreflight;
};

export type ProxmoxNode = {
  node: string;
  status: string;
  cpu: number;
  maxcpu: number;
  mem: number;
  maxmem: number;
};

export type ProxmoxStorage = {
  storage: string;
  type: string;
  content: string;
  active: number;
  shared: number;
  avail: number;
  total: number;
};

export type ProxmoxBridge = {
  iface: string;
  type: string;
  active: number;
  cidr?: string | null;
  autostart: number;
};

export type ProxmoxDetection = {
  version: { version: string; release: string; repoid: string };
  certificateSha256: string;
  certificateTrusted: boolean;
  nodes: ProxmoxNode[];
  storages: ProxmoxStorage[];
  bridges: ProxmoxBridge[];
  nextVmid: number;
};

export type ProxmoxProvisioner = {
  type: "proxmox";
  profileId: string;
  hostUrl: string;
  tokenId: string;
  acceptedCertificateSha256?: string;
  node: string;
  vmid: number;
  vmName: string;
};

export type ProxmoxAlpineSetupResult = {
  host: string;
  user: string;
  keyPath: string;
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  node: string;
  vmid: number;
  vmName: string;
};

export type ProxmoxVmStatus = {
  status: string;
  name: string;
  pid?: number | null;
  maxmem: number;
  cpus: number;
};

export type SetupRunResult = {
  vmName: string;
  namespace: string;
  battlegroupName: string;
  worldUniqueName: string;
  directorNodePort: number | null;
};

export type GenerateSshKeyResult = {
  privateKeyPath: string;
  publicKeyPath: string;
  publicKey: string;
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
  createdAt: string;
  provisioner?: ProxmoxProvisioner;
};

export type RemoteServerProfile = {
  type: RemoteServerKind;
  host: string;
  keyPath?: string;
  createdAt: string;
  provisioner?: ProxmoxProvisioner;
};

export type LocalServerProfile = {
  type: "hyperv";
  vmName: string;
  staticIp: string;
  createdAt: string;
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

export type RemoteServerPackageStatus = {
  installedBuildId?: string | null;
  battlegroupVersion?: string | null;
  liveBattlegroupVersion?: string | null;
  operatorVersion?: string | null;
};

export type PendingServerUpdate =
  | { type: "remote"; server: RemoteServerRecord }
  | { type: "local"; server: DuneVmCandidate };

export type LocalHyperVRuntime = {
  namespace: string;
  battlegroupName: string;
  status: RemoteServerStatus;
  components: RemoteServerComponent[];
};

export type RemoteServerComponent = {
  name: string;
  logKey: string;
  category: "system" | "map";
  state: string;
  tone: "green" | "amber" | "red" | "gray";
  summary: string;
  details: string[];
};

export type RemoteComponentLogResult = {
  component: string;
  output: string;
};

export type TunnelService = "director" | "fileBrowser" | "database" | "pgHero";

export type ServerTunnelStatus = {
  tunnelId: string;
  service: TunnelService;
  localPort: number;
  remotePort: number;
  url: string;
};

export type ServerTunnelStartRequest = {
  tunnelId: string;
  serverKind: "hyperv" | "ubuntu" | "alpine";
  service: TunnelService;
  host: string;
  user?: string;
  keyPath?: string;
  vmName?: string;
  namespace: string;
};

export type RemoteComponentRestartResult = {
  component: string;
  output: string;
};

export type RemoteAttachForm = {
  type: RemoteServerKind;
  host: string;
  keyPath: string;
};

export type LocalHyperVAttachForm = {
  vmName: string;
  staticIp: string;
};

export type UbuntuSshPreflight = {
  hostname: string;
  osPrettyName: string;
  osId: string;
  versionId: string;
  architecture: string;
  kernelRelease: string;
  user: string;
  uid: number;
  passwordlessSudo: boolean;
  systemdAvailable: boolean;
  logicalProcessorCount: number;
  totalMemoryBytes: number;
  availableMemoryBytes: number;
  swapTotalBytes: number;
  rootDiskTotalBytes: number;
  rootDiskAvailableBytes: number;
  publicIp: string | null;
  ipv4Addresses: string[];
  steamcmdInstalled: boolean;
  k3sInstalled: boolean;
  kubectlAvailable: boolean;
};

export type RollbackRequest = {
  vmName: string;
  vmDestination: string;
  switchName: string;
};

export type SetupForm = {
  setupTarget: SetupTarget;
  vmDestination: string;
  vmName: string;
  diskGb: string;
  vmMemoryGb: string;
  processorCount: string;
  enableSwap: boolean;
  networkMode: NetworkMode;
  switchName: string;
  adapterName: string;
  staticIp: string;
  gateway: string;
  dns: string;
  playerIpMode: PlayerIpMode;
  playerIp: string;
  worldName: string;
  region: string;
  tokenSource: string;
  survivalInstances: string;
  includeSocial: boolean;
  deepDesertPveInstances: string;
  deepDesertPvpInstances: string;
  deepDesertWarmServers: string;
  remoteHost: string;
  remoteUser: string;
  remoteKeyPath: string;
  proxmoxHostUrl: string;
  proxmoxTokenId: string;
  proxmoxTokenSecret: string;
  proxmoxAcceptedCertificateSha256: string;
  proxmoxNode: string;
  proxmoxVmStorage: string;
  proxmoxImportStorage: string;
  proxmoxBridge: string;
  proxmoxBridgeCidr: string;
  proxmoxVmid: string;
  proxmoxInstallQemuGuestAgent: boolean;
  saveLocalServer: boolean;
  saveRemoteServer: boolean;
};

export type CalculatedMemory = {
  gb: number;
  lines: string[];
};

export type SetupLayoutPreview = {
  survivalDimensions: string;
  deepDesertTotal: number;
  deepDesertPvp: number;
};

export type SetupRequirements = {
  canContinue: boolean;
  memoryOk: boolean;
  processorOk: boolean;
  diskOk: boolean;
  memoryRequired: string;
  memoryAvailable: string;
  processorRequired: string;
  processorAvailable: string;
  diskRequired: string;
  diskAvailable: string;
};
