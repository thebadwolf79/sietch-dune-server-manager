import { Box, Flex } from "@radix-ui/themes";

import type {
  RemoteServerComponent,
  RemoteServerRecord,
  RemoteServerStatus,
} from "../../types/server";
import type { ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import {
  hasBattlegroupUpdateAvailable,
  isBattlegroupStarted,
  isDirectorReadyPhase,
  remoteServerDefaultUser,
} from "../../utils/remote-server";
import ActionButton from "../ui/ActionButton";
import Metric from "../ui/Metric";
import StatusPill, { type StatusTone } from "../ui/StatusPill";
import ComponentHealthList from "./ComponentHealthList";
import ServerPackageCardStatus from "./ServerPackageCardStatus";
import ServerStatsTable from "./ServerStatsTable";
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

type ResolvedStatus = {
  tone: StatusTone;
  label: string;
  pulse: boolean;
};

function resolveStatus(
  statusError: string | undefined,
  liveStatus: RemoteServerStatus | undefined,
  busy: boolean,
  battlegroupStarted: boolean,
  battlegroupStartRequested: boolean,
  battlegroupStopped: boolean,
): ResolvedStatus {
  if (statusError) return { tone: "err", label: "Check failed", pulse: false };
  if (!liveStatus) return { tone: "gray", label: busy ? "Checking" : "Unknown", pulse: busy };
  if (battlegroupStarted) return { tone: "ok", label: "Started", pulse: false };
  if (battlegroupStartRequested) return { tone: "warn", label: "Starting", pulse: true };
  if (battlegroupStopped) return { tone: "gray", label: "Stopped", pulse: false };
  return { tone: "warn", label: liveStatus.battlegroup.phase || "Pending", pulse: true };
}

function phaseTone(phase: string): StatusTone {
  const v = phase.trim().toLowerCase();
  if (["running", "ready", "healthy", "available", "reconciling"].includes(v)) return "ok";
  if (["pending", "starting", "deploying", "scheduling", "creating"].includes(v)) return "warn";
  if (["failed", "error", "crashloop", "crashloopbackoff", "unhealthy"].includes(v)) return "err";
  return "gray";
}

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
  const updateAvailable = hasBattlegroupUpdateAvailable(liveStatus?.package);
  const busy = !!busyLabel;
  const resolved = resolveStatus(
    statusError,
    liveStatus,
    busy,
    battlegroupStarted,
    battlegroupStartRequested,
    battlegroupStopped,
  );
  const directorReady = !!liveStatus && isDirectorReadyPhase(liveStatus.battlegroup.directorPhase);
  const battlegroup = liveStatus?.battlegroup;

  return (
    <Box className="server-card" data-tone={resolved.tone}>
      <div className="server-card-hero">
        <div className="server-card-rail" />
        <Flex direction="column" gap="1" minWidth="0">
          <Flex align="center" gap="3" wrap="wrap">
            <span className="server-name">{server.name}</span>
            <StatusPill label={resolved.label} tone={resolved.tone} pulse={resolved.pulse} />
            {busyLabel ? <span className="app-title-sub">{busyLabel}</span> : null}
          </Flex>
          <span className="server-host">
            {server.user || remoteServerDefaultUser(server.type)}@{server.host}
            {server.battlegroupName ? ` · ${server.battlegroupName}` : ""}
            {battlegroup?.uptime ? ` · up ${battlegroup.uptime}` : ""}
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

      <div className="server-card-body">
        <div className="metric-grid">
          <Metric label="Namespace" value={server.namespace || ""} />
          <Metric label="BattleGroup" value={server.battlegroupName || ""} />
          <Metric
            label="Database"
            value={battlegroup?.databasePhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.databasePhase ?? "") : "muted"}
          />
          <Metric
            label="Gateway"
            value={battlegroup?.serverGroupPhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.serverGroupPhase) : "muted"}
          />
          <Metric
            label="Director"
            value={battlegroup?.directorPhase ?? ""}
            tone={battlegroup ? phaseTone(battlegroup.directorPhase) : "muted"}
          />
          <Metric label="Uptime" value={battlegroup?.uptime ?? ""} />
        </div>

        <ServerPackageCardStatus guestPackage={liveStatus?.package} />

        {battlegroup?.serverStats && battlegroup.serverStats.length > 0 ? (
          <ServerStatsTable rows={battlegroup.serverStats} />
        ) : null}

        {statusError ? (
          <div className="server-error">{statusError}</div>
        ) : null}

        <div className="action-row">
          {battlegroupStopped || !liveStatus ? (
            <ActionButton
              onClick={onStartBattlegroup}
              busy={busy && !battlegroupStarted}
              disabled={busy || !liveStatus || !battlegroupStopped}
              tone="accent"
              pendingLabel="Starting"
            >
              Start BattleGroup
            </ActionButton>
          ) : null}
          {battlegroupStartRequested ? (
            <ActionButton
              onClick={onStopBattlegroup}
              busy={busy && battlegroupStartRequested && !battlegroupStopped}
              disabled={busy || !liveStatus}
              tone="danger"
              pendingLabel="Stopping"
            >
              Stop BattleGroup
            </ActionButton>
          ) : null}
          {updateAvailable ? (
            <ActionButton
              onClick={onUpdateBattlegroup}
              busy={busy}
              disabled={busy || !liveStatus}
              tone="accent"
              pendingLabel="Updating"
              title="Run vendor update (steamcmd + operators + maps + images)"
            >
              Update Server
            </ActionButton>
          ) : null}
        </div>

        <ServerTunnelControls
          serverKey={server.id}
          namespace={server.namespace}
          host={server.host}
          serverKind={server.type}
          user={server.user || remoteServerDefaultUser(server.type)}
          keyPath={server.keyPath}
          canStartDirectorTunnel={!!liveStatus && !liveStatus.battlegroup.stop && directorReady}
          canStartFileBrowserTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartDatabaseTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          canStartPgHeroTunnel={!!liveStatus && !liveStatus.battlegroup.stop}
          tunnels={tunnels}
          tunnelBusy={tunnelBusy}
          onStartTunnel={onStartTunnel}
          onStopTunnel={onStopTunnel}
          onOpenTunnel={onOpenTunnel}
        />

        {liveComponents.length > 0 ? (
          <ComponentHealthList
            serverKey={server.id}
            components={liveComponents}
            logs={componentLogs}
            logBusy={componentLogBusy}
            restartBusy={componentRestartBusy}
            onRefreshLog={onRefreshComponentLog}
            onRestart={onRestartComponent}
          />
        ) : null}
      </div>
    </Box>
  );
}
