import {
  type DetectionState,
  type HostReadiness,
  type NetworkAdapterCandidate,
  type EnvironmentGate,
  type CalculatedMemory,
  type DriveCandidate,
  type SetupRequirements,
  type SetupForm,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type SetupTarget
} from "../types";
import { parsePositiveInt, formatGiB, formatGiBFloor1, log } from "./helpers";
import { deepDesertInstanceCount, minimumVmMemoryGb, recommendedUbuntuSwapGb } from "./memory";

export function setupEnvironmentGate(
  status: DetectionState,
  readiness: HostReadiness | null,
  adapters: NetworkAdapterCandidate[],
): EnvironmentGate {
  const reasons: string[] = [];
  if (status !== "ready") {
    reasons.push("Environment detection has not completed.");
  }
  if (!readiness) {
    reasons.push("Host readiness was not detected.");
  } else {
    if (!readiness.elevated) {
      reasons.push("Restart the app as administrator to continue setup.");
    }
    if (readiness.virtualizationFirmwareEnabled === false) {
      reasons.push("Hyper-V virtualization support is not operational.");
    }
    if (!readiness.hypervAvailable) {
      reasons.push("Hyper-V PowerShell support is missing.");
    }
    if (!readiness.vmmsRunning) {
      reasons.push("Hyper-V vmms service is not running.");
    }
  }
  if (adapters.length === 0) {
    reasons.push("A physical network adapter with IPv4 and gateway is required.");
  }
  return {
    canContinue: reasons.length === 0,
    reasons,
  };
}

