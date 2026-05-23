import { Badge, Box, Button, Flex, Text } from "@radix-ui/themes";

import type { RemoteServerComponent } from "../../types/server";
import { copyTextToClipboard } from "../../utils/clipboard";
import { componentLogStateKey, isCriticalRestartComponent } from "../../utils/remote-server";

export type ComponentHealthGroupProps = {
  title: string;
  serverKey: string;
  components: RemoteServerComponent[];
  logs: Record<string, string>;
  logBusy: Record<string, boolean>;
  restartBusy: Record<string, boolean>;
  onRefreshLog: (component: RemoteServerComponent) => void;
  onRestart: (component: RemoteServerComponent) => void;
};

export default function ComponentHealthGroup({
  title,
  serverKey,
  components,
  logs,
  logBusy,
  restartBusy,
  onRefreshLog,
  onRestart,
}: ComponentHealthGroupProps) {
  if (components.length === 0) return null;
  return (
    <details className="component-group">
      <summary className="component-group-summary">
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
            <details key={`${component.logKey}-${component.name}`} className="component-row">
              <summary className="component-summary">
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
                        onRefreshLog(component);
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
                        onRestart(component);
                      }}
                    >
                      {restarting ? "Restarting" : "Restart"}
                    </Button>
                  </Flex>
                </Flex>
              </summary>
              <Box className="component-body">
                {component.details.length > 0 ? (
                  <ul className="component-details">
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
                      <Button type="button" size="1" variant="soft" onClick={() => void copyTextToClipboard(logText)}>
                        Copy logs
                      </Button>
                    </Flex>
                    <Box className="component-log" mt="2">
                      {logText.split(/\r?\n/).map((line, index) => (
                        <Text as="div" size="1" className="mono" key={`${component.logKey}-${index}`}>
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
