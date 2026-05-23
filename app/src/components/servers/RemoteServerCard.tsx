import { Badge, Box, Button, Flex, Grid, Heading, Text } from "@radix-ui/themes";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import type { ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import {
  isBattlegroupStarted,
  isDirectorReadyPhase,
  remoteServerDefaultUser,
} from "../../utils/remote-server";
import { remoteStatusLabel, remoteStatusTone } from "../../utils/formatting";
import BusySpinner from "../ui/BusySpinner";
import Metric from "../ui/Metric";
import ComponentHealthList from "./ComponentHealthList";
import ServerPackageCardStatus from "./ServerPackageCardStatus";
import ServerTunnelControls from "./ServerTunnelControls";

export type RemoteServerCardProps = {
  server: RemoteServerRecord;
  onRemove: () => void;
  status?: RemoteServerStatus;
  components: RemoteServerComponent[];
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  statusError?: string;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRefresh: () => void;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
  onUpdateBattlegroup: () => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog: (component: RemoteServerComponent) => void;
  onRestartComponent: (component: RemoteServerComponent) => void;
};

export default function RemoteServerCard({
  server,
  onRemove,
  status,
  components,
  componentLogs,
  componentLogBusy,
  componentRestartBusy,
  statusError,
  busyLabel,
  tunnels,
  tunnelBusy,
  onRefresh,
  onStartBattlegroup,
  onStopBattlegroup,
  onUpdateBattlegroup,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshComponentLog,
  onRestartComponent,
}: RemoteServerCardProps) {
  const liveStatus = statusError ? undefined : status;
  const liveComponents = liveStatus ? components : [];
  const battlegroupStarted = liveStatus ? isBattlegroupStarted(liveStatus.battlegroup) : false;
  const battlegroupStartRequested = liveStatus ? !liveStatus.battlegroup.stop : false;
  const battlegroupStopped = liveStatus ? liveStatus.battlegroup.stop : false;
  const busy = !!busyLabel;

  return (
    <Box className="server-card">
      <Flex align="start" justify="between" gap="3">
        <Box>
          <Flex align="center" gap="2">
            <Heading size="3">{server.name}</Heading>
            <Badge color="bronze" variant="soft">
              Ubuntu SSH
            </Badge>
          </Flex>
          <Text as="div" size="2" color="gray">
            {server.host} · {server.battlegroupName || "unknown battlegroup"}
          </Text>
        </Box>
        <Flex align="center" gap="2">
          <Button type="button" size="1" variant="surface" disabled={busy} onClick={onRefresh}>
            Refresh
          </Button>
          <Badge
            color={remoteStatusTone(
              statusError,
              liveStatus,
              battlegroupStarted,
              battlegroupStartRequested,
              battlegroupStopped,
              server,
            )}
            variant="surface"
          >
            {remoteStatusLabel(statusError, liveStatus, busyLabel, battlegroupStarted, battlegroupStartRequested, server)}
          </Badge>
          <Button type="button" size="1" color="red" variant="soft" onClick={onRemove}>
            Forget
          </Button>
        </Flex>
      </Flex>

      <Grid columns="4" gap="3" mt="3">
        <Metric label="Namespace" value={server.namespace || "pending"} />
        <Metric label="BattleGroup" value={server.battlegroupName || "pending"} />
        <Metric label="Type" value="Remote Ubuntu" />
        <Metric label="World" value={server.worldUniqueName || "unknown"} />
      </Grid>
      <ServerPackageCardStatus guestPackage={liveStatus?.package} />
      {busyLabel ? (
        <Flex align="center" gap="2" mt="3">
          <BusySpinner />
          <Text size="2" color="gray">
            {busyLabel}
          </Text>
        </Flex>
      ) : null}
      {statusError ? (
        <Box className="server-error" mt="3">
          <Text size="2">{statusError}</Text>
        </Box>
      ) : null}
      <Box className="server-state" mt="3">
        <Grid columns="3" gap="3">
          <Metric
            label="BattleGroup State"
            value={
              liveStatus
                ? `${liveStatus.battlegroup.phase || "unknown"}; stop=${liveStatus.battlegroup.stop ? "true" : "false"}`
                : statusError || "Checking"
            }
          />
          <Metric
            label="Director"
            value={liveStatus ? liveStatus.battlegroup.directorPhase || "unknown" : statusError || "Checking"}
          />
          <Metric
            label="Server Group"
            value={liveStatus ? liveStatus.battlegroup.serverGroupPhase || "unknown" : statusError || "Checking"}
          />
        </Grid>
        <ComponentHealthList
          serverKey={server.id}
          components={liveComponents}
          logs={componentLogs}
          logBusy={componentLogBusy}
          restartBusy={componentRestartBusy}
          onRefreshLog={onRefreshComponentLog}
          onRestart={onRestartComponent}
        />
        <ServerTunnelControls
          serverKey={server.id}
          namespace={server.namespace}
          host={server.host}
          serverKind={server.type}
          user={server.user || remoteServerDefaultUser(server.type)}
          keyPath={server.keyPath}
          canStartDirectorTunnel={
            !!liveStatus && !liveStatus.battlegroup.stop && isDirectorReadyPhase(liveStatus.battlegroup.directorPhase)
          }
          canStartFileBrowserTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartDatabaseTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartPgHeroTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          tunnels={tunnels}
          tunnelBusy={tunnelBusy}
          onStartTunnel={onStartTunnel}
          onStopTunnel={onStopTunnel}
          onOpenTunnel={onOpenTunnel}
        />
        <Flex align="center" justify="between" gap="2" mt="3" wrap="wrap">
          <Flex gap="2" wrap="wrap">
            <Button size="1" variant="surface" disabled={busy} onClick={onRefresh}>
              Refresh
            </Button>
            <Button
              size="1"
              variant="surface"
              disabled={busy || !liveStatus || !battlegroupStopped}
              onClick={onStartBattlegroup}
            >
              Start BattleGroup
            </Button>
            <Button
              size="1"
              variant="surface"
              disabled={busy || !liveStatus || !battlegroupStartRequested}
              onClick={onStopBattlegroup}
            >
              Stop BattleGroup
            </Button>
            <Button size="2" color="amber" variant="solid" disabled={busy || !liveStatus} onClick={onUpdateBattlegroup}>
              Update Server
            </Button>
          </Flex>
          {busyLabel ? (
            <Text size="1" color="gray" className="mono">
              {busyLabel}
            </Text>
          ) : null}
        </Flex>
      </Box>
    </Box>
  );
}