export function findDriveForPath(path: string, drives: DriveCandidate[]): DriveCandidate | null {
  const normalizedPath = path.trim().replace(/\//g, "\\").toUpperCase();
  if (!/^[A-Z]:\\/.test(normalizedPath)) {
    return null;
  }

  return (
    drives.find((drive) => {
      const root = drive.root.trim().replace(/\//g, "\\").toUpperCase();
      return normalizedPath.startsWith(root);
    }) ?? null
  );
}

export function setupRequirementStatus(
  calculatedMemory: CalculatedMemory,
  vmMemoryGb: string,
  diskGb: string,
  processorCount: string,
  vmDestination: string,
  readiness: HostReadiness | null,
  drives: DriveCandidate[],
): SetupRequirements {
  const effectiveMemoryGb = Math.max(calculatedMemory.gb, parsePositiveInt(vmMemoryGb));
  const requiredMemoryBytes = effectiveMemoryGb * 1024 * 1024 * 1024;
  const requiredProcessors = Math.max(4, parsePositiveInt(processorCount));
  const requiredDiskGb = Math.max(0, parsePositiveInt(diskGb));
  const requiredDiskBytes = requiredDiskGb * 1024 * 1024 * 1024;
  const memoryAvailable = readiness?.availablePhysicalMemoryBytes ?? 0;
  const processorsAvailable = readiness?.logicalProcessorCount ?? 0;
  const memoryOk = memoryAvailable >= requiredMemoryBytes;
  const processorOk =
    requiredProcessors > 0 && (processorsAvailable === 0 || requiredProcessors <= processorsAvailable);
  const destinationDrive = findDriveForPath(vmDestination, drives);
  const diskOk = destinationDrive ? destinationDrive.freeBytes >= requiredDiskBytes : false;

  return {
    canContinue: memoryOk && processorOk && diskOk,
    memoryOk,
    processorOk,
    diskOk,
    memoryRequired: `${effectiveMemoryGb} GB required`,
    memoryAvailable: readiness ? `${formatGiB(memoryAvailable)} available` : "Run local detection",
    processorRequired: `${requiredProcessors || "A positive number of"} cores requested`,
    processorAvailable: readiness
      ? processorsAvailable
        ? `${processorsAvailable} logical available`
        : "Host CPU count unavailable"
      : "Run local detection",
    diskRequired: `${requiredDiskGb} GB required`,
    diskAvailable: destinationDrive
      ? `${destinationDrive.root} has ${formatGiB(destinationDrive.freeBytes)} free`
      : "Choose a VM destination folder",
  };
}

export function setupBlockingIssues(
  gate: EnvironmentGate,
  requirements: SetupRequirements,
  hasServiceToken: boolean,
  vmDestinationHasVm: boolean,
  form: SetupForm,
): string[] {
  const issues = [...gate.reasons];
  if (!requirements.memoryOk) {
    issues.push(`Memory: ${requirements.memoryRequired}; ${requirements.memoryAvailable}.`);
  }
  if (!requirements.processorOk) {
    issues.push(`CPU Cores: ${requirements.processorRequired}; ${requirements.processorAvailable}.`);
  }
  if (!requirements.diskOk) {
    issues.push(`VM Location: ${requirements.diskRequired}; ${requirements.diskAvailable}.`);
  }
  if (vmDestinationHasVm) {
    issues.push("VM Location already contains VM files. Choose another folder.");
  }
  if (parsePositiveInt(form.deepDesertWarmServers) > 0) {
    issues.push("Warm Deep Desert Instances are not supported yet; set them to 0 for this build.");
  }
  if (deepDesertInstanceCount(form) > 1) {
    issues.push("Only one Deep Desert instance is supported in this build.");
  }
  if (!hasServiceToken) {
    issues.push("Self-Host Service Token is required.");
  }
  return issues;
}

export function remoteSetupRequirementStatus(
  calculatedMemory: CalculatedMemory,
  diskGb: string,
  processorCount: string,
  preflight: UbuntuSshPreflight | null,
  enableSwap: boolean,
): SetupRequirements {
  const requiredMemoryBytes = calculatedMemory.gb * 1024 * 1024 * 1024;
  const requiredProcessors = Math.max(0, parsePositiveInt(processorCount));
  const requiredDiskGb = Math.max(0, parsePositiveInt(diskGb));
  const requiredDiskBytes = requiredDiskGb * 1024 * 1024 * 1024;
  const memoryAvailable = preflight?.availableMemoryBytes ?? 0;
  const existingSwapBytes = preflight?.swapTotalBytes ?? 0;
  const plannedSwapBytes = preflight && enableSwap ? recommendedUbuntuSwapGb(calculatedMemory, preflight) * 1024 * 1024 * 1024 : 0;
  const usableSwapBytes = Math.max(existingSwapBytes, plannedSwapBytes);
  const processorsAvailable = preflight?.logicalProcessorCount ?? 0;
  const diskAvailable = preflight?.rootDiskAvailableBytes ?? 0;
  const memoryOk = !!preflight && (memoryAvailable >= requiredMemoryBytes || memoryAvailable + usableSwapBytes >= requiredMemoryBytes);
  const memoryAvailableLabel =
    preflight && usableSwapBytes > 0
      ? `${formatGiB(memoryAvailable)} RAM available plus ${formatGiB(usableSwapBytes)} ${plannedSwapBytes > existingSwapBytes ? "planned" : "existing"} swap`
      : preflight
        ? `${formatGiB(memoryAvailable)} available`
        : "Run remote detection";

  return {
    canContinue:
      !!preflight &&
      memoryOk &&
      requiredProcessors > 0 &&
      requiredProcessors <= processorsAvailable &&
      diskAvailable >= requiredDiskBytes,
    memoryOk,
    processorOk: !!preflight && requiredProcessors > 0 && requiredProcessors <= processorsAvailable,
    diskOk: !!preflight && diskAvailable >= requiredDiskBytes,
    memoryRequired: `${calculatedMemory.gb} GB required`,
    memoryAvailable: memoryAvailableLabel,
    processorRequired: `${requiredProcessors || "A positive number of"} logical cores recommended`,
    processorAvailable: preflight ? `${processorsAvailable} logical available` : "Run remote detection",
    diskRequired: `${requiredDiskGb} GB free space required`,
    diskAvailable: preflight ? `${formatGiB(diskAvailable)} free on /` : "Run remote detection",
  };
}

export function remoteSetupBlockingIssues(
  requirements: SetupRequirements,
  hasServiceToken: boolean,
  form: SetupForm,
  preflight: UbuntuSshPreflight | null,
): string[] {
  const issues: string[] = [];
  if (!form.remoteHost.trim()) issues.push("Remote server IP is required.");
  if (form.remoteHost.includes(":")) issues.push("Use an IPv4 address for the remote Ubuntu server.");
  if (!form.remoteUser.trim()) issues.push("SSH user is required.");
  if (!form.remoteKeyPath.trim()) issues.push("SSH private key file is required.");
  if (!preflight) issues.push("Run remote resource detection before setup.");
  if (form.enableSwap && !preflight) issues.push("Run remote detection before enabling Ubuntu swap.");
  if (preflight && preflight.osId !== "ubuntu") issues.push("Remote host must be Ubuntu.");
  if (preflight && preflight.uid !== 0 && !preflight.passwordlessSudo) {
    issues.push("Remote setup requires root login or passwordless sudo.");
  }
  if (preflight && !preflight.systemdAvailable) issues.push("Remote host must support systemd.");
  if (!requirements.memoryOk) {
    issues.push(`Memory: ${requirements.memoryRequired}; ${requirements.memoryAvailable}.`);
  }
  if (!requirements.processorOk) {
    issues.push(`CPU Cores: ${requirements.processorRequired}; ${requirements.processorAvailable}.`);
  }
  if (!requirements.diskOk) {
    issues.push(`Disk: ${requirements.diskRequired}; ${requirements.diskAvailable}.`);
  }
  if (!form.playerIp.trim()) issues.push("Player-facing IP is required.");
  if (parsePositiveInt(form.deepDesertWarmServers) > 0) {
    issues.push("Warm Deep Desert Instances are not supported yet; set them to 0 for this build.");
  }
  if (deepDesertInstanceCount(form) > 1) {
    issues.push("Only one Deep Desert instance is supported in this build.");
  }
  if (!hasServiceToken) issues.push("Self-Host Service Token is required.");
  return issues;
}

export function proxmoxSetupRequirementStatus(
  calculatedMemory: CalculatedMemory,
  effectiveMemoryGb: number,
  diskGb: string,
  processorCount: string,
  nodeName: string,
  storageName: string,
  detection: ProxmoxDetection | null,
): SetupRequirements {
  const requiredMemoryBytes = effectiveMemoryGb * 1024 * 1024 * 1024;
  const requiredProcessors = Math.max(0, parsePositiveInt(processorCount));
  const requiredDiskGb = Math.max(0, parsePositiveInt(diskGb));
  const requiredDiskBytes = requiredDiskGb * 1024 * 1024 * 1024;
  const node = detection?.nodes.find((item) => item.node === nodeName.trim()) ?? detection?.nodes[0] ?? null;
  const storage =
    detection?.storages.find((item) => item.storage === storageName.trim()) ??
    detection?.storages.find((item) => item.content.includes("images")) ??
    detection?.storages[0] ??
    null;
  const availableMemoryBytes = node ? Math.max(0, node.maxmem - node.mem) : 0;
  const processorsAvailable = node?.maxcpu ?? 0;
  const storageAvailableBytes = storage?.avail ?? 0;
  const memoryOk = !!node && effectiveMemoryGb > 0 && availableMemoryBytes >= requiredMemoryBytes;
  const processorOk = !!node && requiredProcessors > 0 && requiredProcessors <= processorsAvailable;
  const diskOk = !!storage && requiredDiskGb > 0 && storageAvailableBytes >= requiredDiskBytes;
  const memoryRequired =
    effectiveMemoryGb < calculatedMemory.gb
      ? `${effectiveMemoryGb} GB required with swap profile (${calculatedMemory.gb} GB layout recommendation)`
      : `${effectiveMemoryGb} GB required`;

  return {
    canContinue: memoryOk && processorOk && diskOk,
    memoryOk,
    processorOk,
    diskOk,
    memoryRequired,
    memoryAvailable: node
      ? `${formatGiBFloor1(availableMemoryBytes)} available on ${node.node} (${formatGiBFloor1(node.maxmem)} total)`
      : "Run Proxmox detection",
    processorRequired: `${requiredProcessors || "A positive number of"} cores configured`,
    processorAvailable: node ? `${processorsAvailable} logical available on ${node.node}` : "Run Proxmox detection",
    diskRequired: `${requiredDiskGb} GB virtual disk`,
    diskAvailable: storage
      ? `${storage.storage} has ${formatGiBFloor1(storageAvailableBytes)} free`
      : "Run Proxmox detection and choose VM storage",
  };
}

export function proxmoxSetupBlockingIssues(
  requirements: SetupRequirements,
  hasServiceToken: boolean,
  form: SetupForm,
  detection: ProxmoxDetection | null,
): string[] {
  const issues: string[] = [];
  if (!form.proxmoxHostUrl.trim()) issues.push("Proxmox host URL is required.");
  if (!form.proxmoxTokenId.trim()) issues.push("Proxmox API token id is required.");
  if (!form.proxmoxTokenSecret.trim() && !form.proxmoxAcceptedCertificateSha256.trim()) {
    issues.push("Proxmox API token secret is required the first time this profile is used.");
  }
  if (!detection) issues.push("Run Proxmox resource detection before setup.");
  if (!form.proxmoxNode.trim()) issues.push("Proxmox node is required.");
  if (!form.proxmoxVmStorage.trim()) issues.push("VM storage is required.");
  if (!form.proxmoxImportStorage.trim()) issues.push("Import storage is required.");
  if (!form.proxmoxBridge.trim()) issues.push("Proxmox bridge is required.");
  if (parsePositiveInt(form.proxmoxVmid) <= 0) issues.push("Proxmox VMID is required.");
  if (!form.vmName.trim()) issues.push("VM name is required.");
  if (!form.proxmoxSshKeyPath.trim()) issues.push("Proxmox guest SSH private key is required.");
  if (form.proxmoxTemporaryDhcpIp.trim() && !isIpv4Address(form.proxmoxTemporaryDhcpIp.replace(/,/g, ".").trim())) {
    issues.push("Temporary Proxmox DHCP IP must be a valid IPv4 address.");
  }
  if (form.staticIp.trim() && !isIpv4Address(form.staticIp.replace(/,/g, ".").trim())) {
    issues.push("Static guest IP must be a valid IPv4 address.");
  }
  if (form.gateway.trim() && !isIpv4Address(form.gateway.replace(/,/g, ".").trim())) {
    issues.push("Static guest gateway must be a valid IPv4 address.");
  }
  if (!requirements.memoryOk) issues.push(`Memory: ${requirements.memoryRequired}; ${requirements.memoryAvailable}.`);
  if (!requirements.processorOk) issues.push(`CPU Cores: ${requirements.processorRequired}.`);
  if (!requirements.diskOk) issues.push(`Disk: ${requirements.diskRequired}.`);
  if (form.playerIpMode === "external" && !form.playerIp.trim()) {
    issues.push("Player-facing IP is required.");
  }
  if (parsePositiveInt(form.deepDesertWarmServers) > 0) {
    issues.push("Warm Deep Desert Instances are not supported yet; set them to 0 for this build.");
  }
  if (deepDesertInstanceCount(form) > 1) {
    issues.push("Only one Deep Desert instance is supported in this build.");
  }
  if (!hasServiceToken) issues.push("Self-Host Service Token is required.");
  return issues;
}

function isIpv4Address(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 4 && parts.every((part) => {
    if (!/^\d{1,3}$/.test(part)) return false;
    const number = Number(part);
    return number >= 0 && number <= 255;
  });
}

export function setupIssueSummary(
  setupTarget: SetupTarget,
  issues: string[],
  proxmoxDetection: ProxmoxDetection | null,
): string[] {
  if (setupTarget !== "proxmox") return issues.slice(0, 6);
  const summarized: string[] = [];
  if (issues.some((issue) => issue.startsWith("Proxmox host URL") || issue.startsWith("Proxmox API token"))) {
    summarized.push("Complete the Proxmox connection fields.");
  }
  if (issues.includes("Run Proxmox resource detection before setup.")) {
    summarized.push("Run Proxmox resource detection before choosing VM resources.");
  }
  if (proxmoxDetection) {
    summarized.push(
      ...issues.filter(
        (issue) =>
          !issue.startsWith("Proxmox host URL") &&
          !issue.startsWith("Proxmox API token") &&
          issue !== "Run Proxmox resource detection before setup.",
      ),
    );
  }
  if (!proxmoxDetection) {
    if (issues.includes("Self-Host Service Token is required.")) {
      summarized.push("Self-Host Service Token is required.");
    }
    return summarized;
  }
  return summarized.slice(0, 6);
}
