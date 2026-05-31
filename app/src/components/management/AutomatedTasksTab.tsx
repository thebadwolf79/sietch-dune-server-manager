import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Box,
  Button,
  Callout,
  Checkbox,
  Dialog,
  Flex,
  Link,
  Separator,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";

import { managementApi, managementService } from "../../services/management";
import { openExternal } from "../../services/tauri";
import type { RemoteServerRecord } from "../../types/server";
import type {
  LogDto,
  RestartNoticeOptions,
  RunDto,
  ScheduleConfig,
} from "../../types/management";
import { formatDateTime, formatTime } from "../../utils/formatting";
import Combobox from "./Combobox";
import DumpPruneDialog from "./DumpPruneDialog";

const DIRECT_TASKS: Array<{ id: string; label: string }> = [
  { id: "backup", label: "Backup" },
  { id: "welcome-package", label: "Welcome package scan" },
  { id: "update-check", label: "Check for server update" },
  { id: "update-apply", label: "Apply server update" },
  { id: "restart", label: "Restart server" },
];

export type AutomatedTasksTabProps = {
  tunnelId: string;
  server: RemoteServerRecord;
  onAfterRestart?: () => Promise<void> | void;
};

type LogState = {
  status: "idle" | "loading" | "ready" | "error";
  logs: LogDto[];
  error?: string;
};

