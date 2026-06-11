import { useEffect, useState } from "react";
import { Flex } from "@radix-ui/themes";
import { Layers, Database, Network, Compass, Clock } from "lucide-react";

import type { RemoteServerRecord, RemoteServerStatus } from "../../types/server";
import type { CustomTunnelStartRequest, ServerTunnelStartRequest, ServerTunnelStatus } from "../../types/tunnel";
import type { LogRow } from "../../types/log";
import type { ManagementStatusState } from "../management/useManagementStatus";
import {
  isDirectorReadyPhase,
  phaseTone,
  remoteServerDefaultUser,
} from "../../utils/remote-server";

import SystemStatusHeader, { type Verdict, type LifecyclePhase, type VmStage } from "./SystemStatusHeader";
import MetricTile from "../ui/MetricTile";
import HostHealthPanel from "./HostHealthPanel";
import VmPowerControls from "./VmPowerControls";
import ServerStatsTable from "./ServerStatsTable";
import ServerTunnelControls from "./ServerTunnelControls";
import CustomTunnelControls from "./CustomTunnelControls";
import ManagementServiceCard from "../management/ManagementServiceCard";

export type ServerDashboardProps = {
  server: RemoteServerRecord;
  status?: RemoteServerStatus;
  statusError?: string;
  busyLabel?: string;
  tunnels: Record<string, ServerTunnelStatus>;
  tunnelBusy: Record<string, boolean>;
  managementStatus: ManagementStatusState;
  onRefreshManagement: () => Promise<void>;
  appendLogRow: (row: LogRow) => void;
  onStartBattlegroup: () => void;
  onStopBattlegroup: () => void;
  onRestartBattlegroup: () => void;
  onStartTunnel: (request: ServerTunnelStartRequest) => void;
  onStartCustomTunnel: (request: CustomTunnelStartRequest, name: string) => void;
  onStopTunnel: (tunnelId: string) => void;
  onOpenTunnel: (tunnel: ServerTunnelStatus) => void;
};

