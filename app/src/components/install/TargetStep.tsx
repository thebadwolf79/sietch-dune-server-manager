import { Box, Flex, Grid, Select, Text, Button, TextField, Checkbox, Link } from "@radix-ui/themes";
import { DesktopIcon } from "@radix-ui/react-icons";
import { open } from "@tauri-apps/plugin-dialog";
import {
  type SetupForm,
  type HostReadiness,
  type DriveCandidate,
  type DetectionState,
  type EnvironmentGate,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type ServerPackageStatus,
  type ServerPackageCheckStatus,
  type SetupTarget,
  type SetupRequirements
} from "../../types";
import { SetupSection, FormRow, InlineRequirement } from "../Common";
import { LocalResourceSummary, RemotePreflightSummary, ProxmoxDetectionSummary, UbuntuSetupGuide } from "./ResourceSummaries";
import { parsePositiveInt } from "../../utils/helpers";

export function TargetStep({
  form,
  hostReadiness,
  driveCandidates,
  networkDetection,
  environmentGate,
  requirements,
  vmDestinationHasVm,
  remotePreflight,
  remotePreflightStatus,
  proxmoxDetection,
  proxmoxDetectionStatus,
  serverPackageStatus,
  serverPackageCheckStatus,
  setupRunning,
  update,
  onLocalDetection,
  onRemotePreflight,
  onProxmoxDetection,
  onUpdateServerPackage,
}: {
  form: SetupForm;
  hostReadiness: HostReadiness | null;
  driveCandidates: DriveCandidate[];
  networkDetection: DetectionState;
  environmentGate: EnvironmentGate;
  requirements: SetupRequirements;
  vmDestinationHasVm: boolean;
  remotePreflight: UbuntuSshPreflight | null;
  remotePreflightStatus: DetectionState;
  proxmoxDetection: ProxmoxDetection | null;
  proxmoxDetectionStatus: DetectionState;
  serverPackageStatus: ServerPackageStatus | null;
  serverPackageCheckStatus: ServerPackageCheckStatus;
  setupRunning: boolean;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
  onLocalDetection: () => void;
  onRemotePreflight: () => void;
  onProxmoxDetection: () => void;
  onUpdateServerPackage: () => void;
}) {
  const hypervDetectionReady = networkDetection === "ready" && environmentGate.canContinue;
  const proxmoxDetectionReady = proxmoxDetectionStatus === "ready" && !!proxmoxDetection;

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

  return (
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
  );
}
