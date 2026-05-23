import type { RemoteBattlegroupServerStat } from "../../types/server";
import type { StatusTone } from "../ui/StatusPill";

export type ServerStatsTableProps = {
  rows: RemoteBattlegroupServerStat[];
};

function phaseTone(phase: string): StatusTone {
  const v = phase.trim().toLowerCase();
  if (["running", "ready", "healthy", "available", "reconciling"].includes(v)) return "ok";
  if (["pending", "starting", "deploying", "scheduling", "creating"].includes(v)) return "warn";
  if (["failed", "error", "crashloop", "crashloopbackoff", "unhealthy"].includes(v)) return "err";
  return "gray";
}

/**
 * Compact per-map game-server table parsed from the vendor `battlegroup
 * status` output. Mirrors the wrapper's "Game Servers" section.
 */
export default function ServerStatsTable({ rows }: ServerStatsTableProps) {
  if (rows.length === 0) return null;
  return (
    <div className="server-stats">
      <div className="server-stats-header">
        <span>Map</span>
        <span>Phase</span>
        <span>Ready</span>
        <span>Players</span>
        <span className="server-stats-cell-age">Age</span>
      </div>
      {rows.map((row, index) => (
        <div
          key={`${row.map}-${row.age}-${index}`}
          className="server-stats-row"
          data-tone={phaseTone(row.phase)}
        >
          <span>{row.map}</span>
          <span className="server-stats-cell-phase">{row.phase}</span>
          <span>{row.ready}</span>
          <span>{row.players}</span>
          <span className="server-stats-cell-age">{row.age}</span>
        </div>
      ))}
    </div>
  );
}