export default function AutomatedTasksTab({
  tunnelId,
  server,
  onAfterRestart,
}: AutomatedTasksTabProps) {
  const [runs, setRuns] = useState<RunDto[]>([]);
  const [busyTrigger, setBusyTrigger] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [logsByRun, setLogsByRun] = useState<Record<number, LogState>>({});
  const [noticeOpen, setNoticeOpen] = useState(false);
  const [dumpPruneOpen, setDumpPruneOpen] = useState(false);

  const reload = useCallback(async () => {
    try {
      const r = await managementApi.listRuns(tunnelId, 50);
      setRuns(r);
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    void reload();
    const handle = setInterval(reload, 5000);
    return () => clearInterval(handle);
  }, [reload]);

  const fetchLogs = useCallback(
    async (runId: number) => {
      setLogsByRun((prev) => ({
        ...prev,
        [runId]: { status: "loading", logs: prev[runId]?.logs ?? [] },
      }));
      try {
        const l = await managementApi.listLogs(tunnelId, 500, runId);
        setLogsByRun((prev) => ({ ...prev, [runId]: { status: "ready", logs: l } }));
      } catch (err) {
        setLogsByRun((prev) => ({
          ...prev,
          [runId]: { status: "error", logs: prev[runId]?.logs ?? [], error: String(err) },
        }));
      }
    },
    [tunnelId],
  );

  const trigger = useCallback(
    async (task: string, options?: Record<string, unknown>) => {
      setBusyTrigger(task);
      try {
        await managementApi.triggerRun(tunnelId, task, options);
        await reload();
      } catch (err) {
        alert(`Trigger ${task} failed: ${err}`);
      } finally {
        setBusyTrigger(null);
      }
    },
    [reload, tunnelId],
  );

  return (
    <Box mt="3">
      <ScheduleSettings tunnelId={tunnelId} server={server} onAfterRestart={onAfterRestart} />

      <Box mt="4">
        <Flex justify="between" align="start" gap="3" wrap="wrap">
          <Text size="2" color="gray">
            Run the scheduled maintenance tasks manually. Each run records its own log entries below.
          </Text>
          <Button size="1" variant="surface" onClick={reload}>
            Refresh
          </Button>
        </Flex>
        <Flex gap="2" wrap="wrap" mt="2" mb="3">
          {DIRECT_TASKS.map((t) => (
            <Button
              key={t.id}
              size="1"
              variant="surface"
              disabled={busyTrigger === t.id}
              onClick={() => trigger(t.id)}
            >
              {busyTrigger === t.id ? `Running ${t.label}…` : t.label}
            </Button>
          ))}
          <Button
            size="1"
            variant="surface"
            disabled={busyTrigger === "restart-notice"}
            onClick={() => setNoticeOpen(true)}
          >
            {busyTrigger === "restart-notice"
              ? "Sending restart notice…"
              : "Send restart notice…"}
          </Button>
          <Button
            size="1"
            variant="surface"
            color="red"
            onClick={() => setDumpPruneOpen(true)}
          >
            Clean up database operations…
          </Button>
        </Flex>
      </Box>

      {error ? (
        <Text size="1" color="red">
          {error}
        </Text>
      ) : null}

      <Box>
        <Text size="2" weight="medium" mb="2">
          Recent runs
        </Text>
        <Flex direction="column" gap="2" mt="2">
          {runs.length === 0 ? <Text color="gray">No runs yet.</Text> : null}
          {runs.map((run) => (
            <RunRow
              key={run.id}
              run={run}
              logsState={logsByRun[run.id]}
              onExpand={() => {
                if (!logsByRun[run.id]) void fetchLogs(run.id);
              }}
              onRefreshLogs={() => void fetchLogs(run.id)}
            />
          ))}
        </Flex>
      </Box>

      <RestartNoticeDialog
        open={noticeOpen}
        onOpenChange={setNoticeOpen}
        onSubmit={async (options) => {
          setNoticeOpen(false);
          await trigger("restart-notice", options as Record<string, unknown>);
        }}
      />

      <DumpPruneDialog
        open={dumpPruneOpen}
        onOpenChange={setDumpPruneOpen}
        tunnelId={tunnelId}
      />
    </Box>
  );
}

function RestartNoticeDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (options: RestartNoticeOptions) => Promise<void>;
}) {
  const [leadSecs, setLeadSecs] = useState(1800);
  const [frequencySecs, setFrequencySecs] = useState(600);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Send restart notice</Dialog.Title>
        <Dialog.Description size="2" color="gray" mb="3">
          Publishes a ServerShutdown countdown to the game. If you provide a
          title + body, an additional Generic broadcast carries them as a banner.
        </Dialog.Description>
        <Flex direction="column" gap="3">
          <Box>
            <Text size="2" weight="medium">Lead time (seconds)</Text>
            <TextField.Root
              type="number"
              value={String(leadSecs)}
              onChange={(e) => setLeadSecs(Number(e.target.value) || 0)}
            />
            <Text size="1" color="gray">
              How long until the restart fires. 1800 = 30 min.
            </Text>
          </Box>
          <Box>
            <Text size="2" weight="medium">Warning frequency (seconds)</Text>
            <TextField.Root
              type="number"
              value={String(frequencySecs)}
              onChange={(e) => setFrequencySecs(Number(e.target.value) || 0)}
            />
            <Text size="1" color="gray">
              How often the game re-shows the countdown. 600 = every 10 min.
            </Text>
          </Box>
          <Box>
            <Text size="2" weight="medium">Custom title (optional)</Text>
            <TextField.Root
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Scheduled maintenance"
            />
          </Box>
          <Box>
            <Text size="2" weight="medium">Custom body (optional)</Text>
            <TextArea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              rows={3}
              placeholder="Sent as an in-game banner alongside the countdown."
            />
          </Box>
        </Flex>
        <Flex gap="2" mt="4" justify="end">
          <Dialog.Close>
            <Button variant="soft" color="gray">Cancel</Button>
          </Dialog.Close>
          <Button
            onClick={() => {
              const opts: RestartNoticeOptions = {
                leadSecs,
                frequencySecs,
                durationSecs: leadSecs,
              };
              if (title.trim()) opts.title = title.trim();
              if (body.trim()) opts.body = body.trim();
              void onSubmit(opts);
            }}
          >
            Send notice
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}

