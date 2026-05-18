import { Box, Flex, Text, Badge, Button } from "@radix-ui/themes";
import { type RemoteServerComponent } from "../../types";
import {
  componentLogStateKey,
  isCriticalRestartComponent,
  copyTextToClipboard
} from "../../utils/storage";

export function ComponentHealthGroup({
  title,
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: {
  title: string;
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog?: (component: RemoteServerComponent) => void;
  onRestart?: (component: RemoteServerComponent) => void;
}) {
  if (components.length === 0) return null;
  return (
    <details className="component-group" style={{ width: "100%" }}>
      <summary className="component-group-summary" style={{ listStyle: "none", cursor: "pointer", padding: "6px 0" }}>
        <Flex align="center" justify="between" gap="2">
          <Text size="1" weight="medium" color="gray" className="component-group-title">
            {title}
          </Text>
          <Badge color="gray" variant="soft">
            {components.length}
          </Badge>
        </Flex>
      </summary>
      <Flex direction="column" gap="2" mt="2">
        {components.map((component) => {
          const logKey = componentLogStateKey(serverKey, component);
          const logText = logs[logKey];
          const busy = !!logBusy[logKey];
          const restarting = !!restartBusy[logKey];
          return (
            <details key={`${component.logKey}-${component.name}`} className="component-row" style={{ listStyle: "none", border: "1px solid rgba(255,255,255,0.05)", borderRadius: "4px", overflow: "hidden", marginBottom: "4px" }}>
              <summary className="component-summary" style={{ listStyle: "none", cursor: "pointer", padding: "8px", background: "rgba(255,255,255,0.02)" }}>
                <Flex align="center" justify="between" gap="3" width="100%">
                  <Box minWidth="0">
                    <Flex align="center" gap="2" wrap="wrap">
                      <Text size="2" weight="medium">
                        {component.name}
                      </Text>
                      <Badge color={component.tone} variant="soft">
                        {component.state}
                      </Badge>
                    </Flex>
                    <Text as="div" size="2" color="gray" className="component-summary-text">
                      {component.summary}
                    </Text>
                  </Box>
                  <Flex gap="2" style={{ flexShrink: 0 }}>
                    <Button
                      type="button"
                      size="1"
                      variant="surface"
                      disabled={busy || restarting}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const row = event.currentTarget.closest("details");
                        if (row) row.open = true;
                        onRefreshLog?.(component);
                      }}
                    >
                      {busy ? "Loading logs" : logText ? "Refresh logs" : "View logs"}
                    </Button>
                    <Button
                      type="button"
                      size="1"
                      color={isCriticalRestartComponent(component) ? "amber" : "bronze"}
                      variant="soft"
                      disabled={busy || restarting}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const row = event.currentTarget.closest("details");
                        if (row) row.open = true;
                        onRestart?.(component);
                      }}
                    >
                      {restarting ? "Restarting" : "Restart"}
                    </Button>
                  </Flex>
                </Flex>
              </summary>
              <Box className="component-body" p="3" style={{ background: "rgba(0,0,0,0.15)", borderTop: "1px solid rgba(255,255,255,0.05)" }}>
                {component.details.length > 0 ? (
                  <ul className="component-details" style={{ margin: 0, paddingLeft: "16px", color: "var(--gray-9)", fontSize: "13px" }}>
                    {component.details.map((detail) => (
                      <li key={detail}>{detail}</li>
                    ))}
                  </ul>
                ) : (
                  <Text as="div" size="1" color="gray">
                    No additional details reported.
                  </Text>
                )}
                {logText ? (
                  <>
                    <Flex justify="end" mt="2">
                      <Button
                        type="button"
                        size="1"
                        variant="soft"
                        onClick={() => void copyTextToClipboard(logText)}
                      >
                        Copy logs
                      </Button>
                    </Flex>
                    <Box className="component-log" mt="2" p="2" style={{ background: "rgba(0,0,0,0.3)", borderRadius: "4px", maxHeight: "150px", overflowY: "auto" }}>
                      {logText.split(/\r?\n/).map((line, index) => (
                        <Text as="div" size="1" className="mono" key={`${component.logKey}-${index}`} style={{ whiteSpace: "pre-wrap" }}>
                          {line || "\u00a0"}
                        </Text>
                      ))}
                    </Box>
                  </>
                ) : null}
              </Box>
            </details>
          );
        })}
      </Flex>
    </details>
  );
}

export function ComponentHealthList({
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: {
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog?: (component: RemoteServerComponent) => void;
  onRestart?: (component: RemoteServerComponent) => void;
}) {
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
