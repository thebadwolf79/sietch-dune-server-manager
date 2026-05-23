import type { StatusTone } from "./StatusPill";

export type MetricTone = StatusTone | "muted" | "default";

export type MetricProps = {
  label: string;
  value: string;
  tone?: MetricTone;
};

export default function Metric({ label, value, tone = "default" }: MetricProps) {
  return (
    <div className="metric">
      <div className="metric-label">{label}</div>
      <div className="metric-value" data-tone={tone === "default" ? undefined : tone}>
        {value || "—"}
      </div>
    </div>
  );
}
