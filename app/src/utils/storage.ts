import {
  type RemoteServerRecord,
  type RemoteServerProfile,
  type LocalServerProfile,
  type DuneVmCandidate,
  type RemoteServerComponent,
  type TunnelService,
  type ProxmoxProvisioner,
  type RemoteBattlegroupStatus,
  type SetupRunRequest,
  type RollbackRequest,
  type VmInventoryRecord,
  type SetupForm,
  type SetupLayoutPreview,
  type ProxmoxAlpineSetupResult,
  type RemoteSetupRunResult,
  type UbuntuSshPreflight,
  type RemoteSetupRunRequest
} from "../types";
import { parsePositiveInt } from "./helpers";
import { effectiveProcessorCount } from "./memory";

export const remoteProfileStorageKey = "dune-manager.remote-ubuntu-profile";
export const proxmoxProfileStorageKey = "dune-manager.proxmox-profile";
export const remoteServersStorageKey = "dune-manager.remote-servers";
export const localServersStorageKey = "dune-manager.local-hyperv-servers";

export function localServerKey(server: DuneVmCandidate): string {
  return `hyperv:${server.vm.name}`;
}

export function componentLogStateKey(serverKey: string, component: RemoteServerComponent): string {
  return `${serverKey}:${component.logKey}`;
}

export function serverTunnelKey(serverKey: string, service: TunnelService): string {
  return `${serverKey}:tunnel:${service}`;
}

export function tunnelServiceLabel(service: TunnelService): string {
  if (service === "director") {
    return "Director UI";
  }
  if (service === "database") {
    return "Postgres";
  }
  if (service === "pgHero") {
    return "PgHero";
  }
  return "File Browser";
}

export function isCriticalRestartComponent(component: RemoteServerComponent): boolean {
  return ["database", "message-queue", "server-group"].includes(component.logKey);
}

export async function copyTextToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
}

export function upsertLocalServer(servers: DuneVmCandidate[], record: DuneVmCandidate): DuneVmCandidate[] {
  const index = servers.findIndex((server) => server.vm.name.toLowerCase() === record.vm.name.toLowerCase());
  if (index === -1) {
    return [...servers, record];
  }
  const next = [...servers];
  next[index] = mergeLocalServerAddress(next[index], record);
  return next;
}

export function primaryLocalServerIp(server: DuneVmCandidate): string {
  return server.vm.ipv4Addresses[0] ?? "";
}

export function mergeLocalServerAddress(existing: DuneVmCandidate, record: DuneVmCandidate): DuneVmCandidate {
  if (record.vm.ipv4Addresses.length > 0 || existing.vm.ipv4Addresses.length === 0) {
    return record;
  }
  return {
    ...record,
    vm: {
      ...record.vm,
      ipv4Addresses: existing.vm.ipv4Addresses,
    },
  };
}

export function uniqueBy<T>(values: T[], keyOf: (value: T) => string): T[] {
  const seen = new Set<string>();
  const unique: T[] = [];
  for (const value of values) {
    const key = keyOf(value);
    if (seen.has(key)) continue;
    seen.add(key);
    unique.push(value);
  }
  return unique;
}

export function remoteServerId(type: string, host: string, keyPath = ""): string {
  const normalizedHost = host.trim().toLowerCase();
  return `${type === "alpine" ? "alpine" : "ubuntu"}:${normalizedHost}:${keyPath.trim().toLowerCase()}`;
}

export function remoteServerDefaultUser(type: string): string {
  return type === "alpine" ? "dune" : "root";
}

export function remoteServerKindLabel(type: string): string {
  return type === "alpine" ? "Alpine VM" : "Ubuntu";
}

export function remoteServerPlaceholder(
  profile: RemoteServerProfile,
  name?: string,
  phase = "Retrieving",
): RemoteServerRecord {
  return {
    type: profile.type,
    id: remoteServerId(profile.type, profile.host, profile.keyPath || ""),
    name: name || profile.host || remoteServerKindLabel(profile.type),
    host: profile.host,
    user: remoteServerDefaultUser(profile.type),
    keyPath: profile.keyPath || "",
    namespace: "",
    battlegroupName: "",
    worldUniqueName: "",
    phase,
    createdAt: profile.createdAt,
    provisioner: profile.provisioner,
  };
}

