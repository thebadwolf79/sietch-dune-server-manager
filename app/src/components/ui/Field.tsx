import type { ReactNode } from "react";
import { Box, Text } from "@radix-ui/themes";

export type FieldProps = {
  label: string;
  children: ReactNode;
};

export default function Field({ label, children }: FieldProps) {
  return (
    <Box>
      <Text as="label" size="2" weight="medium" mb="1" className="field-label">
        {label}
      </Text>
      {children}
    </Box>
  );
}
