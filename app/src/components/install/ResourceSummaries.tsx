import { Box, Flex, Text, Badge } from "@radix-ui/themes";
import {
  type HostReadiness,
  type SetupRequirements,
  type CalculatedMemory,
  type UbuntuSshPreflight,
  type ProxmoxDetection
} from "../../types";
import { formatGiB } from "../../utils/helpers";
import { recommendedUbuntuSwapGb } from "../../utils/memory";
import { InfoRow } from "../Common";

export function LocalResourceSummary({
  readiness,
  requirements,
}: {
  readiness: HostReadiness;
  requirements: SetupRequirements;
}) {
  const rows: Array<[string, string, "green" | "amber" | "red"]> = [
    [
      "Hyper-V",
      readiness.hypervAvailable && readiness.vmmsRunning ? "Available and running" : "Needs attention",
      readiness.hypervAvailable && readiness.vmmsRunning ? "green" : "red",
    ],
    [
      "Memory",
      `${requirements.memoryRequired}; ${requirements.memoryAvailable}`,
      requirements.memoryOk ? "green" : "amber",
    ],
    [
      "CPU",
      `${requirements.processorRequired}; ${requirements.processorAvailable}`,
      requirements.processorOk ? "green" : "amber",
    ],
  ];
  return (
    <Box className="info-card" mb="3">
      {rows.map(([label, value, tone]) => (
        <InfoRow key={label} label={label} value={value} tone={tone} />
      ))}
    </Box>
  );
}

export function UbuntuSwapNotice({
  calculatedMemory,
  preflight,
  enabled,
}: {
  calculatedMemory: CalculatedMemory;
  preflight: UbuntuSshPreflight | null;
  enabled: boolean;
}) {
  if (!preflight) {
    return (
      <Text as="div" size="2" color="gray" mt="2">
        Run remote detection to calculate a swap recommendation.
      </Text>
    );
  }
  const requiredBytes = calculatedMemory.gb * 1024 * 1024 * 1024;
  const recommendedSwapGb = recommendedUbuntuSwapGb(calculatedMemory, preflight);
  const totalMemory = preflight.totalMemoryBytes;
  const memoryShortfallIsLarge = totalMemory > 0 && totalMemory < requiredBytes * 0.8;
  const hasExistingSwap = preflight.swapTotalBytes > 0;
  if (!enabled && !hasExistingSwap && !memoryShortfallIsLarge) {
    return (
      <Text as="div" size="2" color="gray" mt="2">
        No swap will be created.
      </Text>
    );
  }
  return (
    <Box className={memoryShortfallIsLarge ? "destructive-warning" : "setup-guide"} mt="2">
      <Flex direction="column" gap="1">
        {hasExistingSwap ? (
          <Text size="2">
            Existing swap detected: {formatGiB(preflight.swapTotalBytes)}. Swap can reduce performance when it is used heavily.
          </Text>
        ) : null}
        {enabled ? (
          <Text size="2">
            Setup will create a native Ubuntu swapfile of about {recommendedSwapGb} GB.
          </Text>
        ) : null}
        {memoryShortfallIsLarge ? (
          <Text size="2" weight="medium">
            Physical memory is more than 20% below the selected layout recommendation. The server may run, but heavy swap use can cause stalls and disconnects.
          </Text>
        ) : null}
      </Flex>
    </Box>
  );
}

export function RemotePreflightSummary({ preflight }: { preflight: UbuntuSshPreflight }) {
  const rows: Array<[string, string, "green" | "amber" | "red"]> = [
    ["Host", `${preflight.hostname} (${preflight.osPrettyName})`, "green"],
    ["Public IP", preflight.publicIp || "Not detected", "green"],
    ["Private IPs", preflight.ipv4Addresses.length ? preflight.ipv4Addresses.join(", ") : "None detected", "green"],
    ["Memory", `${formatGiB(preflight.availableMemoryBytes)} available of ${formatGiB(preflight.totalMemoryBytes)}`, "green"],
    ["Swap", preflight.swapTotalBytes > 0 ? `${formatGiB(preflight.swapTotalBytes)} configured` : "None configured", preflight.swapTotalBytes > 0 ? "amber" : "green"],
    ["Disk", `${formatGiB(preflight.rootDiskAvailableBytes)} free of ${formatGiB(preflight.rootDiskTotalBytes)} on /`, "green"],
    ["CPU", `${preflight.logicalProcessorCount} logical processors`, "green"],
    ["Access", preflight.uid === 0 ? "root" : preflight.passwordlessSudo ? "passwordless sudo" : preflight.sudoCheck || "sudo password required", preflight.uid === 0 || preflight.passwordlessSudo ? "green" : "red"],
    ["Existing tools", `SteamCMD ${preflight.steamcmdInstalled ? "present" : "missing"}, k3s ${preflight.k3sInstalled ? "present" : "missing"}`, preflight.k3sInstalled ? "amber" : "green"],
  ];
  return (
    <Box className="info-card" mb="3">
      {rows.map(([label, value, tone]) => (
        <InfoRow key={label} label={label} value={value} tone={tone} />
      ))}
    </Box>
  );
}

export function ProxmoxDetectionSummary({ detection }: { detection: ProxmoxDetection }) {
  const rows: Array<[string, string, "green" | "amber" | "red"]> = [
    ["Version", detection.version.version || "Unknown", "green"],
    ["Certificate", detection.certificateTrusted ? "Trusted fingerprint" : "Fingerprint captured", detection.certificateTrusted ? "green" : "amber"],
    ["Nodes", detection.nodes.map((node) => `${node.node} (${node.status || "unknown"})`).join(", ") || "None", detection.nodes.length ? "green" : "red"],
    ["Storage", detection.storages.map((storage) => `${storage.storage}: ${storage.content || "unknown"}`).join(", ") || "None", detection.storages.length ? "green" : "red"],
    ["Bridges", detection.bridges.map((bridge) => bridge.cidr ? `${bridge.iface} ${bridge.cidr}` : bridge.iface).join(", ") || "None", detection.bridges.length ? "green" : "red"],
    ["Next VMID", detection.nextVmid ? String(detection.nextVmid) : "Not detected", detection.nextVmid ? "green" : "amber"],
  ];
  return (
    <Box className="info-card" mb="3">
      {rows.map(([label, value, tone]) => (
        <InfoRow key={label} label={label} value={value} tone={tone} />
      ))}
    </Box>
  );
}

export function SetupWarningPills({ warnings }: { warnings: string[] }) {
  return (
    <Flex gap="2" wrap="wrap" className="setup-warning-pills">
      {warnings.map((warning) => (
        <Badge key={warning} color="amber" variant="soft">
          {warning}
        </Badge>
      ))}
    </Flex>
  );
}

export function UbuntuSetupGuide() {
  const rows = [
    "Use an Ubuntu 24+ VPS or dedicated server with enough RAM and CPU for the selected layout.",
    "Add your SSH public key during host creation. Use IPv4 only for wider compatibility.",
    "Restrict SSH port 22 to your IP in the hosting firewall when possible.",
    "Allow UDP 7777-7810 and TCP 31982 from any IP for players.",
  ];
  return (
    <Box className="setup-guide" mb="3">
      <Flex direction="column" gap="2">
        <ul className="setup-guide-list">
          {rows.map((row) => (
            <li key={row}>{row}</li>
          ))}
        </ul>
      </Flex>
    </Box>
  );
}