export function isProxmoxProvisioner(value: unknown): value is ProxmoxProvisioner {
  if (!value || typeof value !== "object") return false;
  const provisioner = value as Partial<ProxmoxProvisioner>;
  return (
    provisioner.type === "proxmox" &&
    typeof provisioner.hostUrl === "string" &&
    typeof provisioner.tokenId === "string" &&
    typeof provisioner.node === "string" &&
    typeof provisioner.vmid === "number"
  );
}

export function remoteServerProfileFromStored(value: unknown): RemoteServerProfile | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Partial<RemoteServerProfile & RemoteServerRecord>;
  if (typeof record.host !== "string") return null;
  const type = record.type === "alpine" ? "alpine" : "ubuntu";
  if (typeof record.keyPath !== "string") return null;
  return {
    type,
    host: record.host,
    keyPath: typeof record.keyPath === "string" ? record.keyPath : "",
    createdAt: typeof record.createdAt === "string" ? record.createdAt : new Date().toISOString(),
    provisioner: isProxmoxProvisioner(record.provisioner) ? record.provisioner : undefined,
  };
}

export function readRemoteServers(): RemoteServerRecord[] {
  const text = window.localStorage.getItem(remoteServersStorageKey);
  if (!text) return [];
  try {
    const value = JSON.parse(text);
    if (!Array.isArray(value)) return [];
    return value
      .map(remoteServerProfileFromStored)
      .filter((profile): profile is RemoteServerProfile => !!profile)
      .map((profile) => remoteServerPlaceholder(profile));
  } catch {
    window.localStorage.removeItem(remoteServersStorageKey);
    return [];
  }
}

export function persistRemoteServers(servers: RemoteServerRecord[]): RemoteServerRecord[] {
  const profiles = uniqueBy(
    servers
      .filter((server) => server.host.trim() && server.keyPath.trim())
      .map((server): RemoteServerProfile => ({
        type: server.type,
        host: server.host,
        keyPath: server.keyPath,
        createdAt: server.createdAt || new Date().toISOString(),
        provisioner: server.provisioner,
      })),
    (profile) => remoteServerId(profile.type, profile.host, profile.keyPath || ""),
  );
  window.localStorage.setItem(remoteServersStorageKey, JSON.stringify(profiles));
  return servers;
}

export function localServerProfileFromStored(value: unknown): LocalServerProfile | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Partial<LocalServerProfile & DuneVmCandidate>;
  const vmName =
    typeof record.vmName === "string"
      ? record.vmName
      : typeof record.vm?.name === "string"
        ? record.vm.name
        : "";
  if (!vmName.trim()) return null;
  return {
    type: "hyperv",
    vmName,
    staticIp: typeof record.staticIp === "string" ? record.staticIp : "",
    createdAt: typeof record.createdAt === "string" ? record.createdAt : new Date().toISOString(),
  };
}

export function localServerPlaceholder(vmName: string, staticIp = ""): DuneVmCandidate {
  return {
    confidence: "low",
    reasons: ["saved profile"],
    vm: {
      name: vmName,
      state: "other",
      rawState: "Retrieving",
      configurationLocation: "",
      path: "",
      memoryAssignedBytes: 0,
      processorCount: 0,
      uptimeSeconds: 0,
      ipv4Addresses: staticIp.trim() ? [staticIp.trim()] : [],
      hardDiskPaths: [],
      diskSizeBytes: 0,
      diskFileSizeBytes: 0,
      switchNames: [],
    },
  };
}

export function readLocalServers(): DuneVmCandidate[] {
  const text = window.localStorage.getItem(localServersStorageKey);
  if (!text) return [];
  try {
    const value = JSON.parse(text);
    if (!Array.isArray(value)) return [];
    return value
      .map(localServerProfileFromStored)
      .filter((profile): profile is LocalServerProfile => !!profile)
      .map((profile) => localServerPlaceholder(profile.vmName, profile.staticIp));
  } catch {
    window.localStorage.removeItem(localServersStorageKey);
    return [];
  }
}

