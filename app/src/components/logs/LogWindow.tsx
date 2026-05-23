import { useLayoutEffect, useRef } from "react";
import { Box, Button, Card, Flex, Grid, Select, Text } from "@radix-ui/themes";
import { ChevronDownIcon, ChevronUpIcon } from "@radix-ui/react-icons";

import type { LogLevelFilter, LogRow } from "../../types/log";

export type LogWindowProps = {
  rows: LogRow[];
  level: LogLevelFilter;
  collapsed: boolean;
  onLevelChange: (level: LogLevelFilter) => void;
  onClear: () => void;
  onToggleCollapsed: () => void;
};

export default function LogWindow({
  rows,
  level,
  collapsed,
  onLevelChange,
  onClear,
  onToggleCollapsed,
}: LogWindowProps) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    if (stickToBottomRef.current) {
      body.scrollTop = body.scrollHeight;
    }
  }, [rows]);
  return (
    <Card size="3" variant="surface" className={`pane log-pane${collapsed ? " is-collapsed" : ""}`}>
      <Flex direction="column" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3" mb={collapsed ? "0" : "3"}>
          <Box minWidth="0">
            <Text as="div" size="2" weight="medium">
              Logs
            </Text>
            <Text as="div" size="1" color="gray">
              {rows.length} entries
            </Text>
          </Box>
          <Flex align="center" gap="2">
            {collapsed ? null : (
              <>
                <Select.Root value={level} onValueChange={(value) => onLevelChange(value as LogLevelFilter)}>
                  <Select.Trigger aria-label="Minimum log level" />
                  <Select.Content>
                    <Select.Item value="debug">Debug</Select.Item>
                    <Select.Item value="info">Info</Select.Item>
                    <Select.Item value="warn">Warn</Select.Item>
                    <Select.Item value="error">Error</Select.Item>
                  </Select.Content>
                </Select.Root>
                <Button type="button" size="1" variant="surface" disabled={rows.length === 0} onClick={onClear}>
                  Clear
                </Button>
              </>
            )}
            <Button
              type="button"
              size="1"
              variant="surface"
              aria-label={collapsed ? "Expand logs" : "Collapse logs"}
              onClick={onToggleCollapsed}
            >
              {collapsed ? <ChevronUpIcon /> : <ChevronDownIcon />}
            </Button>
          </Flex>
        </Flex>
        {collapsed ? null : (
          <Box
            className="log-body"
            ref={bodyRef}
            onScroll={(event) => {
              const body = event.currentTarget;
              const distanceFromBottom = body.scrollHeight - body.scrollTop - body.clientHeight;
              stickToBottomRef.current = distanceFromBottom < 80;
            }}
          >
            <Flex direction="column" gap="0">
              {rows.map((row) => (
                <Grid key={row.id} columns="96px 44px 1fr" gap="2" align="center" className={`log-line log-${row.level}`}>
                  <Text color="gray" className="mono log-meta log-text">
                    {row.timestamp}
                  </Text>
                  <Text className="mono log-meta log-level log-text">{row.level}</Text>
                  <Text className="mono log-text">{row.message}</Text>
                </Grid>
              ))}
            </Flex>
          </Box>
        )}
      </Flex>
    </Card>
  );
}
