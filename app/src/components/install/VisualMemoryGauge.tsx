import { Box, Flex, Text } from "@radix-ui/themes";

export function VisualMemoryGauge({
  requiredGb,
  hostAvailableBytes,
  enableSwap,
  plannedSwapGb,
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
