import { Users } from "lucide-react";
import { EmptyState, Metric } from "../components/primitives";
import type { DirectorPlayerSummary } from "../types";

type PlayersPanelProps = {
  players: DirectorPlayerSummary | null;
};

export function PlayersPanel({ players }: PlayersPanelProps) {
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>Players</h2>
        <Users size={19} />
      </div>
      {!players ? (
        <EmptyState text="No Director player telemetry loaded." />
      ) : (
        <div className="metric-grid">
          <Metric label="Active" value={players.active} />
          <Metric label="Online" value={players.online} />
          <Metric label="In Transit" value={players.inTransit} />
          <Metric label="Grace Period" value={players.gracePeriod} />
          <Metric label="Completion" value={players.completion} />
          <Metric label="Queued" value={players.queued} />
          <Metric label="Login Requests" value={players.loginRequestsTotal} />
          <Metric label="Travel Requests" value={players.travelRequestsTotal} />
        </div>
      )}
    </section>
  );
}
