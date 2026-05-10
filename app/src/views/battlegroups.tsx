import { Download, ExternalLink, Play, RotateCcw, Square } from "lucide-react";
import { EmptyState, StatusPill } from "../components/primitives";
import type { BattleGroupSummary } from "../types";

type BattleGroupsPanelProps = {
  battleGroups: BattleGroupSummary[];
  selectedBattleGroup?: BattleGroupSummary;
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
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>BattleGroups</h2>
        <div className="button-row">
          <button onClick={onStart} disabled={busy || !selectedBattleGroup || !canUseManager || !battleGroupIsStopped}>
            <Play size={16} />
            Start
          </button>
          <button onClick={onStop} disabled={busy || !selectedBattleGroup || !canUseManager || battleGroupIsStopped}>
            <Square size={16} />
            Stop
          </button>
          <button
            onClick={onRestart}
            disabled={busy || !selectedBattleGroup || !canUseManager || !battleGroupIsRunning}
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
      {snapshotPath && <p className="success-line">Snapshot exported to {snapshotPath}</p>}
    </section>
  );
}
