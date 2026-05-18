import { Box, Grid, Text } from "@radix-ui/themes";
import { CubeIcon } from "@radix-ui/react-icons";
import {
  type SetupForm,
  type SetupLayoutPreview,
  type CalculatedMemory,
  type ProxmoxDetection
} from "../../types";
import { SetupSection } from "../Common";
import { effectiveVmMemoryGb, effectiveProxmoxVmMemoryGb, effectiveProcessorCount } from "../../utils/memory";

export function ReviewStep({
  form,
  layoutPreview,
  calculatedMemory,
  proxmoxDetection,
}: {
  form: SetupForm;
  layoutPreview: SetupLayoutPreview;
  calculatedMemory: CalculatedMemory;
  proxmoxDetection: ProxmoxDetection | null;
}) {
  const vmMemoryGb =
    form.setupTarget === "proxmox"
      ? effectiveProxmoxVmMemoryGb(form, calculatedMemory, proxmoxDetection)
      : effectiveVmMemoryGb(form, calculatedMemory);

  const deepDesertEnabled = layoutPreview.deepDesertTotal > 0;

  return (
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
  );
}
