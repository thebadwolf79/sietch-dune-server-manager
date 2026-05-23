import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Box, Flex, Grid, Select, Text, Tooltip } from "@radix-ui/themes";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  FilePlusIcon,
  TrashIcon,
} from "@radix-ui/react-icons";

import { getLogsFolder, openLogsFolder } from "../../services/tauri";
import type { LogLevelFilter, LogRow } from "../../types/log";

export type LogWindowProps = {
  rows: LogRow[];
  level: LogLevelFilter;
  collapsed: boolean;
  scopedToServer: boolean;
  canScopeToServer: boolean;
  onLevelChange: (level: LogLevelFilter) => void;
  onClear: () => void;
  onToggleCollapsed: () => void;
  onToggleScope: (next: boolean) => void;
};

export default function LogWindow({
  rows,
  level,
  collapsed,
  scopedToServer,
  canScopeToServer,
  onLevelChange,
  onClear,
  onToggleCollapsed,
  onToggleScope,
}: LogWindowProps) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);
  const [logsFolder, setLogsFolder] = useState<string>("");

  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body) return;
    if (stickToBottomRef.current) {
      body.scrollTop = body.scrollHeight;
    }
  }, [rows]);

  useEffect(() => {
    void getLogsFolder()
      .then(setLogsFolder)
      .catch(() => undefined);
  }, []);

  const latestLevel = rows.length > 0 ? rows[rows.length - 1].level : "info";

  if (collapsed) {
    return (
      <aside className="log-sidebar" data-collapsed="true">
        <Tooltip content="Expand logs">
          <button
            type="button"
            className="log-sidebar-toggle"
            aria-label="Expand logs"
            onClick={onToggleCollapsed}
          >
            <ChevronLeftIcon />
          </button>
        </Tooltip>
        <div className="log-sidebar-rail">
          <span className="log-sidebar-rail-label">LOGS</span>
          <span className="log-sidebar-rail-count">{rows.length}</span>
          <span className={`log-sidebar-rail-dot log-${latestLevel}`} aria-hidden />
        </div>
      </aside>
    );
  }

  return (
    <aside className="log-sidebar" data-collapsed="false">
      <Flex direction="column" height="100%" minHeight="0" gap="2" p="3">
        <Flex align="center" justify="between" gap="2" wrap="wrap">
          <Tooltip content={logsFolder ? `Persisted at ${logsFolder}` : "Operation log"}>
            <Box>
              <Text as="div" size="2" weight="medium">
                Logs
              </Text>
              <Text as="div" size="1" style={{ color: "var(--color-text-muted)" }}>
                {rows.length} entries
              </Text>
            </Box>
          </Tooltip>
          <Tooltip content="Collapse logs">
            <button
              type="button"
              className="log-sidebar-toggle"
              aria-label="Collapse logs"
              onClick={onToggleCollapsed}
            >
              <ChevronRightIcon />
            </button>
          </Tooltip>
        </Flex>

        <Flex align="center" gap="2" wrap="wrap">
          <Select.Root size="1" value={level} onValueChange={(value) => onLevelChange(value as LogLevelFilter)}>
            <Select.Trigger variant="surface" aria-label="Minimum log level" />
            <Select.Content>
              <Select.Item value="debug">Debug</Select.Item>
              <Select.Item value="info">Info</Select.Item>
              <Select.Item value="warn">Warn</Select.Item>
              <Select.Item value="error">Error</Select.Item>
            </Select.Content>
          </Select.Root>
          {canScopeToServer ? (
            <Tooltip
              content={
                scopedToServer
                  ? "Showing rows for the active server only. Click to show all."
                  : "Showing all rows. Click to scope to the active server."
              }
            >
              <button
                type="button"
                className="log-scope-toggle"
                data-scoped={scopedToServer ? "true" : "false"}
                aria-pressed={scopedToServer}
                onClick={() => onToggleScope(!scopedToServer)}
              >
                {scopedToServer ? "This server" : "All"}
              </button>
            </Tooltip>
          ) : null}
          <div style={{ flex: 1 }} />
          <Tooltip content="Open logs folder">
            <button
              type="button"
              className="log-sidebar-icon-btn"
              aria-label="Open logs folder"
              onClick={() => void openLogsFolder().catch(() => undefined)}
            >
              <FilePlusIcon />
            </button>
          </Tooltip>
          <Tooltip content="Clear in-memory log">
            <button
              type="button"
              className="log-sidebar-icon-btn"
              aria-label="Clear logs"
              disabled={rows.length === 0}
              onClick={onClear}
            >
              <TrashIcon />
            </button>
          </Tooltip>
        </Flex>

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
              <Grid
                key={row.id}
                columns="68px 44px 1fr"
                gap="2"
                align="baseline"
                className={`log-line log-${row.level}`}
              >
                <Text className="log-meta mono">{row.timestamp}</Text>
                <Text className="log-meta log-level mono">{row.level}</Text>
                <Text className="log-text mono">{row.message}</Text>
              </Grid>
            ))}
          </Flex>
        </Box>
      </Flex>
    </aside>
  );
}
