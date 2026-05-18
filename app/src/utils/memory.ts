import { type SetupForm, type CalculatedMemory, type ProxmoxDetection, type UbuntuSshPreflight } from "../types";
import { parsePositiveInt } from "./helpers";

export function calculateRequiredMemory(form: SetupForm): CalculatedMemory {
  const survivalInstances = Math.max(1, parsePositiveInt(form.survivalInstances));
  const deepDesertInstances =
    parsePositiveInt(form.deepDesertPveInstances) + parsePositiveInt(form.deepDesertPvpInstances);
  const survivalGb = survivalInstances * 20;
  const socialGb = form.includeSocial || deepDesertInstances > 0 ? 10 : 0;
  const deepDesertGb = deepDesertInstances * 10;
  const gb = survivalGb + socialGb + deepDesertGb;
  const lines = [
    `${survivalInstances} Hagga Basin ${survivalInstances === 1 ? "instance" : "instances"} x 20 GB = ${survivalGb} GB`,
  ];

  if (form.includeSocial || deepDesertInstances > 0) {
    lines.push("Social Hubs = 10 GB");
  }
  if (deepDesertInstances > 0) {
    lines.push(
      `${deepDesertInstances} Deep Desert ${
        deepDesertInstances === 1 ? "instance" : "instances"
      } x 10 GB = ${deepDesertGb} GB`,
    );
  }

  return { gb, lines };
}

export function normalizeSetupForm(form: SetupForm): SetupForm {
  const deepDesertPve = parsePositiveInt(form.deepDesertPveInstances);
  const deepDesertPvp = parsePositiveInt(form.deepDesertPvpInstances);
  const deepDesertInstances = deepDesertPve + deepDesertPvp;
  const warmServers = Math.min(parsePositiveInt(form.deepDesertWarmServers), deepDesertInstances);
  const normalized = {
    ...form,
    includeSocial: deepDesertInstances > 0 ? true : form.includeSocial,
    deepDesertPveInstances: deepDesertPve > 0 ? "1" : "0",
    deepDesertPvpInstances: deepDesertPve > 0 ? "0" : deepDesertPvp > 0 ? "1" : "0",
    deepDesertWarmServers: String(warmServers),
  };
  if (normalized.playerIpMode === "local" && normalized.staticIp && normalized.playerIp !== normalized.staticIp) {
    return { ...normalized, playerIp: normalized.staticIp };
  }
  return normalized;
}

export function deepDesertInstanceCount(form: SetupForm): number {
  return parsePositiveInt(form.deepDesertPveInstances) + parsePositiveInt(form.deepDesertPvpInstances);
}

export function minimumVmMemoryGb(form: SetupForm, calculatedMemory: CalculatedMemory): number {
  if (form.setupTarget !== "proxmox" || !form.enableSwap) {
    return calculatedMemory.gb;
  }
  const survivalInstances = Math.max(1, parsePositiveInt(form.survivalInstances));
  const deepDesertInstances = deepDesertInstanceCount(form);
  const socialGb = form.includeSocial || deepDesertInstances > 0 ? 5 : 0;
  const lowMemoryGb = survivalInstances * 15 + deepDesertInstances * 10 + socialGb;
  return Math.min(calculatedMemory.gb, Math.max(1, lowMemoryGb));
}

export function effectiveVmMemoryGb(form: SetupForm, calculatedMemory: CalculatedMemory): number {
  return Math.max(minimumVmMemoryGb(form, calculatedMemory), parsePositiveInt(form.vmMemoryGb));
}

export function proxmoxAvailableMemoryWholeGb(form: SetupForm, detection: ProxmoxDetection | null): number | null {
  const node = detection?.nodes.find((item) => item.node === form.proxmoxNode.trim()) ?? detection?.nodes[0] ?? null;
  if (!node) return null;
  const availableBytes = Math.max(0, node.maxmem - node.mem);
  return Math.floor(availableBytes / 1024 / 1024 / 1024);
}

export function effectiveProxmoxVmMemoryGb(
  form: SetupForm,
  calculatedMemory: CalculatedMemory,
  detection: ProxmoxDetection | null,
): number {
  const requestedMemoryGb = effectiveVmMemoryGb(form, calculatedMemory);
  const availableMemoryGb = proxmoxAvailableMemoryWholeGb(form, detection);
  if (availableMemoryGb === null) return requestedMemoryGb;
  return Math.min(requestedMemoryGb, availableMemoryGb);
}

export function proxmoxMemoryLimitText(
  form: SetupForm,
  calculatedMemory: CalculatedMemory,
  detection: ProxmoxDetection | null,
): string {
  const minimumGb = minimumVmMemoryGb(form, calculatedMemory);
  const requestedGb = effectiveVmMemoryGb(form, calculatedMemory);
  if (form.setupTarget !== "proxmox") {
    return `Minimum for this profile is ${minimumGb} GB. You can increase it, but not decrease below the selected profile.`;
  }
  const availableGb = proxmoxAvailableMemoryWholeGb(form, detection);
  if (availableGb === null) {
    return `Minimum for this profile is ${minimumGb} GB. Run Proxmox detection to cap this value to available node memory.`;
  }
  if (availableGb < requestedGb) {
    return `Capped at ${availableGb} GB, the selected node's available whole-GB memory. Profile target is ${minimumGb} GB.`;
  }
  return `Minimum for this profile is ${minimumGb} GB. You can increase it up to ${availableGb} GB available on the selected node.`;
}

export function effectiveProcessorCount(form: SetupForm): number {
  return Math.max(4, parsePositiveInt(form.processorCount));
}

export function recommendedUbuntuSwapGb(calculatedMemory: CalculatedMemory, preflight: UbuntuSshPreflight): number {
  const gib = 1024 * 1024 * 1024;
  const requiredBytes = calculatedMemory.gb * gib;
  const shortfallBytes = Math.max(0, requiredBytes - preflight.availableMemoryBytes);
  const shortfallGb = Math.ceil(shortfallBytes / gib);
  return Math.min(64, Math.max(2, shortfallGb));
}
