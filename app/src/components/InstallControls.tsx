import { useState, type ChangeEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import {
  Card,
  Flex,
  Box,
  Heading,
  Text,
  SegmentedControl,
  Grid,
  Select,
  Button,
  TextField,
  Checkbox,
  Switch,
  Separator,
  Link,
  Badge,
  TextArea
} from "@radix-ui/themes";
import { DesktopIcon, GlobeIcon, RocketIcon, MixIcon, CubeIcon, LightningBoltIcon } from "@radix-ui/react-icons";
import {
  type SetupForm,
  type CalculatedMemory,
  type SetupLayoutPreview,
  type HostReadiness,
  type DriveCandidate,
  type NetworkAdapterCandidate,
  type DetectionState,
  type EnvironmentGate,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type ServerPackageStatus,
  type ServerPackageCheckStatus,
  type SetupTarget,
  type PlayerIpMode,
  type NetworkMode,
  type SetupRequirements
} from "../types";
import {
  oneToFour,
  zeroToOne,
  networkStatusLabel,
  formatGiB,
  zeroTo
} from "../utils/helpers";
import {
  effectiveVmMemoryGb,
  effectiveProxmoxVmMemoryGb,
  effectiveProcessorCount,
  proxmoxMemoryLimitText,
  recommendedUbuntuSwapGb
} from "../utils/memory";
import {
  remoteSetupRequirementStatus,
  proxmoxSetupRequirementStatus,
  setupRequirementStatus,
  remoteSetupBlockingIssues,
  proxmoxSetupBlockingIssues,
  setupBlockingIssues,
  setupIssueSummary
} from "../utils/validation";
import {
  SetupSection,
  FormRow,
  LayoutRow,
  InlineRequirement,
  InfoRow,
  PortForwardingNotice
} from "./Common";

export const defaultHyperVSwitchName = "DuneAwakeningServerSwitch";

export function VisualMemoryGauge({
  requiredGb,
  hostAvailableBytes,
  enableSwap,
  plannedSwapGb
}: {
  requiredGb: number;
  hostAvailableBytes: number;
  enableSwap: boolean;
  plannedSwapGb: number;
}) {
  const hostAvailableGb = hostAvailableBytes > 0 ? hostAvailableBytes / (1024 * 1024 * 1024) : 0;
  if (hostAvailableGb === 0) {
    return (
      <Box className="memory-gauge-container" p="3" style={{ background: "rgba(0,0,0,0.2)", borderRadius: "6px", border: "1px solid rgba(255,255,255,0.05)" }}>
        <Flex justify="between" mb="2" align="center">
          <Text size="2" color="gray" weight="medium">System Memory Allocation</Text>
          <Text size="2" weight="bold" color="amber">
            {requiredGb} GB Required
          </Text>
        </Flex>
        <Text size="1" color="gray">Run target preflight detection to verify available host memory.</Text>
      </Box>
    );
  }

  const totalBarMaxGb = Math.max(requiredGb + 8, hostAvailableGb);
  const layoutPercent = Math.min(100, (requiredGb / totalBarMaxGb) * 100);
  const hostFreePercent = Math.max(0, 100 - layoutPercent);

  const ok = hostAvailableGb >= requiredGb || (enableSwap && (hostAvailableGb + plannedSwapGb) >= requiredGb);
  const alertColor = ok ? "var(--bronze-9)" : "var(--red-9)";

  return (
    <Box className="memory-gauge-container" p="3" style={{ background: "rgba(0,0,0,0.2)", borderRadius: "6px", border: "1px solid rgba(255,255,255,0.05)" }}>
      <Flex justify="between" mb="2" align="center">
        <Text size="2" color="gray" weight="medium">System Memory Allocation</Text>
        <Text size="2" weight="bold" style={{ color: alertColor }}>
          {requiredGb} GB / {hostAvailableGb.toFixed(1)} GB Available
        </Text>
      </Flex>

      <div className="gauge-bar-track" style={{
        height: "12px",
        width: "100%",
        backgroundColor: "rgba(255, 255, 255, 0.05)",
        borderRadius: "6px",
        overflow: "hidden",
        display: "flex",
        border: "1px solid rgba(255,255,255,0.03)"
      }}>
        <div className="gauge-bar-fill-layout" style={{
          width: `${layoutPercent}%`,
          backgroundColor: alertColor,
          transition: "width 0.4s ease",
          boxShadow: `0 0 8px ${alertColor}`
        }} />
        <div className="gauge-bar-fill-free" style={{
          width: `${hostFreePercent}%`,
          backgroundColor: "rgba(255, 255, 255, 0.1)"
        }} />
      </div>

      {enableSwap && plannedSwapGb > 0 && (
        <Text size="1" color="amber" mt="2" as="div" style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <span>⚠️</span> Memory constraints active: allocation includes a {plannedSwapGb} GB swap buffer on the host.
        </Text>
      )}
    </Box>
  );
}

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
    ["Access", preflight.uid === 0 ? "root" : preflight.passwordlessSudo ? "passwordless sudo" : "limited", preflight.uid === 0 || preflight.passwordlessSudo ? "green" : "red"],
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

export function InstallControls({
  form,
  calculatedMemory,
  layoutPreview,
  hostReadiness,
  driveCandidates,
  networkAdapters,
  networkDetection,
  externalIp,
  environmentGate,
  setupRunning,
  vmDestinationHasVm,
  remotePreflight,
  remotePreflightStatus,
  proxmoxDetection,
  proxmoxDetectionStatus,
  serverPackageStatus,
  serverPackageCheckStatus,
  update,
  onUpdateServerPackage,
  onLocalDetection,
  onRemotePreflight,
  onProxmoxDetection,
  onStart,
}: {
  form: SetupForm;
  calculatedMemory: CalculatedMemory;
  layoutPreview: SetupLayoutPreview;
  hostReadiness: HostReadiness | null;
  driveCandidates: DriveCandidate[];
  networkAdapters: NetworkAdapterCandidate[];
  networkDetection: DetectionState;
  externalIp: string | null;
  environmentGate: EnvironmentGate;
  setupRunning: boolean;
  vmDestinationHasVm: boolean;
  remotePreflight: UbuntuSshPreflight | null;
  remotePreflightStatus: DetectionState;
  proxmoxDetection: ProxmoxDetection | null;
  proxmoxDetectionStatus: DetectionState;
  serverPackageStatus: ServerPackageStatus | null;
  serverPackageCheckStatus: ServerPackageCheckStatus;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
  onUpdateServerPackage: () => void;
  onLocalDetection: () => void;
  onRemotePreflight: () => void;
  onProxmoxDetection: () => void;
  onStart: () => void;
}) {
  const [setupStep, setSetupStep] = useState<"target" | "config" | "network" | "review">("target");
  const deepDesertEnabled = layoutPreview.deepDesertTotal > 0;
  const warmOptions = zeroTo(layoutPreview.deepDesertTotal);
  const vmMemoryGb =
    form.setupTarget === "proxmox"
      ? effectiveProxmoxVmMemoryGb(form, calculatedMemory, proxmoxDetection)
      : effectiveVmMemoryGb(form, calculatedMemory);

  const requirements =
    form.setupTarget === "ubuntu"
      ? remoteSetupRequirementStatus(
          calculatedMemory,
          form.diskGb,
          form.processorCount,
          remotePreflight,
          form.enableSwap,
        )
      : form.setupTarget === "proxmox"
        ? proxmoxSetupRequirementStatus(
            calculatedMemory,
            vmMemoryGb,
            form.diskGb,
            form.processorCount,
            form.proxmoxNode,
            form.proxmoxVmStorage,
            proxmoxDetection,
          )
      : setupRequirementStatus(
          calculatedMemory,
          form.vmMemoryGb,
          form.diskGb,
          form.processorCount,
          form.vmDestination,
          hostReadiness,
          driveCandidates,
        );

  const hasServiceToken = form.tokenSource.trim().length > 0;
  const setupNeedsServerPackage = form.setupTarget === "hyperv" || form.setupTarget === "proxmox";
  const serverPackageCurrent =
    !!serverPackageStatus?.complete &&
    !serverPackageStatus.updateAvailable &&
    serverPackageCheckStatus === "current";
  const serverPackageBusy =
    serverPackageCheckStatus === "checking" || serverPackageCheckStatus === "updating";
  const packageBlocksSetup =
    setupNeedsServerPackage &&
    !serverPackageCurrent &&
    (serverPackageCheckStatus === "idle" ||
      serverPackageCheckStatus === "failed" ||
      serverPackageBusy ||
      !serverPackageStatus?.complete ||
      !!serverPackageStatus.updateAvailable);

  const packageActionLabel =
    serverPackageCheckStatus === "checking"
      ? "Checking..."
      : serverPackageCheckStatus === "updating"
        ? "Updating..."
        : serverPackageCheckStatus === "available" || serverPackageCheckStatus === "missing"
          ? "Update package"
          : "Check package";

  const setupIssues =
    form.setupTarget === "ubuntu"
      ? remoteSetupBlockingIssues(requirements, hasServiceToken, form, remotePreflight)
      : form.setupTarget === "proxmox"
        ? proxmoxSetupBlockingIssues(requirements, hasServiceToken, form, proxmoxDetection)
      : setupBlockingIssues(environmentGate, requirements, hasServiceToken, vmDestinationHasVm, form);

  const visibleSetupIssues = setupIssueSummary(form.setupTarget, setupIssues, proxmoxDetection);
  const canStart = setupIssues.length === 0 && !packageBlocksSetup;
  const hypervDetectionReady = networkDetection === "ready" && environmentGate.canContinue;
  const ubuntuDetectionReady = remotePreflightStatus === "ready" && !!remotePreflight;
  const proxmoxDetectionReady = proxmoxDetectionStatus === "ready" && !!proxmoxDetection;
  const flowDetectionReady =
    form.setupTarget === "ubuntu"
      ? ubuntuDetectionReady
      : form.setupTarget === "proxmox"
        ? proxmoxDetectionReady
        : hypervDetectionReady;

  return (
    <Card size="3" variant="surface" className="pane setup-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="start" justify="between" gap="4">
          <Box>
            <Heading size="5">Server Setup</Heading>
            <Text as="p" size="2" color="gray">
              Please configure your server settings below. You'll be able to change them later.
            </Text>
          </Box>
        </Flex>

        {/* Wizard Stepper Tabs Navigation */}
        <SegmentedControl.Root
          value={setupStep}
          onValueChange={(value) => {
            const nextStep = value as any;
            if (nextStep !== "target" && !flowDetectionReady) {
              return;
            }
            setSetupStep(nextStep);
          }}
          size="2"
          variant="surface"
          style={{ width: "100%" }}
        >
          <SegmentedControl.Item value="target">1. Platform & Detection</SegmentedControl.Item>
          <SegmentedControl.Item value="config" style={{ opacity: flowDetectionReady ? 1 : 0.6, cursor: flowDetectionReady ? "pointer" : "not-allowed" }}>2. World & Layout</SegmentedControl.Item>
          <SegmentedControl.Item value="network" style={{ opacity: flowDetectionReady ? 1 : 0.6, cursor: flowDetectionReady ? "pointer" : "not-allowed" }}>3. Networking</SegmentedControl.Item>
          <SegmentedControl.Item value="review" style={{ opacity: flowDetectionReady ? 1 : 0.6, cursor: flowDetectionReady ? "pointer" : "not-allowed" }}>4. Review & Deploy</SegmentedControl.Item>
        </SegmentedControl.Root>

        <Box className="setup-scroll" style={{ flexGrow: 1, overflowY: "auto", paddingRight: "4px" }}>
          <Flex direction="column" gap="5" className={setupRunning ? "setup-controls is-disabled" : "setup-controls"}>

            {/* STEP 1: Platform & Detection */}
            {setupStep === "target" && (
              <>
                <SetupSection icon={DesktopIcon} title="Setup Target" className="setup-order-target">
                  <Grid columns="180px 1fr" gap="3" align="center">
                    <Text size="2" weight="medium">
                      Target
                    </Text>
                    <Select.Root
                      value={form.setupTarget}
                      onValueChange={(value) => {
                        const target = value as SetupTarget;
                        update("setupTarget", target);
                        if (target === "ubuntu") {
                          update("playerIpMode", "external");
                          update("playerIp", form.playerIp || form.remoteHost);
                        } else if (target === "proxmox") {
                          update("networkMode", "dhcp");
                          update("playerIpMode", "external");
                        }
                      }}
                    >
                      <Select.Trigger />
                      <Select.Content>
                        <Select.Item value="hyperv">Local Windows Hyper-V</Select.Item>
                        <Select.Item value="ubuntu">Remote Ubuntu over SSH</Select.Item>
                        <Select.Item value="proxmox">Proxmox VE Cluster</Select.Item>
                      </Select.Content>
                    </Select.Root>
                  </Grid>
                  {form.setupTarget === "ubuntu" ? (
                    <Box className="destructive-warning" mt="3">
                      <Text as="div" size="2" weight="medium">
                        DO NOT USE AN EXISTING SERVER, ALWAYS CREATE A FRESH SERVER, WE ARE NOT RESPONSIBLE OF ANY DATA LOSS YOU MIGHT ENCOUNTER!
                      </Text>
                      <Text as="p" size="2" color="gray">
                        Remote setup installs packages, creates users, configures k3s, downloads server files, opens
                        service ports, and writes system configuration. Use a clean Ubuntu host dedicated to this Dune
                        server so setup cannot conflict with existing workloads or data.
                      </Text>
                    </Box>
                  ) : form.setupTarget === "proxmox" ? (
                    <Box className="destructive-warning" mt="3">
                      <Text as="div" size="2" weight="medium">
                        Proxmox setup creates a new VM and uploads a converted vendor disk image.
                      </Text>
                      <Text as="p" size="2" color="gray">
                        Use a dedicated VMID and storage target. The guest boots with DHCP first, then optional static
                        networking is applied after SSH is reachable.
                      </Text>
                    </Box>
                  ) : null}
                </SetupSection>

                {form.setupTarget === "hyperv" ? (
                  <SetupSection icon={DesktopIcon} title="Local Hyper-V Host" className="setup-order-vm">
                    <Flex direction="column" gap="2">
                      <Flex direction="column" gap="2">
                        <Button
                          type="button"
                          variant="surface"
                          className="setup-detect-button"
                          onClick={onLocalDetection}
                          disabled={networkDetection === "detecting"}
                        >
                          {networkDetection === "detecting" ? "Detecting..." : "Detect local resources"}
                        </Button>
                      </Flex>
                      {networkDetection !== "ready" ? (
                        <Box className="setup-guide">
                          <Text size="2">
                            Run local detection before setup so the app can verify Hyper-V, memory, disk, and network adapter support.
                          </Text>
                        </Box>
                      ) : null}
                      {networkDetection === "ready" && hostReadiness ? (
                        <LocalResourceSummary readiness={hostReadiness} requirements={requirements} />
                      ) : null}
                      <Flex
                        direction="column"
                        gap="2"
                        className={hypervDetectionReady ? "setup-dependent-fields" : "setup-dependent-fields is-flow-disabled"}
                      >
                        <FormRow label="VM Location">
                          <Grid columns="1fr auto" gap="2">
                            <TextField.Root
                              placeholder="Resolving default VM location..."
                              value={form.vmDestination}
                              onChange={(event) => update("vmDestination", event.target.value)}
                            />
                            <Button
                              type="button"
                              variant="surface"
                              onClick={async () => {
                                const selected = await open({
                                  directory: true,
                                  defaultPath: form.vmDestination || undefined,
                                  multiple: false,
                                  title: "Choose VM files destination",
                                });
                                if (typeof selected === "string") {
                                  update("vmDestination", selected);
                                }
                              }}
                            >
                              Choose
                            </Button>
                          </Grid>
                          <InlineRequirement
                            ok={requirements.diskOk && !vmDestinationHasVm}
                            text={
                              vmDestinationHasVm
                                ? "Destination already contains VM files. Choose another folder."
                                : `${requirements.diskRequired}; ${requirements.diskAvailable}`
                            }
                          />
                        </FormRow>
                        <FormRow label="Disk Size">
                          <TextField.Root value={form.diskGb} onChange={(event) => update("diskGb", event.target.value)}>
                            <TextField.Slot side="right">GB</TextField.Slot>
                          </TextField.Root>
                        </FormRow>
                        <FormRow label="Save Server">
                          <Flex align="center" gap="3" className="checkbox-copy-row">
                            <Checkbox
                              checked={form.saveLocalServer}
                              onCheckedChange={(value) => update("saveLocalServer", value === true)}
                            />
                            <Text size="2" color="gray">
                              Add this Hyper-V server to Servers when setup completes
                            </Text>
                          </Flex>
                        </FormRow>
                      </Flex>
                    </Flex>
                  </SetupSection>
                ) : form.setupTarget === "ubuntu" ? (
                  <SetupSection icon={DesktopIcon} title="Remote Ubuntu Host" className="setup-order-remote-host">
                    <Flex direction="column" gap="2">
                      <UbuntuSetupGuide />
                      <FormRow label="Server IP">
                        <TextField.Root
                          placeholder="IPv4 address, for example 203.0.113.10"
                          value={form.remoteHost}
                          onChange={(event) => {
                            update("remoteHost", event.target.value);
                            if (form.playerIpMode === "external" && !form.playerIp.trim()) {
                              update("playerIp", event.target.value);
                            }
                          }}
                        />
                      </FormRow>
                      <FormRow label="SSH User">
                        <TextField.Root value={form.remoteUser} onChange={(event) => update("remoteUser", event.target.value)} />
                      </FormRow>
                      <FormRow label="Private Key">
                        <Grid columns="1fr auto" gap="2">
                          <TextField.Root
                            placeholder="Choose SSH private key"
                            value={form.remoteKeyPath}
                            onChange={(event) => update("remoteKeyPath", event.target.value)}
                          />
                          <Button
                            type="button"
                            variant="surface"
                            onClick={async () => {
                              const selected = await open({
                                directory: false,
                                multiple: false,
                                title: "Choose SSH private key",
                              });
                              if (typeof selected === "string") {
                                update("remoteKeyPath", selected);
                              }
                            }}
                          >
                            Choose
                          </Button>
                        </Grid>
                      </FormRow>
                      <FormRow label="Save Server">
                        <Flex align="center" gap="3" className="checkbox-copy-row">
                          <Checkbox
                            checked={form.saveRemoteServer}
                            onCheckedChange={(value) => update("saveRemoteServer", value === true)}
                          />
                          <Text size="2" color="gray">
                            Add this remote Ubuntu server to Servers when setup starts
                          </Text>
                        </Flex>
                      </FormRow>
                    </Flex>

                    <Button
                      type="button"
                      variant="surface"
                      className="setup-detect-button"
                      onClick={onRemotePreflight}
                      disabled={
                        remotePreflightStatus === "detecting" ||
                        !form.remoteHost.trim() ||
                        !form.remoteUser.trim() ||
                        !form.remoteKeyPath.trim()
                      }
                      style={{ marginTop: "12px" }}
                    >
                      {remotePreflightStatus === "detecting" ? "Detecting..." : remotePreflight ? "Refresh remote resources" : "Detect remote resources"}
                    </Button>
                    {remotePreflight ? (
                      <RemotePreflightSummary preflight={remotePreflight} />
                    ) : null}
                  </SetupSection>
                ) : (
                  <SetupSection icon={DesktopIcon} title="Proxmox Connection" className="setup-order-remote-host">
                    <Flex direction="column" gap="2">
                      <Grid columns="2" gap="3">
                        <FormRow label="Host URL">
                          <TextField.Root
                            placeholder="https://proxmox.example.local:8006"
                            value={form.proxmoxHostUrl}
                            onChange={(event) => update("proxmoxHostUrl", event.target.value)}
                          />
                        </FormRow>
                        <FormRow label="API Token ID">
                          <TextField.Root
                            placeholder="root@pam!dune-manager"
                            value={form.proxmoxTokenId}
                            onChange={(event) => update("proxmoxTokenId", event.target.value)}
                          />
                        </FormRow>
                      </Grid>
                      <FormRow label="API Token Secret">
                        <TextField.Root
                          type="password"
                          placeholder="Stored in the OS credential store after detection"
                          value={form.proxmoxTokenSecret}
                          onChange={(event) => update("proxmoxTokenSecret", event.target.value)}
                        />
                      </FormRow>
                      <Button
                        type="button"
                        variant="surface"
                        className="setup-detect-button"
                        onClick={onProxmoxDetection}
                        disabled={
                          proxmoxDetectionStatus === "detecting" ||
                          !form.proxmoxHostUrl.trim() ||
                          !form.proxmoxTokenId.trim() ||
                          (!form.proxmoxTokenSecret.trim() && !form.proxmoxAcceptedCertificateSha256.trim())
                        }
                      >
                        {proxmoxDetectionStatus === "detecting" ? "Detecting..." : proxmoxDetection ? "Refresh Proxmox resources" : "Detect Proxmox resources"}
                      </Button>
                      {proxmoxDetection ? (
                        <ProxmoxDetectionSummary detection={proxmoxDetection} />
                      ) : null}
                      {!proxmoxDetectionReady ? (
                        <Box className="setup-guide">
                          <Text size="2">
                            Run Proxmox resource detection before selecting node, storage, bridge, and VMID.
                          </Text>
                        </Box>
                      ) : null}
                      <Box className={proxmoxDetectionReady ? "" : "is-flow-disabled"} aria-disabled={!proxmoxDetectionReady}>
                        <Flex direction="column" gap="3">
                          <Grid columns="2" gap="3">
                            <FormRow label="Node">
                              <Select.Root value={form.proxmoxNode} onValueChange={(value) => update("proxmoxNode", value)}>
                                <Select.Trigger />
                                <Select.Content>
                                  {(proxmoxDetection?.nodes ?? []).map((node) => (
                                    <Select.Item key={node.node} value={node.node}>{node.node}</Select.Item>
                                  ))}
                                </Select.Content>
                              </Select.Root>
                            </FormRow>
                            <FormRow label="VMID">
                              <TextField.Root value={form.proxmoxVmid} onChange={(event) => update("proxmoxVmid", event.target.value)} />
                            </FormRow>
                            <FormRow label="VM storage">
                              <Select.Root value={form.proxmoxVmStorage} onValueChange={(value) => update("proxmoxVmStorage", value)}>
                                <Select.Trigger />
                                <Select.Content>
                                  {(proxmoxDetection?.storages ?? []).filter((storage) => storage.content.includes("images")).map((storage) => (
                                    <Select.Item key={storage.storage} value={storage.storage}>{storage.storage}</Select.Item>
                                  ))}
                                </Select.Content>
                              </Select.Root>
                            </FormRow>
                            <FormRow label="Import storage">
                              <Select.Root value={form.proxmoxImportStorage} onValueChange={(value) => update("proxmoxImportStorage", value)}>
                                <Select.Trigger />
                                <Select.Content>
                                  {(proxmoxDetection?.storages ?? []).map((storage) => (
                                    <Select.Item key={storage.storage} value={storage.storage}>{storage.storage}</Select.Item>
                                  ))}
                                </Select.Content>
                              </Select.Root>
                            </FormRow>
                          </Grid>
                          <FormRow label="Bridge">
                            <Select.Root
                              value={form.proxmoxBridge}
                              onValueChange={(value) => {
                                update("proxmoxBridge", value);
                                const bridge = proxmoxDetection?.bridges.find((item) => item.iface === value);
                                if (bridge?.cidr) update("proxmoxBridgeCidr", bridge.cidr);
                              }}
                            >
                              <Select.Trigger />
                              <Select.Content>
                                {(proxmoxDetection?.bridges ?? []).map((bridge) => (
                                  <Select.Item key={bridge.iface} value={bridge.iface}>{bridge.iface}</Select.Item>
                                ))}
                              </Select.Content>
                            </Select.Root>
                          </FormRow>
                          <FormRow label="Save Server">
                            <Flex align="center" gap="3" className="checkbox-copy-row">
                              <Checkbox
                                checked={form.saveRemoteServer}
                                onCheckedChange={(value) => update("saveRemoteServer", value === true)}
                              />
                              <Text size="2" color="gray">
                                Add this Proxmox Alpine server to Servers when setup starts
                              </Text>
                            </Flex>
                          </FormRow>
                        </Flex>
                      </Box>
                    </Flex>
                  </SetupSection>
                )}

                {packageBlocksSetup ? (
                  <Box className="setup-package-gate" mt="4">
                    <Flex align="center" justify="between" gap="3" wrap="wrap">
                      <Box minWidth="0" style={{ flex: 1 }}>
                        <Text as="div" size="2" weight="medium">
                          Server package update required
                        </Text>
                        <Text as="div" size="2" color="gray">
                          {serverPackageStatus?.message || "Check the Dune server package before continuing."}
                        </Text>
                      </Box>
                      <Button
                        size="2"
                        color={serverPackageCheckStatus === "failed" ? "red" : "amber"}
                        variant="surface"
                        disabled={serverPackageBusy}
                        onClick={onUpdateServerPackage}
                      >
                        {packageActionLabel}
                      </Button>
                    </Flex>
                  </Box>
                ) : null}
              </>
            )}

            {/* STEP 2: World & Layout */}
            {setupStep === "config" && (
              <>
                <SetupSection
                  icon={GlobeIcon}
                  title="World"
                  className={form.setupTarget === "ubuntu" ? "setup-order-world-ubuntu" : "setup-order-world"}
                  disabled={!flowDetectionReady}
                >
                  <Grid columns="2" gap="3">
                    <FormRow label="World name">
                      <TextField.Root value={form.worldName} onChange={(event) => update("worldName", event.target.value)} />
                    </FormRow>
                    <FormRow label="Region">
                      <Select.Root value={form.region} onValueChange={(value) => update("region", value)}>
                        <Select.Trigger />
                        <Select.Content>
                          <Select.Item value="Europe Test">Europe Test</Select.Item>
                          <Select.Item value="North America Test">North America Test</Select.Item>
                        </Select.Content>
                      </Select.Root>
                    </FormRow>
                  </Grid>
                  <FormRow label="Self-Host Service Token">
                    <TextArea
                      placeholder="Paste your Self-Host Service Token"
                      value={form.tokenSource}
                      onChange={(event: ChangeEvent<HTMLTextAreaElement>) => update("tokenSource", event.target.value)}
                    />
                    <Text as="p" size="2" color="gray" mt="1">
                      Get the token from{" "}
                      <Link
                        href="#"
                        onClick={(event) => {
                          event.preventDefault();
                          void openExternal("https://account-pts.duneawakening.com/account");
                        }}
                      >
                        account-pts.duneawakening.com/account
                      </Link>
                      .
                    </Text>
                  </FormRow>
                </SetupSection>

                <SetupSection
                  icon={RocketIcon}
                  title="World Layout"
                  className={form.setupTarget === "ubuntu" ? "setup-order-layout-ubuntu" : "setup-order-layout"}
                  disabled={!flowDetectionReady}
                >
                  <Flex direction="column" gap="2">
                    <LayoutRow label="Hagga Basin">
                      <Select.Root
                        value={form.survivalInstances}
                        onValueChange={(value) => update("survivalInstances", value)}
                      >
                        <Select.Trigger />
                        <Select.Content>
                          {oneToFour.map((value) => (
                            <Select.Item key={value} value={value}>
                              {value} {value === "1" ? "instance" : "instances"}
                            </Select.Item>
                          ))}
                        </Select.Content>
                      </Select.Root>
                    </LayoutRow>
                    <LayoutRow label="Social Hubs">
                      <Flex align="center" gap="3">
                        <Checkbox
                          checked={deepDesertEnabled || form.includeSocial}
                          disabled={deepDesertEnabled}
                          onCheckedChange={(value) => update("includeSocial", value === true)}
                        />
                        <Text size="2" color="gray">
                          {deepDesertEnabled ? "Required by Deep Desert" : "Enabled"}
                        </Text>
                      </Flex>
                    </LayoutRow>
                    <LayoutRow label="Deep Desert PvE">
                      <Select.Root
                        value={form.deepDesertPveInstances}
                        onValueChange={(value) => update("deepDesertPveInstances", value)}
                      >
                        <Select.Trigger />
                        <Select.Content>
                          {zeroToOne.map((value) => (
                            <Select.Item key={value} value={value}>
                              {value} {value === "1" ? "instance" : "instances"}
                            </Select.Item>
                          ))}
                        </Select.Content>
                      </Select.Root>
                    </LayoutRow>
                    <LayoutRow label="Deep Desert PvP">
                      <Select.Root
                        value={form.deepDesertPvpInstances}
                        onValueChange={(value) => update("deepDesertPvpInstances", value)}
                      >
                        <Select.Trigger />
                        <Select.Content>
                          {zeroToOne.map((value) => (
                            <Select.Item key={value} value={value}>
                              {value} {value === "1" ? "instance" : "instances"}
                            </Select.Item>
                          ))}
                        </Select.Content>
                      </Select.Root>
                    </LayoutRow>
                    <LayoutRow label="Warm Deep Desert Instances">
                      <Select.Root
                        value={form.deepDesertWarmServers}
                        onValueChange={(value) => update("deepDesertWarmServers", value)}
                      >
                        <Select.Trigger />
                        <Select.Content>
                          {warmOptions.map((value: string) => (
                            <Select.Item key={value} value={value}>
                              {value === "0" ? "0, on demand" : `${value} warm`}
                            </Select.Item>
                          ))}
                        </Select.Content>
                      </Select.Root>
                    </LayoutRow>
                  </Flex>
                </SetupSection>

                <Box
                  className={[
                    "memory-calculation",
                    form.setupTarget === "ubuntu" ? "setup-order-layout-ubuntu" : "setup-order-layout",
                    flowDetectionReady ? "" : "is-flow-disabled",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                >
                  <Flex align="start" justify="between" gap="4" mb="2">
                    <Box>
                      <Text as="div" size="2" weight="medium">
                        Required memory
                      </Text>
                      <Text as="div" size="2" color="gray">
                        Derived from the selected world layout.
                      </Text>
                    </Box>
                    <Text size="7" weight="bold" color="bronze">
                      {calculatedMemory.gb} GB
                    </Text>
                  </Flex>
                  <InlineRequirement
                    ok={requirements.memoryOk}
                    text={`${requirements.memoryRequired}; ${requirements.memoryAvailable}`}
                  />
                  <Separator size="4" my="3" />

                  {/* Visual memory gauge progress meter */}
                  <VisualMemoryGauge
                    requiredGb={calculatedMemory.gb}
                    hostAvailableBytes={
                      form.setupTarget === "ubuntu"
                        ? (remotePreflight?.availableMemoryBytes || 0)
                        : form.setupTarget === "proxmox"
                          ? (() => {
                              const node = proxmoxDetection?.nodes.find((n) => n.node === form.proxmoxNode);
                              return node ? Math.max(0, node.maxmem - node.mem) : 0;
                            })()
                          : (hostReadiness?.availablePhysicalMemoryBytes || 0)
                    }
                    enableSwap={form.enableSwap}
                    plannedSwapGb={form.enableSwap ? (calculatedMemory.gb > 16 ? 8 : 4) : 0}
                  />

                  <Separator size="4" my="3" />
                  <Flex direction="column" gap="1" mb="3">
                    {calculatedMemory.lines.map((line) => (
                      <Text key={line} size="2" color="gray">
                        {line}
                      </Text>
                    ))}
                  </Flex>
                  {form.setupTarget !== "ubuntu" ? (
                    <>
                      <Separator size="4" my="3" />
                      <FormRow label="VM Memory">
                        <TextField.Root
                          value={String(vmMemoryGb)}
                          onChange={(event) => update("vmMemoryGb", event.target.value)}
                        >
                          <TextField.Slot side="right">GB</TextField.Slot>
                        </TextField.Root>
                        <Text as="div" size="2" color="gray" mt="2">
                          {proxmoxMemoryLimitText(form, calculatedMemory, proxmoxDetection)}
                        </Text>
                      </FormRow>
                      <FormRow label="CPU Cores">
                        <TextField.Root
                          value={String(effectiveProcessorCount(form))}
                          onChange={(event) => update("processorCount", event.target.value)}
                        />
                        <InlineRequirement
                          ok={requirements.processorOk}
                          text={`${requirements.processorRequired}; ${requirements.processorAvailable}`}
                        />
                      </FormRow>
                      {form.setupTarget === "proxmox" ? (
                        <>
                          <Separator size="4" my="3" />
                          <Flex align="center" justify="between" gap="3">
                            <Box>
                              <Text as="div" size="2" weight="medium">
                                Experimental guest swap
                              </Text>
                              <Text as="div" size="2" color="gray">
                                Enable the existing Alpine swap profile after bootstrap.
                              </Text>
                            </Box>
                            <Switch checked={form.enableSwap} onCheckedChange={(value) => update("enableSwap", value)} />
                          </Flex>
                          <Separator size="4" my="3" />
                          <Flex align="center" justify="between" gap="3">
                            <Box>
                              <Text as="div" size="2" weight="medium">
                                QEMU guest agent
                              </Text>
                              <Text as="div" size="2" color="gray">
                                Install and start qemu-guest-agent inside the Proxmox Alpine VM.
                              </Text>
                            </Box>
                            <Switch
                              checked={form.proxmoxInstallQemuGuestAgent}
                              onCheckedChange={(value) => update("proxmoxInstallQemuGuestAgent", value)}
                            />
                          </Flex>
                        </>
                      ) : null}
                    </>
                  ) : (
                    <>
                      <Separator size="4" my="3" />
                      <Flex align="center" justify="between" gap="3">
                        <Box>
                          <Text as="div" size="2" weight="medium">
                            Native Ubuntu swap
                          </Text>
                          <Text as="div" size="2" color="gray">
                            Create a swapfile during setup when the host memory is below the selected layout.
                          </Text>
                        </Box>
                        <Switch checked={form.enableSwap} onCheckedChange={(value) => update("enableSwap", value)} />
                      </Flex>
                      <UbuntuSwapNotice
                        calculatedMemory={calculatedMemory}
                        preflight={remotePreflight}
                        enabled={form.enableSwap}
                      />
                    </>
                  )}
                </Box>
              </>
            )}

            {/* STEP 3: Networking */}
            {setupStep === "network" && (
              <>
                {form.setupTarget === "hyperv" ? (
                  <SetupSection icon={MixIcon} title="Network" className="setup-order-network" disabled={!hypervDetectionReady}>
                    {networkDetection !== "ready" ? (
                      <Box className="setup-guide" mb="3">
                        <Flex direction="column" gap="2">
                          <SetupWarningPills warnings={["Local detection required"]} />
                          <Text size="2" color="gray">
                            The app needs host adapter, switch, gateway, and subnet details before it can safely create the VM network.
                          </Text>
                        </Flex>
                      </Box>
                    ) : networkAdapters.length === 0 ? (
                      <Box className="setup-guide" mb="3">
                        <Flex direction="column" gap="2">
                          <SetupWarningPills warnings={["No supported adapter detected"]} />
                          <Text size="2" color="gray">
                            Setup cannot continue until an active physical IPv4 adapter with a gateway is available.
                          </Text>
                        </Flex>
                      </Box>
                    ) : null}
                    <FormRow label="Network mode">
                      <Select.Root
                        value={form.networkMode}
                        onValueChange={(value) => update("networkMode", value as NetworkMode)}
                      >
                        <Select.Trigger />
                        <Select.Content>
                          <Select.Item value="static">Static internal IP</Select.Item>
                          <Select.Item value="dhcp">DHCP</Select.Item>
                        </Select.Content>
                      </Select.Root>
                    </FormRow>
                    <FormRow label="Host network adapter">
                      <Select.Root
                        disabled={networkDetection !== "ready" || networkAdapters.length === 0}
                        value={form.adapterName}
                        onValueChange={(value) => {
                          const adapter = networkAdapters.find((candidate) => candidate.name === value);
                          if (!adapter) return;
                          update("adapterName", value);
                          update("switchName", adapter.existingExternalSwitch || defaultHyperVSwitchName);
                          update("staticIp", adapter.suggestedIpv4Address);
                          update(
                              "playerIp",
                              form.playerIpMode === "external" && externalIp ? externalIp : adapter.suggestedIpv4Address,
                          );
                          update("gateway", adapter.gateway);
                        }}
                      >
                        <Select.Trigger placeholder={networkStatusLabel(networkDetection)} />
                        <Select.Content>
                          {networkAdapters.map((adapter) => (
                            <Select.Item key={adapter.name} value={adapter.name}>
                              {adapter.name} - {adapter.ipv4Address}/{adapter.prefixLength}
                            </Select.Item>
                          ))}
                        </Select.Content>
                      </Select.Root>
                    </FormRow>
                    <FormRow label="Hyper-V switch">
                      <TextField.Root
                        placeholder="Detected from adapter"
                        value={form.switchName}
                        onChange={(event) => update("switchName", event.target.value)}
                      />
                    </FormRow>
                    <Grid columns="3" gap="3">
                      <FormRow label="VM IP">
                        <TextField.Root
                          placeholder="Detected suggestion"
                          value={form.staticIp}
                          onChange={(event) => update("staticIp", event.target.value)}
                        />
                      </FormRow>
                      <FormRow label="Gateway">
                        <TextField.Root
                          placeholder="Detected gateway"
                          value={form.gateway}
                          onChange={(event) => update("gateway", event.target.value)}
                        />
                      </FormRow>
                      <FormRow label="DNS">
                        <TextField.Root value={form.dns} onChange={(event) => update("dns", event.target.value)} />
                      </FormRow>
                    </Grid>
                    <FormRow label="Player-facing IP">
                      <Grid columns="160px 1fr" gap="3">
                        <Select.Root
                          value={form.playerIpMode}
                          onValueChange={(value) => {
                            const mode = value as PlayerIpMode;
                            update("playerIpMode", mode);
                            update("playerIp", mode === "external" ? externalIp || "" : form.staticIp);
                          }}
                        >
                          <Select.Trigger />
                          <Select.Content>
                            <Select.Item value="local">Local IP</Select.Item>
                            <Select.Item value="external">External IP</Select.Item>
                          </Select.Content>
                        </Select.Root>
                        <TextField.Root
                          placeholder={form.playerIpMode === "external" ? "Detected external IP" : "Same as VM IP for LAN"}
                          value={form.playerIp}
                          onChange={(event) => update("playerIp", event.target.value)}
                        />
                      </Grid>
                    </FormRow>
                    {form.playerIpMode === "external" ? <PortForwardingNotice /> : null}
                  </SetupSection>
                ) : form.setupTarget === "ubuntu" ? (
                  <SetupSection icon={MixIcon} title="Network" className="setup-order-network" disabled={!ubuntuDetectionReady}>
                    <FormRow label="Player-facing IP">
                      <Grid columns="160px 1fr" gap="3">
                        <Select.Root
                          value={form.playerIpMode}
                          onValueChange={(value) => {
                            const mode = value as PlayerIpMode;
                            update("playerIpMode", mode);
                            update(
                              "playerIp",
                              mode === "external"
                                ? remotePreflight?.publicIp || form.remoteHost
                                : remotePreflight?.ipv4Addresses[0] || form.remoteHost,
                            );
                          }}
                        >
                          <Select.Trigger />
                          <Select.Content>
                            <Select.Item value="local">Local IP</Select.Item>
                            <Select.Item value="external">External IP</Select.Item>
                          </Select.Content>
                        </Select.Root>
                        <TextField.Root
                          placeholder="Address players use to connect"
                          value={form.playerIp}
                          onChange={(event) => update("playerIp", event.target.value)}
                        />
                      </Grid>
                    </FormRow>
                    {form.playerIpMode === "external" ? <PortForwardingNotice /> : null}
                  </SetupSection>
                ) : (
                  <SetupSection icon={MixIcon} title="Network" className="setup-order-network" disabled={!proxmoxDetectionReady}>
                    <FormRow label="Guest network mode">
                      <TextField.Root value="DHCP First Static Internal IP" readOnly />
                    </FormRow>
                    <Grid columns="3" gap="3">
                      <FormRow label="Static IP">
                        <TextField.Root
                          placeholder="Required"
                          value={form.staticIp}
                          onChange={(event) => update("staticIp", event.target.value)}
                        />
                      </FormRow>
                      <FormRow label="Gateway">
                        <TextField.Root
                          placeholder="Required"
                          value={form.gateway}
                          onChange={(event) => update("gateway", event.target.value)}
                        />
                      </FormRow>
                      <FormRow label="DNS">
                        <TextField.Root
                          placeholder="Required"
                          value={form.dns}
                          onChange={(event) => update("dns", event.target.value)}
                        />
                      </FormRow>
                    </Grid>
                    <FormRow label="Player-facing IP">
                      <Grid columns="160px 1fr" gap="3">
                        <Select.Root
                          value={form.playerIpMode}
                          onValueChange={(value) => {
                            const mode = value as PlayerIpMode;
                            update("playerIpMode", mode);
                            update("playerIp", mode === "local" ? form.staticIp : form.playerIp);
                          }}
                        >
                          <Select.Trigger />
                          <Select.Content>
                            <Select.Item value="local">Local IP</Select.Item>
                            <Select.Item value="external">External IP</Select.Item>
                          </Select.Content>
                        </Select.Root>
                        <TextField.Root
                          placeholder="Address players use to connect"
                          value={form.playerIp}
                          onChange={(event) => update("playerIp", event.target.value)}
                        />
                      </Grid>
                    </FormRow>
                    {form.playerIpMode === "external" ? <PortForwardingNotice /> : null}
                  </SetupSection>
                )}
              </>
            )}

            {/* STEP 4: Review & Deploy */}
            {setupStep === "review" && (
              <SetupSection icon={CubeIcon} title="Visual Pre-flight Review">
                <Grid columns="2" gap="4">
                  <Box>
                    <Text size="1" color="gray" weight="medium">TARGET PLATFORM</Text>
                    <Text size="2" weight="bold" as="div" mt="1" style={{ textTransform: "capitalize" }}>
                      {form.setupTarget === "hyperv" ? "Local Windows Hyper-V" : form.setupTarget === "ubuntu" ? "Remote Ubuntu VPS" : "Proxmox VE Cluster"}
                    </Text>
                  </Box>
                  <Box>
                    <Text size="1" color="gray" weight="medium">HOST LOCATION / VM NAME</Text>
                    <Text size="2" weight="bold" as="div" mt="1">
                      {form.setupTarget === "hyperv" ? form.vmName : form.setupTarget === "ubuntu" ? form.remoteHost : `${form.proxmoxNode} (VMID: ${form.proxmoxVmid})`}
                    </Text>
                  </Box>
                  <Box>
                    <Text size="1" color="gray" weight="medium">WORLD / REGION</Text>
                    <Text size="2" weight="bold" as="div" mt="1">
                      {form.worldName || "Untitled"} ({form.region})
                    </Text>
                  </Box>
                  <Box>
                    <Text size="1" color="gray" weight="medium">RESOURCES ALLOCATION</Text>
                    <Text size="2" weight="bold" as="div" mt="1">
                      {vmMemoryGb} GB Ram / {effectiveProcessorCount(form)} Cores
                    </Text>
                  </Box>
                  <Box>
                    <Text size="1" color="gray" weight="medium">MAP LAYOUT INSTANCES</Text>
                    <Text size="2" weight="bold" as="div" mt="1">
                      Hagga Basin: {form.survivalInstances}, Social Hubs: {form.includeSocial || deepDesertEnabled ? "Yes" : "No"}, Deep Desert: {layoutPreview.deepDesertTotal}
                    </Text>
                  </Box>
                  <Box>
                    <Text size="1" color="gray" weight="medium">PLAYER-FACING IP ADDRESS</Text>
                    <Text size="2" weight="bold" as="div" mt="1">
                      {form.playerIp || "Not configured"} ({form.playerIpMode === "external" ? "External Public" : "Local LAN"})
                    </Text>
                  </Box>
                </Grid>
              </SetupSection>
            )}

          </Flex>
        </Box>

        <Separator size="4" />

        {/* Global Wizard Footer: Issues List & Navigation Buttons */}
        <Flex align="center" justify="between" gap="3" wrap="wrap">
          <Box className="setup-readiness" style={{ flexGrow: 1, minWidth: "200px" }}>
            {setupRunning ? null : canStart ? (
              <Text size="2" color="green" weight="medium" style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <span className="glow-indicator-running" style={{ width: "6px", height: "6px", display: "inline-block", backgroundColor: "#4CAF50", borderRadius: "50%" }} />
                Ready to create one full setup plan.
              </Text>
            ) : packageBlocksSetup && visibleSetupIssues.length === 0 ? (
              <Text size="2" color="amber" weight="medium">
                Resolve the server package update before setup can continue.
              </Text>
            ) : (
              <ul className="setup-issues" style={{ margin: 0, paddingLeft: "16px", color: "var(--red-9)", fontSize: "12px" }}>
                {visibleSetupIssues.map((issue) => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            )}
          </Box>

          <Flex gap="2">
            {setupStep !== "target" ? (
              <Button
                type="button"
                size="2"
                variant="surface"
                color="gray"
                onClick={() => {
                  const steps: Array<"target" | "config" | "network" | "review"> = ["target", "config", "network", "review"];
                  const prevIndex = steps.indexOf(setupStep) - 1;
                  setSetupStep(steps[prevIndex]);
                }}
              >
                Back
              </Button>
            ) : null}

            {setupStep !== "review" ? (
              <Button
                type="button"
                size="2"
                variant="solid"
                color="bronze"
                disabled={setupStep === "target" && !flowDetectionReady}
                onClick={() => {
                  const steps: Array<"target" | "config" | "network" | "review"> = ["target", "config", "network", "review"];
                  const nextIndex = steps.indexOf(setupStep) + 1;
                  setSetupStep(steps[nextIndex]);
                }}
              >
                Next Step
              </Button>
            ) : (
              <Button size="3" onClick={onStart} disabled={!canStart || setupRunning}>
                <LightningBoltIcon /> {setupRunning ? "Setup running..." : "Start full setup"}
              </Button>
            )}
          </Flex>
        </Flex>
      </Flex>
    </Card>
  );
}
