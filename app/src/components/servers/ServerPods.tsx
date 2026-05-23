import { Text } from "@radix-ui/themes";

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
      <Text size="2" style={{ color: "var(--color-text-muted)" }}>
        No pod information yet. Refresh the server to inventory pods.
      </Text>
    );
  }
  const systems = components.filter((component) => component.category !== "map");
  const maps = components.filter((component) => component.category === "map");
  return (
    <div className="pods-table">
      {systems.length > 0 ? (
        <section>
          <div className="section-title">System pods</div>
          <div className="pod-list">
            {systems.map((component) => {
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
                />
              );
            })}
          </div>
        </section>
      ) : null}
      {maps.length > 0 ? (
        <section>
          <div className="section-title">Map server pods</div>
          <div className="pod-list">
            {maps.map((component) => {
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
                />
              );
            })}
          </div>
        </section>
      ) : null}
    </div>
  );
}
