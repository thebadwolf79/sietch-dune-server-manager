import { EmptyState, StatusPill } from "../components/primitives";
import type { KubeItem } from "../types";

type WorkloadsPanelProps = {
  pods: KubeItem[];
  services: KubeItem[];
};

export function WorkloadsPanel({ pods, services }: WorkloadsPanelProps) {
  return (
    <section className="grid two">
      <article className="panel">
        <div className="panel-title">
          <h2>Pods</h2>
          <span>{pods.length}</span>
        </div>
        {pods.length === 0 ? (
          <EmptyState text="No pod data loaded." />
        ) : (
          <div className="compact-list">
            {pods.map((pod) => {
              const status = String(pod.status?.phase ?? "Unknown");
              return (
                <div key={pod.metadata?.name}>
                  <strong>{pod.metadata?.name}</strong>
                  <StatusPill value={status} />
                </div>
              );
            })}
          </div>
        )}
      </article>

      <article className="panel">
        <div className="panel-title">
          <h2>Services</h2>
          <span>{services.length}</span>
        </div>
        {services.length === 0 ? (
          <EmptyState text="No service data loaded." />
        ) : (
          <div className="compact-list">
            {services.map((service) => {
              const ports = Array.isArray(service.spec?.ports)
                ? service.spec?.ports
                    .map((port) => {
                      const row = port as Record<string, unknown>;
                      return row.nodePort ? `${row.port}:${row.nodePort}` : String(row.port);
                    })
                    .join(", ")
                : "";
              return (
                <div key={service.metadata?.name}>
                  <strong>{service.metadata?.name}</strong>
                  <span>{ports}</span>
                </div>
              );
            })}
          </div>
        )}
      </article>
    </section>
  );
}
