import { useCallback, useState } from "react";
import { AlertDialog, Button } from "@radix-ui/themes";
import { ShieldCheck, ShieldAlert, AlertTriangle, Info, Wrench } from "lucide-react";

import type { RemoteServerRecord } from "../../types/server";
import type { LogRow } from "../../types/log";
import type { HealthFinding, HealthSeverity, HostHealthReport } from "../../types/vm";
import { hostApplyFix, hostHealthCheck } from "../../services/tauri";
import { log } from "../../utils/logging";

const sevMeta: Record<
  HealthSeverity,
  { label: string; color: string; bg: string; border: string; icon: typeof Info }
> = {
  ok: {
    label: "OK",
    color: "rgb(118,184,118)",
    bg: "rgba(118,184,118,0.15)",
    border: "rgba(118,184,118,0.3)",
    icon: ShieldCheck,
  },
  info: {
    label: "Info",
    color: "var(--color-text-secondary)",
    bg: "var(--color-bg-elevated)",
    border: "var(--color-border-hair)",
    icon: Info,
  },
  warning: {
    label: "Warning",
    color: "rgb(212,168,94)",
    bg: "rgba(212,168,94,0.15)",
    border: "rgba(212,168,94,0.3)",
    icon: AlertTriangle,
  },
  critical: {
    label: "Critical",
    color: "rgb(214,105,94)",
    bg: "rgba(214,105,94,0.15)",
    border: "rgba(214,105,94,0.3)",
    icon: ShieldAlert,
  },
};

const severityOrder: Record<HealthSeverity, number> = { critical: 0, warning: 1, info: 2, ok: 3 };

export type HostHealthPanelProps = {
  server: RemoteServerRecord;
  appendLogRow: (row: LogRow) => void;
};

/**
 * Host Health & Hardening advisor. SSH-probes the VM for resource conditions an
 * operator can't easily see (no swap, high swappiness, low disk, DB restarts /
 * OOMKilled pods), shows severity-ranked findings, and offers one-click fixes
 * for the safe, idempotent ones (swap, swappiness) behind a confirmation. Host-OS
 * hardening only — it never touches Funcom's game stack.
 */
