import { Card, Flex, Box, Heading, Text, Button } from "@radix-ui/themes";
import {
  type DuneVmCandidate,
  type RemoteServerRecord,
  type RemoteServerStatus,
  type RemoteServerComponent,
  type LocalHyperVRuntime,
  type ProxmoxVmStatus,
  type ServerTunnelStatus,
  type ServerTunnelStartRequest,
  type ServerPackageStatus
} from "../types";
import { localServerKey } from "../utils/storage";

// Sub-components decoupling
import { ServerCard } from "./servers/LocalServerCard";
import { RemoteServerCard } from "./servers/RemoteServerCard";

export function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <Box className="empty-state" p="5" style={{ textAlign: "center", border: "1px dashed rgba(255,255,255,0.1)", borderRadius: "8px", background: "rgba(0,0,0,0.1)" }}>
      <Heading size="3" mb="1">{title}</Heading>
      <Text as="p" size="2" color="gray">
        {body}
      </Text>
    </Box>
  );
}

export function ServersPage({
  duneVms,
  remoteServers,
  remoteStatuses,
  remoteComponents,
  localRuntimes,
  localRuntimeErrors,
  remoteComponentLogs,
  remoteComponentLogBusy,
  remoteComponentRestartBusy,
  remoteStatusErrors,
  remoteBusy,
  serverPackageStatus,
  proxmoxVmStatuses,
  tunnels,
  tunnelBusy,
  onAddLocalServer,
  onAddRemoteServer,
  onRemoveLocalServer,
  onRefreshLocalServer,
  onStartLocalServer,
  onStopLocalServer,
  onRemoveRemoteServer,
  onRefreshRemoteStatus,
  onStartRemoteBattlegroup,
  onStopRemoteBattlegroup,
  onUpdateRemoteBattlegroup,
  onRefreshProxmoxVm,
  onStartProxmoxVm,
  onStopProxmoxVm,
  onStartLocalBattlegroup,
  onStopLocalBattlegroup,
  onUpdateLocalBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshRemoteComponentLog,
  onRestartRemoteComponent,
  onRefreshLocalComponentLog,
  onRestartLocalComponent,
}: {
  duneVms: DuneVmCandidate[];
  remoteServers: RemoteServerRecord[];
  remoteStatuses: Record<string, RemoteServerStatus>;
  remoteComponents: Record<string, RemoteServerComponent[]>;
  localRuntimes: Record<string, LocalHyperVRuntime>;
  localRuntimeErrors: Record<string, string>;
  remoteComponentLogs: Record<string, string>;
  remoteComponentLogBusy: Record<string, boolean>;
  remoteComponentRestartBusy: Record<string, boolean>;
  remoteStatusErrors: Record<string, string>;
  remoteBusy: Record<string, string>;
  serverPackageStatus: ServerPackageStatus | null;
  proxmoxVmStatuses: Record<string, ProxmoxVmStatus>;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onAddLocalServer: () => void;
  onAddRemoteServer: () => void;
  onRemoveLocalServer: (server: DuneVmCandidate) => void;
  onRefreshLocalServer: (server: DuneVmCandidate) => void;
  onStartLocalServer: (server: DuneVmCandidate) => void;
  onStopLocalServer: (server: DuneVmCandidate) => void;
  onRemoveRemoteServer: (server: RemoteServerRecord) => void;
  onRefreshRemoteStatus: (server: RemoteServerRecord) => void;
  onStartRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onStopRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onUpdateRemoteBattlegroup: (server: RemoteServerRecord) => void;
  onRefreshProxmoxVm: (server: RemoteServerRecord) => void;
  onStartProxmoxVm: (server: RemoteServerRecord) => void;
  onStopProxmoxVm: (server: RemoteServerRecord) => void;
  onStartLocalBattlegroup: (server: DuneVmCandidate) => void;
  onStopLocalBattlegroup: (server: DuneVmCandidate) => void;
  onUpdateLocalBattlegroup: (server: DuneVmCandidate) => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshRemoteComponentLog: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRestartRemoteComponent: (server: RemoteServerRecord, component: RemoteServerComponent) => void;
  onRefreshLocalComponentLog: (server: DuneVmCandidate, component: RemoteServerComponent) => void;
  onRestartLocalComponent: (server: DuneVmCandidate, component: RemoteServerComponent) => void;
}) {
  return (
    <Card size="3" variant="surface" className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0">
        <Flex align="center" justify="between" gap="3">
          <Box>
            <Heading size="5">Servers</Heading>
            <Text as="p" size="2" color="gray" mb="0">
              Setup and basic management run through the desktop app and CLI tooling.
            </Text>
          </Box>
          <Flex gap="2" wrap="wrap" justify="end">
            <Button type="button" variant="surface" onClick={onAddLocalServer}>
              Add local Hyper-V server
            </Button>
            <Button type="button" variant="surface" onClick={onAddRemoteServer}>
              Add remote server
            </Button>
          </Flex>
        </Flex>
        <Box className="setup-scroll" style={{ flexGrow: 1, overflowY: "auto" }}>
          <Flex direction="column" gap="3">
            {duneVms.length + remoteServers.length > 0 ? (
              <>
                {duneVms.map((candidate) => (
                  <ServerCard
                    key={candidate.vm.name}
                    candidate={candidate}
                    compact
                    runtime={localRuntimes[localServerKey(candidate)]}
                    runtimeError={localRuntimeErrors[localServerKey(candidate)]}
                    packageStatus={serverPackageStatus}
                    componentLogs={remoteComponentLogs}
                    componentLogBusy={remoteComponentLogBusy}
                    componentRestartBusy={remoteComponentRestartBusy}
                    busyLabel={remoteBusy[localServerKey(candidate)]}
                    tunnels={tunnels}
                    tunnelBusy={tunnelBusy}
                    onRemove={() => onRemoveLocalServer(candidate)}
                    onRefresh={() => onRefreshLocalServer(candidate)}
                    onStart={() => onStartLocalServer(candidate)}
                    onStop={() => onStopLocalServer(candidate)}
                    onStartBattlegroup={() => onStartLocalBattlegroup(candidate)}
                    onStopBattlegroup={() => onStopLocalBattlegroup(candidate)}
                    onUpdateBattlegroup={() => onUpdateLocalBattlegroup(candidate)}
                    onStartTunnel={onStartTunnel}
                    onStopTunnel={onStopTunnel}
                    onOpenTunnel={onOpenTunnel}
                    onRefreshComponentLog={(component) => onRefreshLocalComponentLog(candidate, component)}
                    onRestartComponent={(component) => onRestartLocalComponent(candidate, component)}
                  />
                ))}
                {remoteServers.map((server) => (
                  <RemoteServerCard
                    key={server.id}
                    server={server}
                    compact
                    status={remoteStatuses[server.id]}
                    proxmoxVmStatus={proxmoxVmStatuses[server.id]}
                    components={remoteComponents[server.id] ?? []}
                    componentLogs={remoteComponentLogs}
                    componentLogBusy={remoteComponentLogBusy}
                    componentRestartBusy={remoteComponentRestartBusy}
                    statusError={remoteStatusErrors[server.id]}
                    packageStatus={serverPackageStatus}
                    busyLabel={remoteBusy[server.id]}
                    tunnels={tunnels}
                    tunnelBusy={tunnelBusy}
                    onRemove={() => onRemoveRemoteServer(server)}
                    onRefresh={() => onRefreshRemoteStatus(server)}
                    onStartBattlegroup={() => onStartRemoteBattlegroup(server)}
                    onStopBattlegroup={() => onStopRemoteBattlegroup(server)}
                    onUpdateBattlegroup={() => onUpdateRemoteBattlegroup(server)}
                    onRefreshProxmoxVm={() => onRefreshProxmoxVm(server)}
                    onStartProxmoxVm={() => onStartProxmoxVm(server)}
                    onStopProxmoxVm={() => onStopProxmoxVm(server)}
                    onStartTunnel={onStartTunnel}
                    onStopTunnel={onStopTunnel}
                    onOpenTunnel={onOpenTunnel}
                    onRefreshComponentLog={(component) => onRefreshRemoteComponentLog(server, component)}
                    onRestartComponent={(component) => onRestartRemoteComponent(server, component)}
                  />
                ))}
              </>
            ) : (
              <EmptyState
                title="No Dune servers detected"
                body="Create a new server or add a remote server profile."
              />
            )}
          </Flex>
        </Box>
      </Flex>
    </Card>
  );
}
