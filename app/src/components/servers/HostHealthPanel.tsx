import { useState } from "react";
import { ShieldCheck, ShieldAlert, AlertTriangle, Info, Wrench } from "lucide-react";

export type HealthSeverity = "ok" | "info" | "warning" | "critical";

export type HostMetricChip = {
  label: string;
  value: string;
  severity: HealthSeverity;
};

export type HealthFinding = {
  id: string;
  severity: HealthSeverity;
  title: string;
  detail: string;
  recommendation: string;
  fixLabel: string | null;
};

export type HostHealthReport = {
  overallSeverity: HealthSeverity;
  summary: string;
  clusterChecked: boolean;
  metrics: HostMetricChip[];
  findings: HealthFinding[];
};

const sevMeta: Record<
  HealthSeverity,
  { label: string; text: string; bg: string; border: string; icon: typeof Info }
> = {
  ok: { label: "OK", text: "text-success", bg: "rgba(118, 184, 118, 0.15)", border: "rgba(118, 184, 118, 0.3)", icon: ShieldCheck },
  info: { label: "Info", text: "var(--color-text-primary)", bg: "var(--color-bg-elevated)", border: "var(--color-border-hair)", icon: Info },
  warning: { label: "Warning", text: "text-warning", bg: "rgba(212, 168, 94, 0.15)", border: "rgba(212, 168, 94, 0.3)", icon: AlertTriangle },
  critical: { label: "Critical", text: "text-destructive", bg: "rgba(214, 105, 94, 0.15)", border: "rgba(214, 105, 94, 0.3)", icon: ShieldAlert },
};

const severityOrder: Record<HealthSeverity, number> = { critical: 0, warning: 1, info: 2, ok: 3 };

// Mock data report as fallback
export const mockHostHealth: HostHealthReport = {
  overallSeverity: "warning",
  clusterChecked: true,
  summary: "1 warning, 2 info. Swap is undersized for the configured memory pressure.",
  metrics: [
    { label: "RAM", value: "11.4 / 15.6 GB", severity: "ok" },
    { label: "Swap", value: "1.9 / 2.0 GB", severity: "warning" },
    { label: "Swappiness", value: "60", severity: "info" },
    { label: "Disk /", value: "61% used · 38 GB free", severity: "ok" },
    { label: "DB restarts", value: "2", severity: "info" },
  ],
  findings: [
    {
      id: "swap-undersized",
      severity: "warning",
      title: "Swap space is nearly exhausted",
      detail: "Swap is 1.9 GB used of 2.0 GB while 4.2 GB of RAM is committed. Under load the kernel may OOM-kill the database pod.",
      recommendation: "Grow the swapfile to at least 8 GB and persist it in /etc/fstab.",
      fixLabel: "Resize swap to 8 GB",
    },
    {
      id: "swappiness-high",
      severity: "info",
      title: "Swappiness is higher than recommended",
      detail: "vm.swappiness=60 encourages the kernel to swap out game-server pages, adding latency spikes.",
      recommendation: "Lower vm.swappiness to 10 for a latency-sensitive game host.",
      fixLabel: "Set swappiness to 10",
    },
    {
      id: "fstab-swap-missing",
      severity: "info",
      title: "Swap is not persisted in /etc/fstab",
      detail: "The active swapfile is not referenced in /etc/fstab and will not survive a host reboot.",
      recommendation: "Add the swapfile entry to /etc/fstab.",
      fixLabel: "Persist swap in fstab",
    },
  ],
};

export type HostHealthPanelProps = {
  report?: HostHealthReport;
};

