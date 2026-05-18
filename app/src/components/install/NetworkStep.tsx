import { Box, Flex, Grid, Select, TextField } from "@radix-ui/themes";
import { MixIcon } from "@radix-ui/react-icons";
import {
  type SetupForm,
  type NetworkAdapterCandidate,
  type DetectionState,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type NetworkMode,
  type PlayerIpMode
} from "../../types";
import { SetupSection, FormRow, PortForwardingNotice } from "../Common";
import { SetupWarningPills } from "./ResourceSummaries";
import { networkStatusLabel } from "../../utils/helpers";

export const defaultHyperVSwitchName = "DuneAwakeningServerSwitch";

export function NetworkStep({
  form,
  networkDetection,
  networkAdapters,
  externalIp,
  remotePreflight,
  proxmoxDetection,
  update,
}: {
  form: SetupForm;
  networkDetection: DetectionState;
  networkAdapters: NetworkAdapterCandidate[];
  externalIp: string | null;
  remotePreflight: UbuntuSshPreflight | null;
  proxmoxDetection: ProxmoxDetection | null;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
}) {
  const hypervDetectionReady = networkDetection === "ready";
  const ubuntuDetectionReady = remotePreflight !== null;
  const proxmoxDetectionReady = proxmoxDetection !== null;

  return (
    <>
      {form.setupTarget === "hyperv" ? (
        <SetupSection icon={MixIcon} title="Network" className="setup-order-network" disabled={!hypervDetectionReady}>
          {networkDetection !== "ready" ? (
            <Box className="setup-guide" mb="3">
              <Flex direction="column" gap="2">
                <SetupWarningPills warnings={["Local detection required"]} />
                <Box style={{ fontSize: "13px", color: "var(--gray-11)" }}>
                  The app needs host adapter, switch, gateway, and subnet details before it can safely create the VM network.
                </Box>
              </Flex>
            </Box>
          ) : networkAdapters.length === 0 ? (
            <Box className="setup-guide" mb="3">
              <Flex direction="column" gap="2">
                <SetupWarningPills warnings={["No supported adapter detected"]} />
                <Box style={{ fontSize: "13px", color: "var(--gray-11)" }}>
                  Setup cannot continue until an active physical IPv4 adapter with a gateway is available.
                </Box>
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
          <Grid columns="3" my="2" gap="3">
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
          <Grid columns="3" my="2" gap="3">
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
  );
}
