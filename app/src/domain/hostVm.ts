export type HostStatus = {
  user: string;
  isElevated: boolean;
  hypervAvailable: boolean;
  vmmsStatus?: string | null;
  sshAvailable: boolean;
  defaultInstallPathExists: boolean;
  defaultInstallPath: string;
};

export type AppConfig = {
  installPath: string;
  vmName: string;
  vmIp: string;
  steamcmdPath: string;
  sshUser: string;
  sshPath: string;
  managerApiUrl: string;
  managerApiToken: string;
  managerApiNamespace: string;
  managerApiImage: string;
  managerApiBinaryPath: string;
  managerApiDirectorUrl: string;
};

export type VmStatus = {
  name: string;
  state: string;
  status: string;
  memoryAssignedBytes: number;
  uptime: string;
  path: string;
  configurationLocation: string;
  ipAddresses: string[];
};

export type GuestConnection = {
  ip: string;
  sshUser: string;
  keyPath: string;
  connected: boolean;
  sudo: boolean;
  hostname: string;
  kernel: string;
  kubectl: boolean;
};
