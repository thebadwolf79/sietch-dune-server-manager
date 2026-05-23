import { Box, Flex, Heading, Text } from "@radix-ui/themes";

import type { RemoteServerRecord, RemoteServerStatus } from "../../types/server";
import { remoteServerDefaultUser, resolveServerStatus } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import EmptyState from "../ui/EmptyState";
import StatusPill from "../ui/StatusPill";

export type ServersListPageProps = {
  servers: RemoteServerRecord[];
  statuses: Record<string, RemoteServerStatus>;
  statusErrors: Record<string, string>;
  busyMap: Record<string, string>;
  onOpenServer: (serverId: string) => void;
  onAddServer: () => void;
};

export default function ServersListPage({
  servers,
  statuses,
  statusErrors,
  busyMap,
  onOpenServer,
  onAddServer,
}: ServersListPageProps) {
  return (
    <Box className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0" p="4">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="6" className="h-display">
              Servers
            </Heading>
            <Text as="p" size="2" mt="1" style={{ color: "var(--color-text-muted)" }}>
              Attached remote Dune battlegroups. Click a row to open its console.
            </Text>
          </Box>
          <ActionButton onClick={onAddServer} tone="accent">
            + Add server
          </ActionButton>
        </Flex>
        <Box className="page-scroll">
          {servers.length > 0 ? (
            <div className="server-list">
              {servers.map((server, index) => {
                const status = statuses[server.id];
                const resolved = resolveServerStatus(
                  statusErrors[server.id],
                  status,
                  !!busyMap[server.id],
                  server,
                );
                const userName = server.user || remoteServerDefaultUser(server.type);
                return (
                  <button
                    key={server.id}
                    type="button"
                    className="server-row"
                    data-tone={resolved.tone}
                    style={{ animationDelay: `${index * 30}ms` }}
                    onClick={() => onOpenServer(server.id)}
                  >
                    <span className="server-row-rail" />
                    <span className="server-row-content">
                      <span className="server-row-name">{server.name}</span>
                      <span className="server-row-host">
                        {userName}@{server.host}
                        {server.battlegroupName ? ` · ${server.battlegroupName}` : ""}
                      </span>
                    </span>
                    <StatusPill
                      label={resolved.label}
                      tone={resolved.tone}
                      pulse={resolved.pulse}
                    />
                  </button>
                );
              })}
            </div>
          ) : (
            <EmptyState
              title="No remote servers attached"
              body="Add a remote Ubuntu host that already has a Dune battlegroup running."
            />
          )}
        </Box>
      </Flex>
    </Box>
  );
}
