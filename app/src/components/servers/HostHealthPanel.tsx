import { useCallback, useState } from "react";
import { AlertDialog, Badge, Box, Button, Card, Flex, Text } from "@radix-ui/themes";

import type { RemoteServerRecord } from "../../types/server";
import type { LogRow } from "../../types/log";
import type { HealthFinding, HealthSeverity, HostHealthReport } from "../../types/vm";
import { hostApplyFix, hostHealthCheck } from "../../services/tauri";
import { log } from "../../utils/logging";

const SEVERITY_TONE: Record<HealthSeverity, "green" | "blue" | "orange" | "red"> = {
  ok: "green",
  info: "blue",
  warning: "orange",
  critical: "red",
};

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

  return (
    <Card>
      <Flex direction="column" gap="3">
        <Flex justify="between" align="center" wrap="wrap" gap="2">
          <Flex align="center" gap="2">
            <Text size="3" weight="medium">
              Host health &amp; hardening
            </Text>
            {report ? (
              <Badge color={SEVERITY_TONE[report.overallSeverity]}>{report.overallSeverity}</Badge>
            ) : null}
          </Flex>
          <Button size="1" variant="surface" onClick={runCheck} disabled={busy}>
            {busy ? "Checking…" : report ? "Re-check" : "Check host health"}
          </Button>
        </Flex>

        <Text size="1" color="gray">
          Checks VM memory, swap, disk, and (if reachable) DB restarts / OOMKilled pods, and
          recommends fixes. Host-OS only — never touches the game stack.
        </Text>

        {error ? (
          <Text size="1" color="red">
            {error}
          </Text>
        ) : null}

        {m ? (
          <Flex gap="3" wrap="wrap">
            <MetricChip label="RAM" value={`${fmtGb(m.memAvailableMb)} free / ${fmtGb(m.memTotalMb)}`} />
            <MetricChip
              label="Swap"
              value={
                m.swapTotalMb === 0
                  ? "none"
                  : `${fmtGb(m.swapUsedMb)} used / ${fmtGb(m.swapTotalMb)}`
              }
            />
            <MetricChip label="Swappiness" value={m.swappiness != null ? String(m.swappiness) : "—"} />
            <MetricChip label="Disk /" value={`${m.diskRootAvailGb.toFixed(1)} GB free`} />
            {report?.clusterChecked && m.dbMaxRestarts != null ? (
              <MetricChip label="DB restarts" value={String(m.dbMaxRestarts)} />
            ) : null}
          </Flex>
        ) : null}

        {report
          ? report.findings.map((f) => (
              <Box key={f.id} className="server-error" style={{ background: "var(--color-panel-translucent)" }}>
                <Flex justify="between" align="start" gap="2">
                  <Flex direction="column" gap="1" style={{ minWidth: 0 }}>
                    <Flex align="center" gap="2">
                      <Badge color={SEVERITY_TONE[f.severity]}>{f.severity}</Badge>
                      <Text size="2" weight="medium">
                        {f.title}
                      </Text>
                    </Flex>
                    <Text size="1" color="gray">
                      {f.detail}
                    </Text>
                    {f.recommendation ? (
                      <Text size="1">→ {f.recommendation}</Text>
                    ) : null}
                  </Flex>
                  {f.fixId ? (
                    <Button
                      size="1"
                      onClick={() => setConfirm(f)}
                      disabled={applyingId !== null}
                    >
                      {applyingId === f.id ? "Applying…" : f.fixLabel ?? "Apply fix"}
                    </Button>
                  ) : null}
                </Flex>
              </Box>
            ))
          : null}
      </Flex>

      <AlertDialog.Root
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
      >
        <AlertDialog.Content maxWidth="460px">
          <AlertDialog.Title>Apply “{confirm?.fixLabel ?? confirm?.title}”?</AlertDialog.Title>
          <AlertDialog.Description size="2">
            This changes the VM&apos;s host OS over SSH (it does not touch the game). {confirm?.recommendation}
          </AlertDialog.Description>
          <Flex gap="2" mt="4" justify="end">
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
          </Flex>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </Card>
  );
}

function fmtGb(mb: number): string {
  return `${(mb / 1024).toFixed(1)} GB`;
}

function MetricChip({ label, value }: { label: string; value: string }) {
  return (
    <Flex direction="column" gap="0">
      <Text size="1" color="gray" style={{ textTransform: "uppercase", letterSpacing: 0.5 }}>
        {label}
      </Text>
      <Text size="2" weight="medium" className="mono">
        {value}
      </Text>
    </Flex>
  );
}
