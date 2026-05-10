import { Activity, Map, RefreshCw } from "lucide-react";
import { EmptyState, StatusPill } from "../components/primitives";
import type { DirectorMapSummary } from "../types";

type DirectorViewProps = {
  directorMaps: DirectorMapSummary[];
  busy: boolean;
  onReload: () => void;
  onEditMap: (mapName: string) => void;
  onClearMapOverride: (mapName: string) => void;
};

export function DirectorView({
  directorMaps,
  busy,
  onReload,
  onEditMap,
  onClearMapOverride
}: DirectorViewProps) {
  return (
    <>
      <section className="panel">
        <div className="panel-title">
          <h2>Director Maps</h2>
          <div className="button-row">
            <button onClick={onReload} disabled={busy}>
              <RefreshCw size={16} />
              Reload
            </button>
            <Map size={19} />
          </div>
        </div>
        {directorMaps.length === 0 ? (
          <EmptyState text="No Director map data loaded." />
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Map</th>
                  <th>Kind</th>
                  <th>Players</th>
                  <th>Queue</th>
                  <th>Servers</th>
                  <th>Override</th>
                </tr>
              </thead>
              <tbody>
                {directorMaps.map((map) => (
                  <tr key={`${map.kind}-${map.name}`}>
                    <td>
                      <strong>{map.name}</strong>
                    </td>
                    <td>{map.kind}</td>
                    <td>{map.players}</td>
                    <td>{map.queued}</td>
                    <td>{map.servers.length}</td>
                    <td>
                      <div className="button-row">
                        <StatusPill value={map.hasOverride ? "Active" : "None"} />
                        <button onClick={() => onEditMap(map.name)} disabled={busy}>
                          Edit
                        </button>
                        <button onClick={() => onClearMapOverride(map.name)} disabled={busy || !map.hasOverride}>
                          Clear
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {directorMaps.length > 0 && (
        <section className="panel">
          <div className="panel-title">
            <h2>Server Runtime</h2>
            <Activity size={19} />
          </div>
          <div className="map-card-grid">
            {directorMaps.map((map) => (
              <article className="runtime-map" key={`${map.kind}-runtime-${map.name}`}>
                <div className="mini-title">
                  <strong>{map.name}</strong>
                  <span>{map.kind}</span>
                </div>
                <div className="runtime-stats">
                  <span>{map.players} players</span>
                  <span>{map.online} online</span>
                  <span>{map.queued} queued</span>
                </div>
                <div className="runtime-servers">
                  {map.servers.length === 0 ? (
                    <EmptyState text="No server rows reported." />
                  ) : (
                    map.servers.map((server) => (
                      <div key={`${map.name}-${server.partitionId}-${server.dimensionIndex}-${server.serverId}`}>
                        <div>
                          <strong>{server.label || "Unnamed"}</strong>
                          <span className="mono">{server.serverId || "No server id"}</span>
                        </div>
                        <StatusPill value={server.status} />
                        <span>{server.players} players</span>
                        <span>{server.queued ?? "N/A"} queued</span>
                        <span>
                          {server.heartbeatSecondsAgo === null || server.heartbeatSecondsAgo === undefined
                            ? "No heartbeat"
                            : `${server.heartbeatSecondsAgo}s ago`}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </article>
            ))}
          </div>
        </section>
      )}
    </>
  );
}
