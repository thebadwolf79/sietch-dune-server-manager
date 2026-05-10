import { Download, ExternalLink, Play, RotateCcw, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { EmptyState, StatusPill } from "../components/primitives";
import type { BattleGroupDetail, BattleGroupSummary } from "../types";

export type BattleGroupLifecycle = {
  action: "start" | "stop" | "restart";
  requestedAt: number;
  target: "running" | "stopped";
  status: string;
  lastPhase?: string;
};

type BattleGroupsPanelProps = {
  battleGroups: BattleGroupSummary[];
  selectedBattleGroup?: BattleGroupSummary;
  battleGroupDetail: BattleGroupDetail | null;
  lifecycle: BattleGroupLifecycle | null;
  busy: boolean;
  canUseManager: boolean;
  battleGroupIsStopped: boolean;
  battleGroupIsRunning: boolean;
  snapshotPath: string;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onExport: () => void;
  onSelect: (group: BattleGroupSummary) => void;
};

export function BattleGroupsPanel({
  battleGroups,
  selectedBattleGroup,
  battleGroupDetail,
  lifecycle,
  busy,
  canUseManager,
  battleGroupIsStopped,
  battleGroupIsRunning,
  snapshotPath,
  onStart,
  onStop,
  onRestart,
  onExport,
  onSelect
}: BattleGroupsPanelProps) {
  const lifecycleActive = Boolean(lifecycle);

  return (
    <section className="panel">
      <div className="panel-title">
        <h2>BattleGroups</h2>
        <div className="button-row">
          <button
            onClick={onStart}
            disabled={busy || lifecycleActive || !selectedBattleGroup || !canUseManager || !battleGroupIsStopped}
          >
            <Play size={16} />
            Start
          </button>
          <button
            onClick={onStop}
            disabled={busy || lifecycleActive || !selectedBattleGroup || !canUseManager || battleGroupIsStopped}
          >
            <Square size={16} />
            Stop
          </button>
          <button
            onClick={onRestart}
            disabled={busy || lifecycleActive || !selectedBattleGroup || !canUseManager || !battleGroupIsRunning}
          >
            <RotateCcw size={16} />
            Restart
          </button>
          <button onClick={onExport} disabled={busy || !selectedBattleGroup || !canUseManager}>
            <Download size={16} />
            Export
          </button>
        </div>
      </div>
      {battleGroups.length === 0 ? (
        <EmptyState text="No BattleGroups were found." />
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Title</th>
                <th>Phase</th>
                <th>Server Sets</th>
                <th>Image</th>
                <th>Services</th>
              </tr>
            </thead>
            <tbody>
              {battleGroups.map((group) => (
                <tr
                  key={group.namespace}
                  className={group.namespace === selectedBattleGroup?.namespace ? "selected" : ""}
                  onClick={() => onSelect(group)}
                >
                  <td>
                    <strong>{group.title || group.name}</strong>
                    <span>{group.namespace}</span>
                  </td>
                  <td>
                    <StatusPill value={group.phase} />
                  </td>
                  <td>{group.serverSets}</td>
                  <td className="mono">{group.serverImage}</td>
                  <td>
                    <div className="link-row">
                      {group.fileBrowserUrl && (
                        <a href={group.fileBrowserUrl} target="_blank" rel="noreferrer">
                          Files <ExternalLink size={14} />
                        </a>
                      )}
                      {group.directorUrl && (
                        <a href={group.directorUrl} target="_blank" rel="noreferrer">
                          Director <ExternalLink size={14} />
                        </a>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {(lifecycle || battleGroupDetail) && (
        <BattleGroupLifecyclePanel lifecycle={lifecycle} battleGroupDetail={battleGroupDetail} />
      )}
      {snapshotPath && <p className="success-line">Snapshot exported to {snapshotPath}</p>}
    </section>
  );
}

function BattleGroupLifecyclePanel({
  lifecycle,
  battleGroupDetail
}: {
  lifecycle: BattleGroupLifecycle | null;
  battleGroupDetail: BattleGroupDetail | null;
}) {
  const [now, setNow] = useState(Date.now());
  const subsystems = [
    { label: "BattleGroup", value: lifecycle?.lastPhase || battleGroupDetail?.phase || "Unknown" },
    { label: "Database", value: battleGroupDetail?.databasePhase || "Unknown" },
    { label: "Server group", value: battleGroupDetail?.serverGroupPhase || "Unknown" },
    { label: "Gateway", value: battleGroupDetail?.gatewayPhase || "Unknown" },
    { label: "Director", value: battleGroupDetail?.directorPhase || "Unknown" }
  ];

  useEffect(() => {
    if (!lifecycle) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [lifecycle]);

  return (
    <section className="lifecycle-panel">
      <div className="mini-title">
        <strong>{lifecycle ? lifecycleTitle(lifecycle.action) : "Subsystem State"}</strong>
        <span>
          {lifecycle
            ? `${lifecycle.status} / requested ${Math.max(0, Math.round((now - lifecycle.requestedAt) / 1000))}s ago`
            : "Latest Manager API detail"}
        </span>
      </div>
      <div className="subsystem-grid">
        {subsystems.map((item) => (
          <div key={item.label}>
            <span>{item.label}</span>
            <StatusPill value={item.value} />
          </div>
        ))}
      </div>
    </section>
  );
}

function lifecycleTitle(action: BattleGroupLifecycle["action"]) {
  return action === "restart" ? "Restarting BattleGroup" : action === "start" ? "Starting BattleGroup" : "Stopping BattleGroup";
}