function ScheduleSettings({
  tunnelId,
  server,
  onAfterRestart,
}: {
  tunnelId: string;
  server: RemoteServerRecord;
  onAfterRestart?: () => Promise<void> | void;
}) {
  const [config, setConfig] = useState<ScheduleConfig | null>(null);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [busyLabel, setBusyLabel] = useState("Saving…");
  const [error, setError] = useState<string | null>(null);

  // Editable form fields, mirroring ScheduleConfig. Reset from `config`
  // every time we enter edit mode so Cancel reverts cleanly.
  const [hour, setHour] = useState(5);
  const [minute, setMinute] = useState(0);
  const [warnFreq, setWarnFreq] = useState(600);
  const [warnDur, setWarnDur] = useState(1800);
  const [updateLead, setUpdateLead] = useState(1800);
  const [tz, setTz] = useState("UTC");
  // Master switches. Undefined from older services reads as enabled.
  const [restartEnabled, setRestartEnabled] = useState(true);
  const [updateEnabled, setUpdateEnabled] = useState(true);
  const [backupEnabled, setBackupEnabled] = useState(true);
  // 5-field cron (min hour dom mon dow); empty string = disabled.
  const [backupCron, setBackupCron] = useState("");
  const [backupCronStatus, setBackupCronStatus] = useState<
    | { state: "idle" }
    | { state: "validating" }
    | { state: "ok"; tz: string; next: string[] }
    | { state: "error"; message: string }
  >({ state: "idle" });

  const refresh = useCallback(async () => {
    try {
      const c = await managementApi.getConfig(tunnelId);
      setConfig(c);
      setHour(c.restartHour);
      setMinute(c.restartMinute);
      setWarnFreq(c.restartWarningFrequencySecs);
      setWarnDur(c.restartWarningDurationSecs);
      setUpdateLead(c.updateLeadSecs);
      setTz(c.restartTz);
      setRestartEnabled(c.restartEnabled ?? true);
      setUpdateEnabled(c.updateEnabled ?? true);
      setBackupEnabled(c.backupEnabled ?? true);
      setBackupCron(c.backupCron ?? "");
      setBackupCronStatus({ state: "idle" });
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [tunnelId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const startEdit = useCallback(() => {
    if (!config) return;
    setHour(config.restartHour);
    setMinute(config.restartMinute);
    setWarnFreq(config.restartWarningFrequencySecs);
    setWarnDur(config.restartWarningDurationSecs);
    setUpdateLead(config.updateLeadSecs);
    setTz(config.restartTz);
    setRestartEnabled(config.restartEnabled ?? true);
    setUpdateEnabled(config.updateEnabled ?? true);
    setBackupEnabled(config.backupEnabled ?? true);
    setBackupCron(config.backupCron ?? "");
    setBackupCronStatus({ state: "idle" });
    setEditing(true);
    setError(null);
  }, [config]);

  // Live-validate the cron expression while editing. Empty = disabled (no
  // server round-trip). The service caps `count` at 20 and returns a parse
  // error string when invalid; we surface either the next-fire preview or
  // the error inline.
  useEffect(() => {
    if (!editing) return;
    const trimmed = backupCron.trim();
    if (!trimmed) {
      setBackupCronStatus({ state: "idle" });
      return;
    }
    setBackupCronStatus({ state: "validating" });
    const handle = setTimeout(async () => {
      try {
        const result = await managementApi.cronPreview(tunnelId, trimmed, 5);
        if (result.ok) {
          setBackupCronStatus({ state: "ok", tz: result.tz, next: result.next });
        } else {
          setBackupCronStatus({ state: "error", message: result.error });
        }
      } catch (err) {
        setBackupCronStatus({ state: "error", message: String(err) });
      }
    }, 300);
    return () => clearTimeout(handle);
  }, [backupCron, editing, tunnelId]);

  const cancelEdit = useCallback(() => {
    setEditing(false);
    setError(null);
  }, []);

  const save = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setBusyLabel("Saving…");
      if (backupEnabled && !backupCron.trim()) {
        throw new Error("A cron expression is required while auto backup is enabled.");
      }
      if (backupCron.trim() && backupCronStatus.state === "error") {
        throw new Error(`Cron expression invalid: ${backupCronStatus.message}`);
      }
      await managementApi.setConfig(tunnelId, {
        restartHour: hour,
        restartMinute: minute,
        restartWarningFrequencySecs: warnFreq,
        restartWarningDurationSecs: warnDur,
        updateLeadSecs: updateLead,
        restartTz: tz,
        restartEnabled,
        updateEnabled,
        backupEnabled,
        backupCron: backupCron.trim(),
      });

      setBusyLabel("Restarting service…");
      await managementService.restart({
        host: server.host,
        user: server.user,
        keyPath: server.keyPath,
        port: server.port,
      });

      // Poll until the API is back up. Local SSH tunnel survives the restart;
      // the remote axum listener just takes ~1s to rebind.
      const deadline = Date.now() + 15_000;
      let lastErr: unknown = null;
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 700));
        try {
          await managementApi.getConfig(tunnelId);
          lastErr = null;
          break;
        } catch (err) {
          lastErr = err;
        }
      }
      if (lastErr) {
        throw new Error(`service did not come back up: ${lastErr}`);
      }

      await refresh();
      await onAfterRestart?.();
      setEditing(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setBusyLabel("Saving…");
    }
  }, [
    tunnelId,
    hour,
    minute,
    warnFreq,
    warnDur,
    updateLead,
    tz,
    restartEnabled,
    updateEnabled,
    backupEnabled,
    backupCron,
    backupCronStatus,
    refresh,
    server.host,
    server.user,
    server.keyPath,
    server.port,
    onAfterRestart,
  ]);

  const loadTimezones = useCallback(
    async (query: string) => {
      try {
        const all = await managementApi.listTimezones(tunnelId);
        const q = query.trim().toLowerCase();
        const filtered = q
          ? all.filter((tz) => tz.toLowerCase().includes(q))
          : all;
        return [...filtered]
          .sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base", numeric: true }))
          .slice(0, 200)
          .map((name) => ({ name }));
      } catch {
        return [];
      }
    },
    [tunnelId],
  );

  const restartRequired = config?.restartRequired ?? false;
  const displayHour = config ? pad2(config.restartHour) : "—";
  const displayMinute = config ? pad2(config.restartMinute) : "—";

  return (
    <Box className="schedule-section">
      <Flex justify="between" align="baseline" mb="2">
        <Text size="3" weight="medium">Schedule settings</Text>
        {!editing && config ? (
          <Button size="1" variant="surface" onClick={startEdit}>
            Configure
          </Button>
        ) : null}
      </Flex>
      <Text size="1" color="gray">
        Stored in the service&apos;s sqlite. Saving restarts the service automatically so changes take effect immediately.
      </Text>

      <Separator size="4" my="3" />

      {editing ? (
        <Box className="schedule-grid">
          <Text size="2">Auto restart</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={restartEnabled}
              onCheckedChange={(checked) => setRestartEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Run the daily restart and its warning broadcast
            </Text>
          </Flex>

          <Text size="2">Daily restart (HH:MM)</Text>
          <Flex gap="2" align="center">
            <TextField.Root
              inputMode="numeric"
              value={pad2(hour)}
              onChange={(e) => setHour(clampInt(e.target.value, 23))}
              onFocus={(e) => e.target.select()}
              style={{ width: 70 }}
            />
            <Text>:</Text>
            <TextField.Root
              inputMode="numeric"
              value={pad2(minute)}
              onChange={(e) => setMinute(clampInt(e.target.value, 59))}
              onFocus={(e) => e.target.select()}
              style={{ width: 70 }}
            />
          </Flex>

          <Text size="2">Timezone</Text>
          <Combobox
            value={tz}
            onChange={(v) => setTz(v)}
            loadOptions={loadTimezones}
            getOptionValue={(o: { name: string }) => o.name}
            resolveLabel={async (id) => id}
            renderOption={(o: { name: string }) => (
              <Text size="2" className="mono">{o.name}</Text>
            )}
            placeholder="Pick a timezone…"
            searchPlaceholder="Search IANA timezones…"
          />

          <Text size="2">Warning lead (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(warnDur)}
            onChange={(e) => setWarnDur(Number(e.target.value) || 0)}
          />

          <Text size="2">Warning frequency (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(warnFreq)}
            onChange={(e) => setWarnFreq(Number(e.target.value) || 0)}
          />

          <Text size="2">Auto update</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={updateEnabled}
              onCheckedChange={(checked) => setUpdateEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Check Steam for new builds and apply them automatically
            </Text>
          </Flex>

          <Text size="2">Update apply lead (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(updateLead)}
            onChange={(e) => setUpdateLead(Number(e.target.value) || 0)}
          />

          <Text size="2">Auto backup</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={backupEnabled}
              onCheckedChange={(checked) => setBackupEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Run scheduled backups (also requires a cron below)
            </Text>
          </Flex>

          <Text size="2">
            Backup cron (5-field){" "}
            <Link
              size="1"
              href="https://crontab.guru/"
              onClick={(e) => {
                e.preventDefault();
                void openExternal("https://crontab.guru/");
              }}
            >
              crontab.guru
            </Link>
          </Text>
          <Box>
            <TextField.Root
              value={backupCron}
              onChange={(e) => setBackupCron(e.target.value)}
              placeholder="e.g. 0 4 * * *  (every day at 04:00)"
            />
            <Box mt="1">
              {backupEnabled && !backupCron.trim() ? (
                <Text size="1" color="red">
                  A cron expression is required while auto backup is enabled.
                </Text>
              ) : (
                <CronStatusHint status={backupCronStatus} />
              )}
            </Box>
          </Box>

        </Box>
      ) : (
        <Box className="schedule-grid">
          <Text size="2" color="gray">Auto restart</Text>
          <Text size="2">
            {config ? ((config.restartEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">Daily restart</Text>
          <Text size="2">
            {displayHour}:{displayMinute}
          </Text>

          <Text size="2" color="gray">Timezone</Text>
          <Text size="2" className="mono">{config?.restartTz ?? "—"}</Text>

          <Text size="2" color="gray">Warning lead</Text>
          <Text size="2">
            {config ? `${config.restartWarningDurationSecs}s` : "—"}
          </Text>

          <Text size="2" color="gray">Warning frequency</Text>
          <Text size="2">
            {config ? `${config.restartWarningFrequencySecs}s` : "—"}
          </Text>

          <Text size="2" color="gray">Auto update</Text>
          <Text size="2">
            {config ? ((config.updateEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">Update apply lead</Text>
          <Text size="2">
            {config ? `${config.updateLeadSecs}s` : "—"}
          </Text>

          <Text size="2" color="gray">Auto backup</Text>
          <Text size="2">
            {config ? ((config.backupEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">Backup cron</Text>
          <Text size="2" className="mono">
            {config
              ? config.backupCron && config.backupCron.trim()
                ? config.backupCron
                : "disabled (manual only)"
              : "—"}
          </Text>

        </Box>
      )}

      {error ? (
        <Text size="1" color="red" mt="2">{error}</Text>
      ) : null}

      <Flex gap="2" mt="3" align="center" wrap="wrap">
        {editing ? (
          <>
            <Button size="1" onClick={save} disabled={busy}>
              {busy ? busyLabel : "Save"}
            </Button>
            <Button size="1" variant="soft" color="gray" onClick={cancelEdit} disabled={busy}>
              Cancel
            </Button>
          </>
        ) : null}
        {!editing && restartRequired ? (
          <Callout.Root color="amber" size="1" style={{ padding: "4px 10px" }}>
            <Callout.Text>
              Saved values differ from what the running service loaded — restart the service to apply them.
            </Callout.Text>
          </Callout.Root>
        ) : null}
      </Flex>
    </Box>
  );
}

function RunRow({
  run,
  logsState,
  onExpand,
  onRefreshLogs,
}: {
  run: RunDto;
  logsState: LogState | undefined;
  onExpand: () => void;
  onRefreshLogs: () => void;
}) {
  return (
    <details
      className="run-row"
      onToggle={(e) => {
        if ((e.currentTarget as HTMLDetailsElement).open) onExpand();
      }}
    >
      <summary className="run-row-summary">
        <Text size="1" className="mono" style={{ minWidth: 40, color: "var(--gray-9)" }}>
          #{run.id}
        </Text>
        <Text size="2" style={{ flex: "0 0 auto", minWidth: 140 }}>
          {run.taskId}
        </Text>
        <Badge color={statusColor(run.status)}>{run.status}</Badge>
        <Text size="1" className="mono" style={{ color: "var(--gray-10)" }}>
          {formatDateTime(run.startedAt)}
        </Text>
        <Text size="1" className="mono" style={{ color: "var(--gray-10)", marginLeft: "auto" }}>
          {run.durationMs != null ? `${(run.durationMs / 1000).toFixed(1)}s` : "—"}
        </Text>
      </summary>
      <Box className="run-row-body">
        <Flex justify="between" align="center" mb="2">
          <Text size="1" color="gray">
            {run.trigger}
            {run.dryRun ? " · dry-run" : ""}
            {run.error ? ` · error: ${run.error}` : ""}
          </Text>
          <Button
            size="1"
            variant="ghost"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onRefreshLogs();
            }}
          >
            {logsState?.status === "loading" ? "Loading…" : "Refresh logs"}
          </Button>
        </Flex>
        <Box className="run-log-box">
          {logsState === undefined || logsState.status === "loading" ? (
            <Text color="gray" size="1">
              Loading logs…
            </Text>
          ) : logsState.status === "error" ? (
            <Text color="red" size="1">
              {logsState.error}
            </Text>
          ) : logsState.logs.length === 0 ? (
            <Text color="gray" size="1">
              No log entries for this run.
            </Text>
          ) : (
            logsState.logs.map((log) => (
              <div key={log.id}>
                <span className={`log-level-${log.level}`}>{log.level.toUpperCase()}</span>
                <span className="log-ts">{formatTime(log.createdAt)}</span>
                {log.message}
              </div>
            ))
          )}
        </Box>
      </Box>
    </details>
  );
}

function CronStatusHint({
  status,
}: {
  status:
    | { state: "idle" }
    | { state: "validating" }
    | { state: "ok"; tz: string; next: string[] }
    | { state: "error"; message: string };
}) {
  if (status.state === "idle") {
    return (
      <Text size="1" color="gray">
        Empty = disabled. Standard 5-field cron (min hour day month dow) in your configured timezone.
      </Text>
    );
  }
  if (status.state === "validating") {
    return (
      <Text size="1" color="gray">
        Checking…
      </Text>
    );
  }
  if (status.state === "error") {
    return (
      <Text size="1" color="red">
        {status.message}
      </Text>
    );
  }
  return (
    <Box>
      <Text size="1" color="green">
        Valid. Next runs ({status.tz}):
      </Text>
      <Flex direction="column" mt="1" gap="1">
        {status.next.map((time) => (
          <Text key={time} size="1" className="mono" color="gray">
            {time}
          </Text>
        ))}
      </Flex>
    </Box>
  );
}

function statusColor(s: string): "gray" | "green" | "red" | "amber" {
  if (s === "success") return "green";
  if (s === "failed") return "red";
  if (s === "running") return "amber";
  return "gray";
}

function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

// Parse a numeric text field, ignoring non-digits, and clamp to [0, max].
// Used by the HH:MM restart fields so they can show zero-padded values
// (a native number input strips leading zeros).
function clampInt(raw: string, max: number): number {
  const n = Number(raw.replace(/\D/g, ""));
  if (!Number.isFinite(n) || n < 0) return 0;
  return Math.min(Math.trunc(n), max);
}