export function persistLocalServers(servers: DuneVmCandidate[]): DuneVmCandidate[] {
  const profiles = uniqueBy(
    servers
      .filter((server) => server.vm.name.trim())
      .map((server): LocalServerProfile => ({
        type: "hyperv",
        vmName: server.vm.name,
        staticIp: primaryLocalServerIp(server),
        createdAt: new Date().toISOString(),
      })),
    (profile) => profile.vmName.trim().toLowerCase(),
  );
  window.localStorage.setItem(localServersStorageKey, JSON.stringify(profiles));
  return servers;
}

export function persistProxmoxProfile(profile: {
  hostUrl: string;
  tokenId: string;
  acceptedCertificateSha256?: string;
}) {
  window.localStorage.setItem(
    proxmoxProfileStorageKey,
    JSON.stringify({
      proxmoxHostUrl: profile.hostUrl,
      proxmoxTokenId: profile.tokenId,
      proxmoxAcceptedCertificateSha256: profile.acceptedCertificateSha256 || "",
    }),
  );
}

export function isRemoteServerRecord(value: unknown): value is RemoteServerRecord {
  if (!value || typeof value !== "object") return false;
  const record = value as Partial<RemoteServerRecord>;
  return typeof record.id === "string" && typeof record.host === "string" && typeof record.name === "string";
}

export function isDuneVmCandidate(value: unknown): value is DuneVmCandidate {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<DuneVmCandidate>;
  const vm = candidate.vm as Partial<VmInventoryRecord> | undefined;
  return (
    !!vm &&
    typeof vm.name === "string" &&
    typeof vm.rawState === "string" &&
    typeof vm.state === "string" &&
    Array.isArray(vm.ipv4Addresses) &&
    Array.isArray(vm.hardDiskPaths) &&
    Array.isArray(vm.switchNames)
  );
}

export function isStartedPhase(phase: string): boolean {
  return ["running", "ready", "healthy", "available", "reconciling"].includes(
    phase.trim().toLowerCase(),
  );
}

export function isDirectorReadyPhase(phase: string): boolean {
  const normalized = phase.trim().toLowerCase();
  return normalized.length === 0 || isStartedPhase(normalized);
}

export function isBattlegroupStarted(status: RemoteBattlegroupStatus): boolean {
  return (
    !status.stop &&
    isStartedPhase(status.phase) &&
    isStartedPhase(status.serverGroupPhase) &&
    isDirectorReadyPhase(status.directorPhase)
  );
}

export function omitKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _removed, ...rest } = record;
  return rest;
}

export function omitPrefix<T>(record: Record<string, T>, prefix: string): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([key]) => !key.startsWith(prefix)));
}

export function rollbackRequestFromSetup(request: SetupRunRequest): RollbackRequest {
  return {
    vmName: request.vmName,
    vmDestination: request.vmDestination,
    switchName: request.switchName,
  };
}

export function upsertRemoteServer(servers: RemoteServerRecord[], record: RemoteServerRecord): RemoteServerRecord[] {
  const index = servers.findIndex((server) => server.id === record.id);
  if (index === -1) {
    return [...servers, record];
  }
  const next = [...servers];
  next[index] = { ...next[index], ...record };
  return next;
}

export function remoteServerFromDetected(existing: RemoteServerRecord, detected: RemoteServerRecord): RemoteServerRecord {
  return {
    ...detected,
    type: existing.type,
    id: existing.id,
    host: existing.host,
    keyPath: existing.keyPath,
    user: existing.user || remoteServerDefaultUser(existing.type),
    createdAt: existing.createdAt,
    provisioner: existing.provisioner,
  };
}

export function remoteServerDraftFromForm(form: SetupForm): RemoteServerRecord {
  const host = form.remoteHost.trim();
  return remoteServerPlaceholder({
    type: "ubuntu",
    host,
    keyPath: form.remoteKeyPath.trim(),
    createdAt: new Date().toISOString(),
  }, form.worldName.trim() || undefined, "Setup running");
}

