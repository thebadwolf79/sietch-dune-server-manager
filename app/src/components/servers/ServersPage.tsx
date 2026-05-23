import { Box, Button, Card, Flex, Heading, Text } from "@radix-ui/themes";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import type { ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import EmptyState from "../ui/EmptyState";
import RemoteServerCard from "./RemoteServerCard";

export type ServersPageProps = {
  remoteServers: RemoteServerRecord[];
  remoteStatuses: Record<string, RemoteServerStatus>;
  remoteComponents: Record<string, RemoteServerComponent[]>;
  remoteComponentLogs: Record<string, string>;
  remoteComponentLogBusy: Record<string, boolean>;
  remoteComponentRestartBusy: Record<string, boolean>;
  remoteStatusErrors: Record<string, string>;
  remoteBusy: Record<string, string>;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onAddRemoteServer: () => void;
  onRemoveRemoteServer: (server: RemoteServerRecord) => void;
  onRefreshRemoteStatus: (server: RemoteServerRecord) => void;
  onStartRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStopRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onUpdateRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshRemoteComponentLog: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRestartRemoteComponent: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
};

export default function ServersPage({
  remoteServers,
  remoteStatuses,
  remoteComponents,
  remoteComponentLogs,
  remoteComponentLogBusy,
  remoteComponentRestartBusy,
  remoteStatusErrors,
  remoteBusy,
  tunnels,
  tunnelBusy,
  onAddRemoteServer,
  onRemoveRemoteServer,
  onRefreshRemoteStatus,
  onStartRemoteBattlegroup,
  onStopRemoteBattlegroup,
  onUpdateRemoteBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshRemoteComponentLog,
  onRestartRemoteComponent,
}: ServersPageProps) {
  return (
    <Card size="3" variant="surface" className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="5">Servers</Heading>
            <Text as="p" size="2" color="gray" mb="0">
              Manage existing remote Dune servers over SSH and Kubernetes.
            </Text>
          </Box>
          <Button type="button" variant="surface" onClick={onAddRemoteServer}>
            Add remote server
          </Button>
        </Flex>
        <Box className="page-scroll">
          <Flex direction="column" gap="3">
            {remoteServers.length > 0 ? (
              remoteServers.map((server) => (
                <RemoteServerCard
                  key={server.id}
                  server={server}
                  status={remoteStatuses[server.id]}
                  components={remoteComponents[server.id] ?? []}
                  componentLogs={remoteComponentLogs}
                  componentLogBusy={remoteComponentLogBusy}
                  componentRestartBusy={remoteComponentRestartBusy}
                  statusError={remoteStatusErrors[server.id]}
                  busyLabel={remoteBusy[server.id]}
                  tunnels={tunnels}
                  tunnelBusy={tunnelBusy}
                  onRemove={() => onRemoveRemoteServer(server)}
                  onRefresh={() => onRefreshRemoteStatus(server)}
                  onStartBattlegroup={() => onStartRemoteBattlegroup(server)}
                  onStopBattlegroup={() => onStopRemoteBattlegroup(server)}
                  onUpdateBattlegroup={() => onUpdateRemoteBattlegroup(server)}
                  onStartTunnel={onStartTunnel}
                  onStopTunnel={onStopTunnel}
                  onOpenTunnel={onOpenTunnel}
                  onRefreshComponentLog={(component) => onRefreshRemoteComponentLog(server, component)}
                  onRestartComponent={(component) => onRestartRemoteComponent(server, component)}
                />
              ))
            ) : (
              <EmptyState title="No remote servers" body="Add a remote Ubuntu host that already has a Dune battlegroup." />
            )}
          </Flex>
        </Box>
      </Flex>
    </Card>
  );
}