export default function HostHealthPanel({ report = mockHostHealth }: HostHealthPanelProps) {
  const [confirming, setConfirming] = useState<string | null>(null);
  const overall = sevMeta[report.overallSeverity];
  const OverallIcon = overall.icon;
  const findings = [...report.findings].sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity]);

  return (
    <div
      style={{
        border: "1px solid var(--color-border-hair)",
        backgroundColor: "var(--color-bg-panel)",
        borderRadius: "8px",
        overflow: "hidden",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "12px",
          borderBottom: "1px solid var(--color-border-hair)",
          padding: "16px",
        }}
        className="sm-row-layout-justify"
      >
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <span
            className="chamfer-sm"
            style={{
              display: "flex",
              width: "36px",
              height: "36px",
              alignItems: "center",
              justifyContent: "center",
              backgroundColor: overall.bg,
            }}
          >
            <OverallIcon className={overall.text} style={{ width: 20, height: 20 }} />
          </span>
          <div>
            <h3
              style={{
                fontFamily: "var(--font-display)",
                fontSize: "14px",
                margin: 0,
                color: "var(--color-text-primary)",
              }}
            >
              Host Health &amp; Hardening
            </h3>
            <p style={{ fontSize: "12px", margin: "4px 0 0 0", color: "var(--color-text-muted)" }}>
              {report.summary}
            </p>
          </div>
        </div>
        <span
          className="font-display"
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            borderRadius: "999px",
            border: `1px solid ${overall.border}`,
            padding: "2px 10px",
            fontSize: "11px",
            letterSpacing: "0.04em",
            backgroundColor: overall.bg,
            color: overall.text,
          }}
        >
          <span
            style={{
              width: "6px",
              height: "6px",
              borderRadius: "50%",
              backgroundColor: "currentColor",
            }}
            aria-hidden="true"
          />
          {overall.label}
        </span>
      </div>

      {/* Metric chips */}
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "8px",
          borderBottom: "1px solid var(--color-border-hair)",
          padding: "16px",
        }}
      >
        {report.metrics.map((m) => {
          const meta = sevMeta[m.severity];
          return (
            <div
              key={m.label}
              style={{
                borderRadius: "6px",
                border: `1px solid ${meta.border}`,
                backgroundColor: m.severity === "ok" ? "var(--color-bg-elevated)" : meta.bg,
                padding: "8px 12px",
                flex: "1 1 100px",
              }}
            >
              <span
                style={{
                  fontFamily: "var(--font-display)",
                  fontSize: "10px",
                  textTransform: "uppercase",
                  letterSpacing: "0.05em",
                  color: "var(--color-text-muted)",
                }}
              >
                {m.label}
              </span>
              <p
                style={{
                  marginTop: "4px",
                  fontFamily: "var(--font-mono)",
                  fontSize: "14px",
                  margin: "4px 0 0 0",
                  color: m.severity === "ok" ? "var(--color-text-primary)" : meta.text,
                }}
              >
                {m.value}
              </p>
            </div>
          );
        })}
      </div>

      {/* Findings */}
      <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
        {findings.map((f) => {
          const meta = sevMeta[f.severity];
          const Icon = meta.icon;
          const isConfirming = confirming === f.id;
          return (
            <li
              key={f.id}
              style={{
                borderBottom: "1px solid var(--color-border-hair)",
                padding: "16px",
              }}
              className="last-border-0"
            >
              <div style={{ display: "flex", gap: "12px" }}>
                <Icon className={meta.text} style={{ width: 16, height: 16, marginTop: "2px", flexShrink: 0 }} />
                <div style={{ minWidth: 0, flex: 1 }}>
                  <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px" }}>
                    <span style={{ fontSize: "14px", fontWeight: 500, color: "var(--color-text-primary)" }}>
                      {f.title}
                    </span>
                    <span
                      className="chamfer-sm font-display"
                      style={{
                        border: `1px solid ${meta.border}`,
                        padding: "1px 6px",
                        fontSize: "10px",
                        backgroundColor: meta.bg,
                        color: meta.text,
                      }}
                    >
                      {meta.label}
                    </span>
                  </div>
                  <p style={{ margin: "6px 0 0 0", fontSize: "13.5px", color: "var(--color-text-secondary)" }}>
                    {f.detail}
                  </p>
                  <p style={{ margin: "4px 0 0 0", fontSize: "12px", color: "var(--color-text-muted)" }}>
                    <span style={{ fontWeight: 500, color: "var(--color-text-secondary)" }}>Recommendation: </span>
                    {f.recommendation}
                  </p>

                  {f.fixLabel && (
                    <div style={{ marginTop: "10px" }}>
                      {isConfirming ? (
                        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px" }}>
                          <span style={{ fontSize: "12px", color: "var(--color-warn)" }}>Apply this fix on the host?</span>
                          <button
                            type="button"
                            className="action-btn"
                            style={{
                              padding: "4px 8px",
                              fontSize: "12px",
                              minHeight: "26px",
                              borderColor: "var(--color-accent)",
                              color: "var(--color-accent-strong)",
                            }}
                            onClick={() => setConfirming(null)}
                          >
                            Confirm
                          </button>
                          <button
                            type="button"
                            className="action-btn"
                            style={{
                              padding: "4px 8px",
                              fontSize: "12px",
                              minHeight: "26px",
                              border: 0,
                              background: "transparent",
                            }}
                            onClick={() => setConfirming(null)}
                          >
                            Cancel
                          </button>
                        </div>
                      ) : (
                        <button
                          type="button"
                          className="action-btn"
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "6px",
                            padding: "4px 8px",
                            fontSize: "12px",
                            minHeight: "26px",
                          }}
                          onClick={() => setConfirming(f.id)}
                        >
                          <Wrench style={{ width: 14, height: 14 }} /> {f.fixLabel}
                        </button>
                      )}
                    </div>
                  )}
                </div>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