export function proxmoxProvisionerFromForm(form: SetupForm, result?: Partial<ProxmoxAlpineSetupResult>): ProxmoxProvisioner {
  return {
    type: "proxmox",
    profileId: proxmoxProfileId(form),
    hostUrl: form.proxmoxHostUrl.trim(),
    tokenId: form.proxmoxTokenId.trim(),
    acceptedCertificateSha256: form.proxmoxAcceptedCertificateSha256.trim() || undefined,
    node: result?.node || form.proxmoxNode.trim(),
    vmid: result?.vmid || parsePositiveInt(form.proxmoxVmid),
    vmName: result?.vmName || form.vmName.trim(),
  };
}

export function proxmoxServerDraftFromForm(form: SetupForm): RemoteServerRecord {
  const host = form.staticIp.trim() || form.proxmoxTemporaryDhcpIp.trim();
  return remoteServerPlaceholder({
    type: "alpine",
    host,
    keyPath: form.proxmoxSshKeyPath.trim(),
    createdAt: new Date().toISOString(),
    provisioner: proxmoxProvisionerFromForm(form),
  }, form.worldName.trim() || undefined, "Setup running");
}

export function proxmoxServerRecordFromSetup(
  form: SetupForm,
  result: ProxmoxAlpineSetupResult,
  existingId?: string,
): RemoteServerRecord {
  const profile = remoteServerPlaceholder({
    type: "alpine",
    host: result.host,
    keyPath: form.proxmoxSshKeyPath.trim() || result.keyPath,
    createdAt: new Date().toISOString(),
    provisioner: proxmoxProvisionerFromForm(form, result),
  });
  return {
    ...profile,
    id: existingId || profile.id,
    name: form.worldName.trim() || result.battlegroupName,
    host: result.host,
    user: result.user || "dune",
    keyPath: form.proxmoxSshKeyPath.trim() || result.keyPath,
    namespace: result.namespace,
    battlegroupName: result.battlegroupName,
    worldUniqueName: result.worldUniqueName,
    phase: "Ready",
  };
}

export function remoteServerRecordFromSetup(
  form: SetupForm,
  result: { namespace: string; battlegroupName: string; worldUniqueName: string },
  existingId?: string,
): RemoteServerRecord {
  const host = form.remoteHost.trim();
  const profile = remoteServerPlaceholder({
    type: "ubuntu",
    host,
    keyPath: form.remoteKeyPath.trim(),
    createdAt: new Date().toISOString(),
  });
  return {
    ...profile,
    id: existingId || profile.id,
    name: form.worldName.trim() || result.battlegroupName,
    namespace: result.namespace,
    battlegroupName: result.battlegroupName,
    worldUniqueName: result.worldUniqueName,
    phase: "Ready",
  };
}

export function remoteServerActionRequest(server: RemoteServerRecord) {
  return {
    serverType: server.type,
    host: server.host,
    user: server.user || remoteServerDefaultUser(server.type),
    keyPath: server.keyPath || undefined,
    namespace: server.namespace,
    battlegroupName: server.battlegroupName,
  };
}

export function proxmoxProfileId(form: Pick<SetupForm, "proxmoxHostUrl" | "proxmoxTokenId">): string {
  return `${form.proxmoxHostUrl.trim().toLowerCase()}|${form.proxmoxTokenId.trim().toLowerCase()}`;
}

export function proxmoxConnectionRequest(form: SetupForm) {
  return {
    profileId: proxmoxProfileId(form),
    hostUrl: form.proxmoxHostUrl.trim(),
    tokenId: form.proxmoxTokenId.trim(),
    tokenSecret: form.proxmoxTokenSecret.trim() || undefined,
    acceptedCertificateSha256: form.proxmoxAcceptedCertificateSha256.trim() || undefined,
  };
}

export function proxmoxVmActionRequest(provisioner: ProxmoxProvisioner) {
  return {
    profileId: provisioner.profileId,
    hostUrl: provisioner.hostUrl,
    tokenId: provisioner.tokenId,
    acceptedCertificateSha256: provisioner.acceptedCertificateSha256,
    node: provisioner.node,
    vmid: provisioner.vmid,
  };
}

