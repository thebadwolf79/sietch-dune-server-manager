import { Terminal } from "lucide-react";
import { EmptyState } from "../components/primitives";

export function LogsPanel() {
  return (
    <section className="panel">
      <div className="panel-title">
        <h2>Logs</h2>
        <Terminal size={19} />
      </div>
      <EmptyState text="Log export and streaming will live here once the manager log endpoints are wired." />
    </section>
  );
}
