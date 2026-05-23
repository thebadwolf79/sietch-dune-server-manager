import { Box } from "@radix-ui/themes";

export type BusySpinnerProps = Record<string, never>;

export default function BusySpinner(_props: BusySpinnerProps) {
  return <Box className="inline-spinner" aria-hidden />;
}
