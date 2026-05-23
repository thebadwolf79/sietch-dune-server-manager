import { Box, Flex, Tabs } from "@radix-ui/themes";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import type { ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import type { ServerSubPage } from "../../types/ui";
import { remoteServerDefaultUser, resolveServerStatus } from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import StatusPill from "../ui/StatusPill";
import ServerDashboard from "./ServerDashboard";
import ServerPods from "./ServerPods";
import ServerUpdatePanel from "./ServerUpdatePanel";

export type ServerDetailPageProps = {
  server: RemoteServerRecord;
  sub: ServerSubPage;
  onSubChange: (sub: ServerSubPage) => void;
  status?: RemoteServerStatus;
  statusError?: string;
  busyLabel?: string;
  components: RemoteServerComponent[];
  componentLogs: Record<string, string>;
  componentLogBusy: Record<string, boolean>;
  componentRestartBusy: Record<string, boolean>;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  onRefresh: () => void;
  onRemove: () => void;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
  onRestartBattlegroup: () => void;
  onUpdateBattlegroup: () => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
  onRefreshComponentLog: (component: RemoteServerComponent) => void;
  onRestartComponent: (component: RemoteServerComponent) => void;
};

export default function ServerDetailPage(props: ServerDetailPageProps) {
  const {
    server,
    sub,
    onSubChange,
    status,
    statusError,
    busyLabel,
    components,
    componentLogs,
    componentLogBusy,
    componentRestartBusy,
    tunnels,
    tunnelBusy,
    onRefresh,
    onRemove,
    onStartBattlegroup,
    onStopBattlegroup,
    onRestartBattlegroup,
    onUpdateBattlegroup,
    onStartTunnel,
    onStopTunnel,
    onOpenTunnel,
    onRefreshComponentLog,
    onRestartComponent,
  } = props;
  const busy = !!busyLabel;
  const liveStatus = statusError ? undefined : status;
  const resolved = resolveServerStatus(statusError, liveStatus, busy, server);

  return (
    <Box className="pane page-pane">
      <Flex direction="column" gap="4" height="100%" minHeight="0" p="4">
        <div className="server-detail-hero" data-tone={resolved.tone}>
          <div className="server-detail-hero-rail" />
          <Flex direction="column" gap="1" minWidth="0">
            <Flex align="center" gap="3" wrap="wrap">
              <span className="server-name">{server.name}</span>
              <StatusPill label={resolved.label} tone={resolved.tone} pulse={resolved.pulse} />
              {busyLabel ? <span className="app-title-sub">{busyLabel}</span> : null}
            </Flex>
            <span className="server-host">
              {server.user || remoteServerDefaultUser(server.type)}@{server.host}
              {server.battlegroupName ? ` · ${server.battlegroupName}` : ""}
              {liveStatus?.battlegroup.uptime ? ` · up ${liveStatus.battlegroup.uptime}` : ""}
            </span>
          </Flex>
          <Flex align="center" gap="2">
            <ActionButton onClick={onRefresh} busy={busy} pendingLabel="Refreshing">
              Refresh
            </ActionButton>
            <ActionButton onClick={onRemove} tone="danger" disabled={busy}>
              Forget
            </ActionButton>
          </Flex>
        </div>

        <Tabs.Root
          className="server-detail-tabs"
          value={sub}
          onValueChange={(value) => onSubChange(value as ServerSubPage)}
        >
          <Tabs.List size="2" color="bronze">
            <Tabs.Trigger value="dashboard">Dashboard</Tabs.Trigger>
            <Tabs.Trigger value="update">Update</Tabs.Trigger>
            <Tabs.Trigger value="pods">Pods</Tabs.Trigger>
          </Tabs.List>

          <Tabs.Content value="dashboard" className="server-detail-tab-content">
            <ServerDashboard
              server={server}
              status={liveStatus}
              statusError={statusError}
              busyLabel={busyLabel}
              tunnels={tunnels}
              tunnelBusy={tunnelBusy}
              onStartBattlegroup={onStartBattlegroup}
              onStopBattlegroup={onStopBattlegroup}
              onRestartBattlegroup={onRestartBattlegroup}
              onStartTunnel={onStartTunnel}
              onStopTunnel={onStopTunnel}
              onOpenTunnel={onOpenTunnel}
            />
          </Tabs.Content>
          <Tabs.Content value="update" className="server-detail-tab-content">
            <ServerUpdatePanel
              server={server}
              status={liveStatus}
              busyLabel={busyLabel}
              onUpdateBattlegroup={onUpdateBattlegroup}
            />
          </Tabs.Content>
          <Tabs.Content value="pods" className="server-detail-tab-content">
            <ServerPods
              serverKey={server.id}
              components={liveStatus ? components : []}
              logs={componentLogs}
              logBusy={componentLogBusy}
              restartBusy={componentRestartBusy}
              onRefreshLog={onRefreshComponentLog}
              onRestart={onRestartComponent}
            />
          </Tabs.Content>
        </Tabs.Root>
      </Flex>
    </Box>
  );
}
