export type SetupSelections = {
  steamcmdPath: string;
  steamcmdInstallDir: string;
  serverInstallDir: string;
  vmDestinationPath: string;
  vmSwitchName: string;
  physicalAdapterName: string;
  memoryGb: number;
  vmIpMode: string;
  staticIp: string;
  staticCidr: string;
  staticGateway: string;
  staticDns: string;
  playerIpMode: string;
  manualPlayerIp: string;
  worldName: string;
  worldRegion: string;
  bootstrapProfileId: string;
};

export type SetupPersistedState = {
  currentStage: string;
  completedStages: string[];
  lastError: string;
  logPath: string;
  selections: SetupSelections;
};

export type SteamCmdDetection = {
  found: boolean;
  path: string;
  candidates: string[];
};

export type SetupState = {
  persisted: SetupPersistedState;
  steamcmd: SteamCmdDetection;
  suggestedSteamcmdInstallDir: string;
  suggestedServerInstallDir: string;
  serverInstalled: boolean;
  serverInstallPath: string;
  vmExists: boolean;
  vmState: string;
  vmIp: string;
  elevated: boolean;
  hypervAvailable: boolean;
  vmmsRunning: boolean;
};

export type DriveOption = {
  name: string;
  root: string;
  freeGb: number;
};

export type NetworkAdapterOption = {
  name: string;
  interfaceDescription: string;
  ipv4Address: string;
  prefixLength: number;
  cidr: string;
  gateway: string;
  boundSwitchName: string;
};

export type VmSwitchOption = {
  name: string;
  switchType: string;
  netAdapterInterfaceDescription: string;
};

export type VmImportOptions = {
  vmcxPath: string;
  existingVm: boolean;
  existingVmState: string;
  drives: DriveOption[];
  networkAdapters: NetworkAdapterOption[];
  switches: VmSwitchOption[];
  suggestedDestination: string;
};

export type VmDestinationStatus = {
  exists: boolean;
  isEmpty: boolean;
};

export type SetupCommandResult = {
  ok: boolean;
  stage: string;
  message: string;
  stdout: string;
};

export type GuestBootstrapRequest = {
  installPath: string;
  ip: string;
  playerIp: string;
  staticIp: string;
  staticCidr: string;
  staticGateway: string;
  staticDns: string;
  worldName: string;
  region: string;
  selfHostToken: string;
  profileId: string;
};
