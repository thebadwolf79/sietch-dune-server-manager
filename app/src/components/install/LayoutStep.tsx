import { Box, Flex, Grid, Select, Text, TextField, Checkbox, TextArea, Link, Separator, Switch } from "@radix-ui/themes";
import { GlobeIcon, RocketIcon } from "@radix-ui/react-icons";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { ChangeEvent } from "react";
import {
  type SetupForm,
  type CalculatedMemory,
  type SetupLayoutPreview,
  type HostReadiness,
  type UbuntuSshPreflight,
  type ProxmoxDetection,
  type SetupRequirements
} from "../../types";
import { SetupSection, FormRow, LayoutRow, InlineRequirement } from "../Common";
import { VisualMemoryGauge } from "./VisualMemoryGauge";
import { UbuntuSwapNotice } from "./ResourceSummaries";
import { oneToFour, zeroToOne, zeroTo } from "../../utils/helpers";
import {
  effectiveVmMemoryGb,
  effectiveProxmoxVmMemoryGb,
  effectiveProcessorCount,
  proxmoxMemoryLimitText
} from "../../utils/memory";

export function LayoutStep({
  form,
  calculatedMemory,
  layoutPreview,
  hostReadiness,
  remotePreflight,
  proxmoxDetection,
  requirements,
  flowDetectionReady,
  update,
}: {
  form: SetupForm;
  calculatedMemory: CalculatedMemory;
  layoutPreview: SetupLayoutPreview;
  hostReadiness: HostReadiness | null;
  remotePreflight: UbuntuSshPreflight | null;
  proxmoxDetection: ProxmoxDetection | null;
  requirements: SetupRequirements;
  flowDetectionReady: boolean;
  update: <K extends keyof SetupForm>(key: K, value: SetupForm[K]) => void;
}) {
  const deepDesertEnabled = layoutPreview.deepDesertTotal > 0;
  const warmOptions = zeroTo(layoutPreview.deepDesertTotal);
  const vmMemoryGb =
    form.setupTarget === "proxmox"
      ? effectiveProxmoxVmMemoryGb(form, calculatedMemory, proxmoxDetection)
      : effectiveVmMemoryGb(form, calculatedMemory);

  return (
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
  );
}