export function proxmoxSetupRunRequest(form: SetupForm, memoryGb: number) {
  return {
    ...proxmoxConnectionRequest(form),
    node: form.proxmoxNode.trim(),
    vmStorage: form.proxmoxVmStorage.trim(),
    importStorage: form.proxmoxImportStorage.trim(),
    bridge: form.proxmoxBridge.trim(),
    bridgeCidr: form.proxmoxBridgeCidr.trim() || undefined,
    vmid: parsePositiveInt(form.proxmoxVmid),
    vmName: form.vmName.trim(),
    diskGb: Math.max(1, parsePositiveInt(form.diskGb)),
    memoryGb,
    processorCount: effectiveProcessorCount(form),
    staticIp: form.staticIp.trim(),
    gateway: form.gateway.trim(),
    dns: form.dns.trim(),
    temporaryDhcpIp: form.proxmoxTemporaryDhcpIp.trim() || undefined,
    sshKeyPath: form.proxmoxSshKeyPath.trim(),
    playerIp: form.playerIp.trim(),
    worldName: form.worldName,
    region: form.region,
    selfHostToken: form.tokenSource,
    survivalInstances: Math.max(1, parsePositiveInt(form.survivalInstances)),
    deepDesertPveInstances: parsePositiveInt(form.deepDesertPveInstances),
    deepDesertPvpInstances: parsePositiveInt(form.deepDesertPvpInstances),
    deepDesertWarmServers: parsePositiveInt(form.deepDesertWarmServers),
    enableSwap: form.enableSwap,
    installQemuGuestAgent: form.proxmoxInstallQemuGuestAgent,
  };
}

export function remoteSetupRunRequest(form: SetupForm): RemoteSetupRunRequest {
  return {
    host: form.remoteHost.trim(),
    user: form.remoteUser.trim() || "root",
    keyPath: form.remoteKeyPath.trim(),
    playerIp: form.playerIp.trim(),
    worldName: form.worldName,
    region: form.region,
    selfHostToken: form.tokenSource,
    survivalInstances: Math.max(1, parsePositiveInt(form.survivalInstances)),
    deepDesertPveInstances: parsePositiveInt(form.deepDesertPveInstances),
    deepDesertPvpInstances: parsePositiveInt(form.deepDesertPvpInstances),
    deepDesertWarmServers: parsePositiveInt(form.deepDesertWarmServers),
    enableSwap: form.enableSwap,
  };
}

export function setupRunRequest(form: SetupForm, memoryGb: number): SetupRunRequest {
  return {
    vmDestination: form.vmDestination,
    vmName: form.vmName,
    diskGb: parsePositiveInt(form.diskGb),
    memoryGb,
    processorCount: effectiveProcessorCount(form),
    enableSwap: false,
    networkMode: form.networkMode,
    switchName: form.switchName,
    adapterName: form.adapterName,
    staticIp: form.staticIp,
    gateway: form.gateway,
    dns: form.dns,
    playerIp: form.playerIp,
    worldName: form.worldName,
    region: form.region,
    selfHostToken: form.tokenSource,
    survivalInstances: Math.max(1, parsePositiveInt(form.survivalInstances)),
    deepDesertPveInstances: parsePositiveInt(form.deepDesertPveInstances),
    deepDesertPvpInstances: parsePositiveInt(form.deepDesertPvpInstances),
    deepDesertWarmServers: parsePositiveInt(form.deepDesertWarmServers),
  };
}

export function setupLayoutPreview(form: SetupForm): SetupLayoutPreview {
  const survivalInstances = Math.max(1, parsePositiveInt(form.survivalInstances));
  const deepDesertPve = parsePositiveInt(form.deepDesertPveInstances);
  const deepDesertPvp = parsePositiveInt(form.deepDesertPvpInstances);
  const deepDesertTotal = deepDesertPve + deepDesertPvp;
  const survivalDimensions = Array.from({ length: survivalInstances }, (_, index) => index).join(", ");

  return {
    survivalDimensions,
    deepDesertTotal,
    deepDesertPvp,
  };
}
