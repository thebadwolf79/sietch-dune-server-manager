import { useLayoutEffect, useRef, useState } from "react";
import {
  Card,
  Flex,
  Box,
  Heading,
  Text,
  SegmentedControl,
  Button,
  Separator
} from "@radix-ui/themes";
import { LightningBoltIcon } from "@radix-ui/react-icons";
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
  type ServerPackageCheckStatus
} from "../types";
import {
  effectiveVmMemoryGb,
  effectiveProxmoxVmMemoryGb
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

// Sub-components decoupling
import { TargetStep } from "./install/TargetStep";
import { LayoutStep } from "./install/LayoutStep";
import { NetworkStep } from "./install/NetworkStep";
import { ReviewStep } from "./install/ReviewStep";

type SetupStep = "target" | "config" | "network" | "review";

const setupSteps: SetupStep[] = ["target", "config", "network", "review"];

export function InstallControls({
  form,
  hostReadiness,
  driveCandidates,
  networkDetection,
  networkAdapters,
  externalIp,
  environmentGate,
  remotePreflight,
  remotePreflightStatus,
  proxmoxDetection,
  proxmoxDetectionStatus,
  serverPackageStatus,
  serverPackageCheckStatus,
  vmDestinationHasVm,
  calculatedMemory,
  layoutPreview,
  setupRunning,
  update,
  onLocalDetection,
  onRemotePreflight,
  onProxmoxDetection,
  onGenerateProxmoxSshKey,
  onUpdateServerPackage,
  onStart,
}: {
  form: SetupForm;
  hostReadiness: HostReadiness | null;
  driveCandidates: DriveCandidate[];
  networkDetection: DetectionState;
  networkAdapters: NetworkAdapterCandidate[];
  externalIp: string | null;
  environmentGate: EnvironmentGate;
  remotePreflight: UbuntuSshPreflight | null;
  remotePreflightStatus: DetectionState;
  proxmoxDetection: ProxmoxDetection | null;
  proxmoxDetectionStatus: DetectionState;
  serverPackageStatus: ServerPackageStatus | null;
  serverPackageCheckStatus: ServerPackageCheckStatus;
  vmDestinationHasVm: boolean;
  calculatedMemory: CalculatedMemory;
  layoutPreview: SetupLayoutPreview;
  setupRunning: boolean;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
  onLocalDetection: () => void;
  onRemotePreflight: () => void;
  onProxmoxDetection: () => void;
  onGenerateProxmoxSshKey: () => void;
  onUpdateServerPackage: () => void;
  onStart: () => void;
}) {
  const [setupStep, setSetupStep] = useState<SetupStep>("target");
  const setupScrollRef = useRef<HTMLDivElement | null>(null);
  const currentSetupStepRef = useRef<SetupStep>(setupStep);
  const setupStepScrollPositions = useRef<Record<SetupStep, number>>({
    target: 0,
    config: 0,
    network: 0,
    review: 0,
  });
  
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

  const rememberSetupScroll = () => {
    const scroller = setupScrollRef.current;
    if (!scroller) return;
    setupStepScrollPositions.current[currentSetupStepRef.current] = scroller.scrollTop;
  };

  const showSetupStep = (nextStep: SetupStep) => {
    if (nextStep !== "target" && !flowDetectionReady) {
      return;
    }
    rememberSetupScroll();
    setSetupStep(nextStep);
  };

  useLayoutEffect(() => {
    currentSetupStepRef.current = setupStep;
    const savedTop = setupStepScrollPositions.current[setupStep] ?? 0;
    if (setupScrollRef.current) {
      setupScrollRef.current.scrollTop = savedTop;
    }
    const animationFrame = window.requestAnimationFrame(() => {
      if (setupScrollRef.current) {
        setupScrollRef.current.scrollTop = savedTop;
      }
    });
    return () => window.cancelAnimationFrame(animationFrame);
  }, [setupStep]);

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
            showSetupStep(value as SetupStep);
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

        <Box
          ref={setupScrollRef}
          className="setup-scroll"
          style={{ flexGrow: 1, overflowY: "auto", paddingRight: "4px" }}
          onScroll={(event) => {
            setupStepScrollPositions.current[currentSetupStepRef.current] = event.currentTarget.scrollTop;
          }}
        >
          <Flex direction="column" gap="5" className={setupRunning ? "setup-controls is-disabled" : "setup-controls"}>
            
            {/* STEP 1: Platform & Detection */}
            {setupStep === "target" && (
              <TargetStep
                form={form}
                hostReadiness={hostReadiness}
                driveCandidates={driveCandidates}
                networkDetection={networkDetection}
                environmentGate={environmentGate}
                requirements={requirements}
                vmDestinationHasVm={vmDestinationHasVm}
                remotePreflight={remotePreflight}
                remotePreflightStatus={remotePreflightStatus}
                proxmoxDetection={proxmoxDetection}
                proxmoxDetectionStatus={proxmoxDetectionStatus}
                serverPackageStatus={serverPackageStatus}
                serverPackageCheckStatus={serverPackageCheckStatus}
                setupRunning={setupRunning}
                update={update}
                onLocalDetection={onLocalDetection}
                onRemotePreflight={onRemotePreflight}
                onProxmoxDetection={onProxmoxDetection}
                onGenerateProxmoxSshKey={onGenerateProxmoxSshKey}
                onUpdateServerPackage={onUpdateServerPackage}
              />
            )}

            {/* STEP 2: World & Layout */}
            {setupStep === "config" && (
              <LayoutStep
                form={form}
                calculatedMemory={calculatedMemory}
                layoutPreview={layoutPreview}
                hostReadiness={hostReadiness}
                remotePreflight={remotePreflight}
                proxmoxDetection={proxmoxDetection}
                requirements={requirements}
                flowDetectionReady={flowDetectionReady}
                update={update}
              />
            )}

            {/* STEP 3: Networking */}
            {setupStep === "network" && (
              <NetworkStep
                form={form}
                networkDetection={networkDetection}
                networkAdapters={networkAdapters}
                externalIp={externalIp}
                remotePreflight={remotePreflight}
                proxmoxDetection={proxmoxDetection}
                update={update}
              />
            )}

            {/* STEP 4: Review & Deploy */}
            {setupStep === "review" && (
              <ReviewStep
                form={form}
                layoutPreview={layoutPreview}
                calculatedMemory={calculatedMemory}
                proxmoxDetection={proxmoxDetection}
              />
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
                  const prevIndex = setupSteps.indexOf(setupStep) - 1;
                  showSetupStep(setupSteps[prevIndex]);
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
                  const nextIndex = setupSteps.indexOf(setupStep) + 1;
                  showSetupStep(setupSteps[nextIndex]);
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
