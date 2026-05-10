import { SlidersHorizontal } from "lucide-react";
import { EmptyState, InfoRow } from "../../components/primitives";
import type { BattleGroupDetail } from "../../types";

type LiveConfigPanelProps = {
  battleGroupDetail: BattleGroupDetail | null;
};

export function LiveConfigPanel({ battleGroupDetail }: LiveConfigPanelProps) {
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>Live Config</h2>
        <SlidersHorizontal size={19} />
      </div>
      {!battleGroupDetail ? (
        <EmptyState text="No live BattleGroup detail loaded." />
      ) : (
        <>
          <section className="config-summary">
            <InfoRow label="Title" value={battleGroupDetail.title} />
            <InfoRow label="Database" value={battleGroupDetail.databasePhase || "Unknown"} />
            <InfoRow label="Server group" value={battleGroupDetail.serverGroupPhase || battleGroupDetail.phase} />
            <InfoRow label="Gateway" value={battleGroupDetail.gatewayPhase || "Unknown"} />
            <InfoRow label="Director" value={battleGroupDetail.directorPhase || "Unknown"} />
            <InfoRow label="Stop flag" value={battleGroupDetail.stop ? "true" : "false"} />
          </section>
          <div className="image-list">
            <strong>Images</strong>
            {[battleGroupDetail.serverImage, ...battleGroupDetail.utilityImages].filter(Boolean).map((image) => (
              <span className="mono chip" key={image}>
                {image}
              </span>
            ))}
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Map</th>
                  <th>Replicas</th>
                  <th>Memory</th>
                  <th>Scaling</th>
                </tr>
              </thead>
              <tbody>
                {battleGroupDetail.serverSets.map((set) => (
                  <tr key={set.map}>
                    <td>
                      <strong>{set.map}</strong>
                    </td>
                    <td>{set.replicas}</td>
                    <td>{set.memoryLimit || "Unset"}</td>
                    <td>{set.dedicatedScaling ? "Dedicated" : "Fixed"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}
