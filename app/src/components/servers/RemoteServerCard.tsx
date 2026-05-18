import { Box, Flex, Heading, Text, Badge, Button, Grid } from "@radix-ui/themes";
import {
  type RemoteServerRecord,
  type RemoteServerStatus,
  type ProxmoxVmStatus,
  type RemoteServerComponent,
  type ServerPackageStatus,
  type ServerTunnelStatus,
  type ServerTunnelStartRequest
} from "../../types";
import {
  remoteServerDefaultUser,
  remoteServerKindLabel,
  isBattlegroupStarted,
  isDirectorReadyPhase
} from "../../utils/storage";
import { Metric } from "../Common";
import { BusySpinner, ServerTunnelControls } from "./TunnelControls";
import { ComponentHealthList } from "./ComponentHealth";
import { ServerPackageCardStatus, serverPackageUpdateRequired } from "./LocalServerCard";

export function RemoteServerCard({
  server,
  compact = false,
  onRemove,
  status,
  proxmoxVmStatus,
  components,
  packageStatus,
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
  onRefreshProxmoxVm,
  onStartProxmoxVm,
  onStopProxmoxVm,
  onStartTunnel,
  onStopTunnel,
  onOpenTunnel,
  onRefreshComponentLog,
  onRestartComponent,
}: {
  server: RemoteServerRecord;
  compact?: boolean;
  onRemove?: () => void;
  status?: RemoteServerStatus;
  proxmoxVmStatus?: ProxmoxVmStatus;
  components: RemoteServerComponent[];
  packageStatus: ServerPackageStatus | null;
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  statusError?: string;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRefresh?: () => void;
  onStartBattlegroup?: () => void;
  onStopBattlegroup?: () => void;
  onUpdateBattlegroup?: () => void;
  onRefreshProxmoxVm?: () => void;
  onStartProxmoxVm?: () => void;
  onStopProxmoxVm?: () => void;
  onStartTunnel?: (request: ServerTunnelStartRequest) => void;
  onStopTunnel?: (tunnelId: string) => void;
  onOpenTunnel?: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog?: (component: RemoteServerComponent) => void;
  onRestartComponent?: (component: RemoteServerComponent) => void;
}) {
  const liveStatus = statusError ? undefined : status;
  const guestPackage = liveStatus?.package;
  const serverUpdateRequired = serverPackageUpdateRequired(guestPackage, packageStatus);
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
            <Heading size={compact ? "3" : "4"}>{server.name}</Heading>
            <Badge color="bronze" variant="soft">
              {remoteServerKindLabel(server.type)}
            </Badge>
          </Flex>
          <Text as="div" size="2" color="gray">
            {server.host} · {server.battlegroupName || "setup pending"}
          </Text>
        </Box>
        <Flex align="center" gap="2">
          <Button
            type="button"
            size="1"
            variant="surface"
            disabled={busy}
            onClick={(event) => {
              event.stopPropagation();
              onRefresh?.();
            }}
          >
            Refresh
          </Button>
          <Badge
            color={
              statusError
                ? "red"
                : battlegroupStarted
                  ? "green"
                  : battlegroupStartRequested
                    ? "amber"
                  : battlegroupStopped
                    ? "gray"
                    : server.phase === "Setup running"
                      ? "amber"
                      : "green"
            }
            variant="surface"
          >
            {statusError
              ? "Check failed"
              : busyLabel
                ? "Retrieving"
                : liveStatus
                ? battlegroupStarted
                  ? "Started"
                  : battlegroupStartRequested
                    ? "Starting"
                    : "Stopped"
                : server.phase}
          </Badge>
          {onRemove ? (
            <Button
              type="button"
              size="1"
              color="red"
              variant="soft"
              onClick={(event) => {
                event.stopPropagation();
                onRemove();
              }}
            >
              Forget
            </Button>
          ) : null}
        </Flex>
      </Flex>

      <Grid columns={compact ? "2" : "5"} gap="3" mt="3">
        <Metric label="Namespace" value={server.namespace || "pending"} />
        <Metric label="BattleGroup" value={server.battlegroupName || "pending"} />
        <Metric label="Type" value={server.type === "alpine" ? "Alpine VM over SSH" : "Ubuntu over SSH"} />
        <Metric label="Created" value={new Date(server.createdAt).toLocaleString()} />
        {server.provisioner ? <Metric label="Proxmox VM" value={`${server.provisioner.node}/${server.provisioner.vmid}`} /> : null}
      </Grid>
      {server.provisioner ? (
        <Flex align="center" gap="2" mt="3" wrap="wrap">
          <Badge color={proxmoxVmStatus?.status === "running" ? "green" : "gray"} variant="surface">
            VM {proxmoxVmStatus?.status || "unknown"}
          </Badge>
          <Button size="1" variant="surface" disabled={busy} onClick={onRefreshProxmoxVm}>
            VM Status
          </Button>
          <Button size="1" variant="surface" disabled={busy || proxmoxVmStatus?.status === "running"} onClick={onStartProxmoxVm}>
            Start VM
          </Button>
          <Button size="1" variant="surface" disabled={busy || proxmoxVmStatus?.status === "stopped"} onClick={onStopProxmoxVm}>
            Stop VM
          </Button>
        </Flex>
      ) : null}
      <ServerPackageCardStatus guestPackage={guestPackage} packageStatus={packageStatus} />
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
      {packageStatus?.updateAvailable ? (
        <Box className="setup-guide" mt="3">
          <Text size="2">
            Server package build {packageStatus.latestBuildId || "latest"} is available. Stop the BattleGroup fully before updating this server.
          </Text>
        </Box>
      ) : null}
      <Box className="server-state" mt="3">
        <Grid columns="2" gap="3">
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
          canStartDirectorTunnel={!!liveStatus && !liveStatus.battlegroup.stop && isDirectorReadyPhase(liveStatus.battlegroup.directorPhase)}
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
            {serverUpdateRequired ? (
              <Button
                size="2"
                color="amber"
                variant="solid"
                disabled={busy || !liveStatus}
                onClick={onUpdateBattlegroup}
              >
                Update Server
              </Button>
            ) : null}
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