export default function HostHealthPanel({ server, appendLogRow }: HostHealthPanelProps) {
  const [report, setReport] = useState<HostHealthReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<HealthFinding | null>(null);

  const runCheck = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await hostHealthCheck({
        serverType: server.type,
        host: server.host,
        user: server.user,
        keyPath: server.keyPath,
        port: server.port,
        namespace: server.namespace || undefined,
      });
      setReport(r);
      appendLogRow(log.info("host.health", `Host health: ${r.summary}`, server.id));
    } catch (e) {
      const msg = String(e);
      setError(msg);
      appendLogRow(log.error("host.health", `Health check failed: ${msg}`, server.id));
    } finally {
      setBusy(false);
    }
  }, [server, appendLogRow]);

  const applyFix = useCallback(
    async (finding: HealthFinding) => {
      if (!finding.fixId) return;
      setApplyingId(finding.id);
      setError(null);
      try {
        const res = await hostApplyFix({
          serverType: server.type,
          host: server.host,
          user: server.user,
          keyPath: server.keyPath,
          port: server.port,
          fixId: finding.fixId,
          param: finding.fixParam ?? undefined,
        });
        appendLogRow(log.info("host.health", `Applied ${finding.fixId}: ${res.message}`, server.id));
        await runCheck();
      } catch (e) {
        const msg = String(e);
        setError(msg);
        appendLogRow(log.error("host.health", `Apply ${finding.fixId} failed: ${msg}`, server.id));
      } finally {
        setApplyingId(null);
      }
    },
    [server, appendLogRow, runCheck],
  );

  const m = report?.metrics;
  const overall = report ? sevMeta[report.overallSeverity] : null;
  const OverallIcon = overall?.icon ?? ShieldCheck;
  const chips: { label: string; value: string }[] = m
    ? [
        { label: "RAM", value: `${fmtGb(m.memAvailableMb)} free / ${fmtGb(m.memTotalMb)}` },
        {
          label: "Swap",
          value: m.swapTotalMb === 0 ? "none" : `${fmtGb(m.swapUsedMb)} used / ${fmtGb(m.swapTotalMb)}`,
        },
        { label: "Swappiness", value: m.swappiness != null ? String(m.swappiness) : "—" },
        {
          label: "Disk /",
          value: `${m.diskRootAvailGb.toFixed(1)} GB free${
            m.diskRootUsePct != null ? ` · ${m.diskRootUsePct}% used` : ""
          }`,
        },
        ...(report?.clusterChecked && m.dbMaxRestarts != null
          ? [{ label: "DB restarts", value: String(m.dbMaxRestarts) }]
          : []),
      ]
    : [];
  const findings = report
    ? [...report.findings].sort((a, b) => severityOrder[a.severity] - severityOrder[b.severity])
    : [];

  return (
    <div
      style={{
        border: "1px solid var(--color-border-hair)",
        backgroundColor: "var(--color-bg-panel)",
        borderRadius: "8px",
        overflow: "hidden",
      }}
      className="bracket chamfer"
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
          flexWrap: "wrap",
          borderBottom: "1px solid var(--color-border-hair)",
          padding: "16px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0 }}>
          <span
            className="chamfer-sm"
            style={{
              display: "flex",
              width: 36,
              height: 36,
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
              backgroundColor: overall?.bg ?? "var(--color-bg-elevated)",
            }}
          >
            <OverallIcon style={{ width: 20, height: 20, color: overall?.color ?? "var(--color-text-muted)" }} />
          </span>
          <div style={{ minWidth: 0 }}>
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
              {report
                ? report.summary
                : "Checks VM memory, swap, disk, and DB restarts / OOMKilled pods, and recommends fixes. Host-OS only — never touches the game stack."}
            </p>
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "10px", flexShrink: 0 }}>
          {overall ? (
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
                color: overall.color,
              }}
            >
              <span
                style={{ width: 6, height: 6, borderRadius: "50%", backgroundColor: "currentColor" }}
                aria-hidden="true"
              />
              {overall.label}
            </span>
          ) : null}
          <Button size="1" variant="surface" onClick={runCheck} disabled={busy}>
            {busy ? "Checking…" : report ? "Re-check" : "Check"}
          </Button>
        </div>
      </div>

      {error ? (
        <div
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--color-border-hair)",
            color: "rgb(214,105,94)",
            fontSize: "12px",
          }}
        >
          {error}
        </div>
      ) : null}

      {/* Metric chips */}
      {chips.length > 0 ? (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "8px",
            borderBottom: "1px solid var(--color-border-hair)",
            padding: "16px",
          }}
        >
          {chips.map((c) => (
            <div
              key={c.label}
              style={{
                borderRadius: "6px",
                border: "1px solid var(--color-border-hair)",
                backgroundColor: "var(--color-bg-elevated)",
                padding: "8px 12px",
                flex: "1 1 100px",
              }}
            >
              <span
                style={{
                  display: "block",
                  fontFamily: "var(--font-display)",
                  fontSize: "10px",
                  textTransform: "uppercase",
                  letterSpacing: "0.05em",
                  color: "var(--color-text-muted)",
                }}
              >
                {c.label}
              </span>
              <span className="mono" style={{ fontSize: "13px", color: "var(--color-text-primary)" }}>
                {c.value}
              </span>
            </div>
          ))}
        </div>
      ) : null}

      {/* Findings */}
      {findings.map((f) => {
        const meta = sevMeta[f.severity];
        const Icon = meta.icon;
        return (
          <div
            key={f.id}
            style={{
              display: "flex",
              gap: "12px",
              alignItems: "flex-start",
              padding: "14px 16px",
              borderBottom: "1px solid var(--color-border-hair)",
            }}
          >
            <Icon style={{ width: 16, height: 16, color: meta.color, marginTop: 2, flexShrink: 0 }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
                <span style={{ fontSize: "13px", fontWeight: 600, color: "var(--color-text-primary)" }}>
                  {f.title}
                </span>
                <span
                  className="font-display"
                  style={{
                    fontSize: "10px",
                    textTransform: "uppercase",
                    letterSpacing: "0.04em",
                    color: meta.color,
                  }}
                >
                  {meta.label}
                </span>
              </div>
              <p style={{ fontSize: "12px", margin: "4px 0 0 0", color: "var(--color-text-muted)" }}>
                {f.detail}
              </p>
              {f.recommendation ? (
                <p style={{ fontSize: "12px", margin: "4px 0 0 0", color: "var(--color-text-secondary)" }}>
                  → {f.recommendation}
                </p>
              ) : null}
            </div>
            {f.fixId ? (
              <Button
                size="1"
                onClick={() => setConfirm(f)}
                disabled={applyingId !== null}
                style={{ flexShrink: 0 }}
              >
                <Wrench style={{ width: 12, height: 12 }} />
                {applyingId === f.id ? "Applying…" : f.fixLabel ?? "Apply"}
              </Button>
            ) : null}
          </div>
        );
      })}

      <AlertDialog.Root
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
      >
        <AlertDialog.Content maxWidth="460px">
          <AlertDialog.Title>Apply “{confirm?.fixLabel ?? confirm?.title}”?</AlertDialog.Title>
          <AlertDialog.Description size="2">
            This changes the VM&apos;s host OS over SSH (it does not touch the game).{" "}
            {confirm?.recommendation}
          </AlertDialog.Description>
          <div style={{ display: "flex", gap: "8px", marginTop: "16px", justifyContent: "flex-end" }}>
            <AlertDialog.Cancel>
              <Button variant="soft" color="gray">
                Cancel
              </Button>
            </AlertDialog.Cancel>
            <Button
              onClick={() => {
                const f = confirm;
                setConfirm(null);
                if (f) void applyFix(f);
              }}
            >
              Apply
            </Button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </div>
  );
}

function fmtGb(mb: number): string {
  return `${(mb / 1024).toFixed(1)} GB`;
}
