import { Box, Flex } from "@radix-ui/themes";

import type { RemoteServerComponent } from "../../types/server";
import ComponentHealthGroup from "./ComponentHealthGroup";

export type ComponentHealthListProps = {
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog: (component: RemoteServerComponent) => void;
  onRestart: (component: RemoteServerComponent) => void;
};

export default function ComponentHealthList({
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: ComponentHealthListProps) {
  if (components.length === 0) return null;
  const systems = components.filter((component) => component.category !== "map");
  const maps = components.filter((component) => component.category === "map");
  return (
    <Box className="component-health" mt="3">
      <Flex direction="column" gap="3">
        <ComponentHealthGroup
          title="Systems"
          serverKey={serverKey}
          components={systems}
          logs={logs}
          logBusy={logBusy}
          restartBusy={restartBusy}
          onRefreshLog={onRefreshLog}
          onRestart={onRestart}
        />
        <ComponentHealthGroup
          title="Maps"
          serverKey={serverKey}
          components={maps}
          logs={logs}
          logBusy={logBusy}
          restartBusy={restartBusy}
          onRefreshLog={onRefreshLog}
          onRestart={onRestart}
        />
      </Flex>
    </Box>
  );
}