export default function ServerDashboard({
  server,
  status,
  statusError,
  busyLabel,
  tunnels,
  tunnelBusy,
  managementStatus,
  onRefreshManagement,
  appendLogRow,
  onStartBattlegroup,
  onStopBattlegroup,
  onRestartBattlegroup,
  onStartTunnel,
  onStartCustomTunnel,
  onStopTunnel,
  onOpenTunnel,
}: ServerDashboardProps) {
  const liveStatus = statusError ? undefined : status;
  const battlegroup = liveStatus?.battlegroup;
  const battlegroupStopped = liveStatus ? liveStatus.battlegroup.stop : false;
  const directorReady = !!liveStatus && isDirectorReadyPhase(liveStatus.battlegroup.directorPhase);
  const busy = !!busyLabel;

  // 1. Parse active players and capacity
  let activePlayers = 0;
  let capacity = 40;
  if (battlegroup?.serverStats) {
    let currentSum = 0;
    let maxSum = 0;
    for (const stat of battlegroup.serverStats) {
      if (stat.players) {
        const parts = stat.players.split("/");
        const cur = parseInt(parts[0], 10);
        const max = parts[1] ? parseInt(parts[1], 10) : 0;
        if (!isNaN(cur)) currentSum += cur;
        if (!isNaN(max)) maxSum += max;
      }
    }
    activePlayers = currentSum;
    if (maxSum > 0) capacity = maxSum;
  }

  // 2. Track player history for sparkline
  const [playerHistory, setPlayerHistory] = useState<number[]>([activePlayers]);
  useEffect(() => {
    setPlayerHistory((prev) => {
      const next = [...prev, activePlayers];
      if (next.length > 15) next.shift();
      return next;
    });
  }, [activePlayers]);

  // 3. Synthesize single-glance verdict, details, stage, and lifecycle
  let verdict: Verdict = "operational";
  let detail = "All cluster components and services are operating normally.";
  let stage: VmStage = "running";
  let lifecycle: LifecyclePhase = "healthy";

  if (statusError) {
    verdict = "down";
    detail = statusError;
    stage = "off";
    lifecycle = "stopped";
  } else if (liveStatus) {
    stage = "running";
    if (battlegroupStopped) {
      verdict = "degraded";
      detail = "BattleGroup is stopped.";
      lifecycle = "stopped";
    } else {
      const dbPhase = battlegroup?.databasePhase || "";
      const gwPhase = battlegroup?.serverGroupPhase || "";
      const dirPhase = battlegroup?.directorPhase || "";

      const dbRunning = dbPhase === "Running";
      const gwRunning = gwPhase === "Running";
      const dirRunning = dirPhase === "Running" || dirPhase === "Healthy";

      if (!dbRunning || !gwRunning || !dirRunning) {
        verdict = "degraded";
        detail = `Component alert: Database is ${dbPhase || "unknown"} · Gateway is ${gwPhase || "unknown"} · Director is ${dirPhase || "unknown"}.`;
        
        const starting = [dbPhase, gwPhase, dirPhase].some(p => ["starting", "pending", "starting…"].includes(p.toLowerCase()));
        lifecycle = starting ? "starting" : "degraded";
      } else {
        verdict = "operational";
        detail = "BattleGroup is healthy. All services active.";
        lifecycle = "healthy";
      }
    }
  } else {
    // Loading / unknown state
    verdict = "degraded";
    detail = "Connecting to host…";
    stage = "running";
    lifecycle = "starting";
  }

  // Handle busy labels for starting/stopping
  if (busyLabel) {
    if (busyLabel.toLowerCase().includes("starting")) {
      lifecycle = "starting";
      detail = `${busyLabel}…`;
    } else if (busyLabel.toLowerCase().includes("stopping")) {
      lifecycle = "stopping";
      detail = `${busyLabel}…`;
    }
  }

  const dbTone = battlegroup ? phaseTone(battlegroup.databasePhase ?? "") : "muted";
  const gwTone = battlegroup ? phaseTone(battlegroup.serverGroupPhase) : "muted";
  const dirTone = battlegroup ? phaseTone(battlegroup.directorPhase) : "muted";

  return (
    <Flex direction="column" gap="4">
      {/* 1 — Pinned verdict command deck */}
      <SystemStatusHeader
        verdict={verdict}
        detail={detail}
        serverName={server.name}
        host={`${server.user || remoteServerDefaultUser(server.type)}@${server.host}`}
        uptime={battlegroup?.uptime ?? ""}
        activePlayers={activePlayers}
        capacity={capacity}
        playerTrend={playerHistory}
        vmName={server.worldUniqueName || "dune-awakening"}
        stage={stage}
        lifecycle={lifecycle}
        busy={busy}
        onStartBg={onStartBattlegroup}
        onStopBg={onStopBattlegroup}
        onRestartBg={onRestartBattlegroup}
      />

      {/* Hyper-V VM power controls (#28) — self-hides when not on the Hyper-V host */}
      <VmPowerControls />

      {/* 2 — Bento metric grid */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))",
          gap: "12px",
        }}
      >
        <MetricTile
          label="Namespace"
          value={server.namespace || "—"}
          icon={Layers}
          span
        />
        <MetricTile
          label="Database"
          value={battlegroup?.databasePhase ?? "—"}
          tone={dbTone === "ok" ? "healthy" : dbTone === "err" ? "danger" : dbTone === "warn" ? "warning" : "muted"}
          icon={Database}
          mono={false}
        />
        <MetricTile
          label="Gateway"
          value={battlegroup?.serverGroupPhase ?? "—"}
          tone={gwTone === "ok" ? "healthy" : gwTone === "err" ? "danger" : gwTone === "warn" ? "warning" : "muted"}
          icon={Network}
          mono={false}
        />
        <MetricTile
          label="Director"
          value={battlegroup?.directorPhase ?? "—"}
          tone={dirTone === "ok" ? "healthy" : dirTone === "err" ? "danger" : dirTone === "warn" ? "warning" : "muted"}
          icon={Compass}
          mono={false}
        />
        <MetricTile
          label="Uptime"
          value={battlegroup?.uptime ?? "—"}
          icon={Clock}
        />
      </div>

      {/* 3 — Dense data two-column layout */}
      <div className="dashboard-grid-columns" style={{ display: "grid", gridTemplateColumns: "1fr", gap: "16px" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
          {battlegroup?.serverStats && battlegroup.serverStats.length > 0 ? (
            <ServerStatsTable rows={battlegroup.serverStats} />
          ) : null}

          <ManagementServiceCard
            server={server}
            status={managementStatus}
            onRefresh={onRefreshManagement}
            appendLogRow={appendLogRow}
          />
        </div>
        <div>
          <HostHealthPanel server={server} appendLogRow={appendLogRow} />
        </div>
      </div>

      {/* 4 — Tunnels */}
      <ServerTunnelControls
        serverKey={server.id}
        namespace={server.namespace}
        host={server.host}
        serverKind={server.type}
        user={server.user || remoteServerDefaultUser(server.type)}
        keyPath={server.keyPath}
        port={server.port}
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
      <CustomTunnelControls
        key={server.id}
        serverKey={server.id}
        host={server.host}
        serverKind={server.type}
        user={server.user || remoteServerDefaultUser(server.type)}
        keyPath={server.keyPath}
        port={server.port}
        tunnels={tunnels}
        tunnelBusy={tunnelBusy}
        onStartCustomTunnel={onStartCustomTunnel}
        onStopTunnel={onStopTunnel}
        onOpenTunnel={onOpenTunnel}
      />
    </Flex>
  );
}
