import { Box, Text } from "@radix-ui/themes";

export type MetricProps = {
  label: string;
  value: string;
};

export default function Metric({ label, value }: MetricProps) {
  return (
    <Box className="metric">
      <Text as="div" size="1" color="gray">
        {label}
      </Text>
      <Text as="div" size="2" className="mono metric-value">
        {value}
      </Text>
    </Box>
  );
}
