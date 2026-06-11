import { Box, Flex, Text } from "@radix-ui/themes";

import type { RemoteServerComponent } from "../../types/server";
import { componentLogStateKey } from "../../utils/remote-server";
import ServerPodRow from "./ServerPodRow";

export type ServerPodsProps = {
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog: (component: RemoteServerComponent) => void;
  onRestart: (component: RemoteServerComponent) => void;
};

/**
 * Per-server Pods sub-tab: flat table of pod components grouped by
 * category (systems first, then maps). Each row expands to show details,
 * a live log tail, and per-pod restart/refresh actions.
 */
export default function ServerPods({
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: ServerPodsProps) {
  if (components.length === 0) {
    return (
      <Box
        className="bracket chamfer"
        p="4"
        mt="3"
        style={{
          background: "var(--color-bg-panel)",
          border: "1px solid var(--color-border-hair)",
          borderRadius: "var(--radius-3)",
        }}
      >
        <Text size="2" style={{ color: "var(--color-text-muted)" }}>
          No pod information yet. Refresh the server to inventory pods.
        </Text>
      </Box>
    );
  }
  const systems = components.filter((component) => component.category !== "map");
  const maps = components.filter((component) => component.category === "map");
  return (
    <Flex direction="column" gap="4" mt="3">
      {systems.length > 0 ? (
        <div>
          <Text size="2" weight="bold" className="font-display" mb="2" style={{ display: "block", color: "var(--color-text-primary)" }}>
            System pods
          </Text>
          <Box
            className="bracket chamfer"
            style={{
              background: "var(--color-bg-panel)",
              border: "1px solid var(--color-border-hair)",
              borderRadius: "var(--radius-3)",
              overflow: "hidden",
              display: "flex",
              flexDirection: "column",
            }}
          >
            {systems.map((component, index) => {
              const key = componentLogStateKey(serverKey, component);
              return (
                <ServerPodRow
                  key={component.logKey}
                  component={component}
                  logKey={key}
                  logText={logs[key]}
                  logBusy={!!logBusy[key]}
                  restartBusy={!!restartBusy[key]}
                  onRefreshLog={() => onRefreshLog(component)}
                  onRestart={() => onRestart(component)}
                  isLast={index === systems.length - 1}
                />
              );
            })}
          </Box>
        </div>
      ) : null}
      {maps.length > 0 ? (
        <div>
          <Text size="2" weight="bold" className="font-display" mb="2" style={{ display: "block", color: "var(--color-text-primary)" }}>
            Map server pods
          </Text>
          <Box
            className="bracket chamfer"
            style={{
              background: "var(--color-bg-panel)",
              border: "1px solid var(--color-border-hair)",
              borderRadius: "var(--radius-3)",
              overflow: "hidden",
              display: "flex",
              flexDirection: "column",
            }}
          >
            {maps.map((component, index) => {
              const key = componentLogStateKey(serverKey, component);
              return (
                <ServerPodRow
                  key={component.logKey}
                  component={component}
                  logKey={key}
                  logText={logs[key]}
                  logBusy={!!logBusy[key]}
                  restartBusy={!!restartBusy[key]}
                  onRefreshLog={() => onRefreshLog(component)}
                  onRestart={() => onRestart(component)}
                  isLast={index === maps.length - 1}
                />
              );
            })}
          </Box>
        </div>
      ) : null}
    </Flex>
  );
}

