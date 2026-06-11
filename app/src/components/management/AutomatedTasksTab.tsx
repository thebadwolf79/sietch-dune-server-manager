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
    <Box mt="3" style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      <ScheduleSettings tunnelId={tunnelId} server={server} onAfterRestart={onAfterRestart} />

      <Box>
        <Flex justify="between" align="start" gap="3" wrap="wrap" mb="2">
          <Text size="2" color="gray">
            Run the scheduled maintenance tasks manually. Each run records its own log entries below.
          </Text>
          <button
            type="button"
            onClick={reload}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "4px 10px",
              fontSize: "12px",
              cursor: "pointer",
              border: "1px solid var(--color-border-hair)",
              background: "var(--color-bg-elevated)",
              borderRadius: "var(--radius-1)",
              color: "var(--color-text-primary)",
              transition: "all 140ms var(--ease-out)",
            }}
            className="chamfer-sm"
          >
            Refresh
          </button>
        </Flex>
        <Flex gap="2" wrap="wrap" mt="1" mb="2">
          {DIRECT_TASKS.map((t) => (
            <button
              key={t.id}
              type="button"
              disabled={busyTrigger === t.id}
              onClick={() => trigger(t.id)}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: busyTrigger === t.id ? "not-allowed" : "pointer",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-elevated)",
                borderRadius: "var(--radius-1)",
                color: "var(--color-text-primary)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              {busyTrigger === t.id ? `Running ${t.label}…` : t.label}
            </button>
          ))}
          <button
            type="button"
            disabled={busyTrigger === "restart-notice"}
            onClick={() => setNoticeOpen(true)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "6px 12px",
              fontSize: "12.5px",
              cursor: busyTrigger === "restart-notice" ? "not-allowed" : "pointer",
              border: "1px solid var(--color-border-hair)",
              background: "var(--color-bg-elevated)",
              borderRadius: "var(--radius-1)",
              color: "var(--color-text-primary)",
              transition: "all 140ms var(--ease-out)",
            }}
            className="chamfer-sm"
          >
            {busyTrigger === "restart-notice" ? "Sending notice…" : "Send restart notice…"}
          </button>
          <button
            type="button"
            onClick={() => setDumpPruneOpen(true)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "6px 12px",
              fontSize: "12.5px",
              cursor: "pointer",
              border: "1px solid rgba(214, 105, 94, 0.3)",
              background: "var(--color-bg-elevated)",
              borderRadius: "var(--radius-1)",
              color: "var(--color-err)",
              transition: "all 140ms var(--ease-out)",
            }}
            className="chamfer-sm"
          >
            Clean up database operations…
          </button>
        </Flex>
      </Box>

      {error && (
        <Text size="1" color="red" style={{ display: "block" }}>
          {error}
        </Text>
      )}

      <Box>
        <Text
          size="2"
          weight="bold"
          style={{
            display: "block",
            marginBottom: "8px",
            fontFamily: "var(--font-mono)",
            textTransform: "uppercase",
            letterSpacing: "0.05em",
          }}
        >
          Recent runs
        </Text>
        <Flex direction="column" gap="2">
          {runs.length === 0 ? (
            <Text color="gray" size="2">
              No runs yet.
            </Text>
          ) : null}
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

      <DumpPruneDialog open={dumpPruneOpen} onOpenChange={setDumpPruneOpen} tunnelId={tunnelId} />
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
          Publishes a ServerShutdown countdown to the game. If you provide a title + body, an
          additional Generic broadcast carries them as a banner.
        </Dialog.Description>
        <Flex direction="column" gap="3">
          <Box>
            <Text size="2" weight="medium">
              Lead time (seconds)
            </Text>
            <TextField.Root
              type="number"
              value={String(leadSecs)}
              onChange={(e) => setLeadSecs(Number(e.target.value) || 0)}
              mt="1"
            />
            <Text size="1" color="gray">
              How long until the restart fires. 1800 = 30 min.
            </Text>
          </Box>
          <Box>
            <Text size="2" weight="medium">
              Warning frequency (seconds)
            </Text>
            <TextField.Root
              type="number"
              value={String(frequencySecs)}
              onChange={(e) => setFrequencySecs(Number(e.target.value) || 0)}
              mt="1"
            />
            <Text size="1" color="gray">
              How often the game re-shows the countdown. 600 = every 10 min.
            </Text>
          </Box>
          <Box>
            <Text size="2" weight="medium">
              Custom title (optional)
            </Text>
            <TextField.Root
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Scheduled maintenance"
              mt="1"
            />
          </Box>
          <Box>
            <Text size="2" weight="medium">
              Custom body (optional)
            </Text>
            <TextArea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              rows={3}
              placeholder="Sent as an in-game banner alongside the countdown."
              mt="1"
            />
          </Box>
        </Flex>
        <Flex gap="2" mt="4" justify="end">
          <Dialog.Close>
            <Button variant="soft" color="gray">
              Cancel
            </Button>
          </Dialog.Close>
          <button
            type="button"
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
            className="action-btn"
            data-tone="accent"
            style={{ padding: "6px 14px", fontSize: "12.5px" }}
          >
            Send notice
          </button>
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

  const [hour, setHour] = useState(5);
  const [minute, setMinute] = useState(0);
  const [warnFreq, setWarnFreq] = useState(600);
  const [warnDur, setWarnDur] = useState(1800);
  const [updateLead, setUpdateLead] = useState(1800);
  const [tz, setTz] = useState("UTC");
  const [restartEnabled, setRestartEnabled] = useState(true);
  const [updateEnabled, setUpdateEnabled] = useState(true);
  const [backupEnabled, setBackupEnabled] = useState(true);
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
        const filtered = q ? all.filter((tz) => tz.toLowerCase().includes(q)) : all;
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
    <Box
      className="schedule-section bracket chamfer"
      style={{
        background: "var(--color-bg-panel)",
        border: "1px solid var(--color-border-hair)",
        padding: "16px",
      }}
    >
      <Flex justify="between" align="baseline" mb="2">
        <Text
          size="3"
          weight="bold"
          style={{
            fontFamily: "var(--font-mono)",
            textTransform: "uppercase",
            letterSpacing: "0.05em",
          }}
        >
          Schedule settings
        </Text>
        {!editing && config ? (
          <button
            type="button"
            onClick={startEdit}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "4px 10px",
              fontSize: "12px",
              cursor: "pointer",
              border: "1px solid var(--color-border-hair)",
              background: "var(--color-bg-elevated)",
              borderRadius: "var(--radius-1)",
              color: "var(--color-text-primary)",
            }}
            className="chamfer-sm"
          >
            Configure
          </button>
        ) : null}
      </Flex>
      <Text size="2" color="gray" as="div" mb="3">
        Stored in the service's sqlite. Saving restarts the service automatically so changes take
        effect immediately.
      </Text>

      {editing ? (
        <Box className="schedule-grid" style={{ borderTop: "1px solid var(--color-border-hair)", paddingTop: "16px" }}>
          <Text size="2" weight="medium">Auto restart</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={restartEnabled}
              onCheckedChange={(checked) => setRestartEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Run the daily restart and its warning broadcast
            </Text>
          </Flex>

          <Text size="2" weight="medium">Daily restart (HH:MM)</Text>
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

          <Text size="2" weight="medium">Timezone</Text>
          <Combobox
            value={tz}
            onChange={(v) => setTz(v)}
            loadOptions={loadTimezones}
            getOptionValue={(o: { name: string }) => o.name}
            resolveLabel={async (id) => id}
            renderOption={(o: { name: string }) => (
              <Text size="2" className="mono">
                {o.name}
              </Text>
            )}
            placeholder="Pick a timezone…"
            searchPlaceholder="Search IANA timezones…"
          />

          <Text size="2" weight="medium">Warning lead (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(warnDur)}
            onChange={(e) => setWarnDur(Number(e.target.value) || 0)}
          />

          <Text size="2" weight="medium">Warning frequency (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(warnFreq)}
            onChange={(e) => setWarnFreq(Number(e.target.value) || 0)}
          />

          <Text size="2" weight="medium">Auto update</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={updateEnabled}
              onCheckedChange={(checked) => setUpdateEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Check Steam for new builds and apply them automatically
            </Text>
          </Flex>

          <Text size="2" weight="medium">Update apply lead (seconds)</Text>
          <TextField.Root
            type="number"
            value={String(updateLead)}
            onChange={(e) => setUpdateLead(Number(e.target.value) || 0)}
          />

          <Text size="2" weight="medium">Auto backup</Text>
          <Flex align="center" gap="2">
            <Checkbox
              checked={backupEnabled}
              onCheckedChange={(checked) => setBackupEnabled(Boolean(checked))}
            />
            <Text size="2" color="gray">
              Run scheduled backups (also requires a cron below)
            </Text>
          </Flex>

          <Text size="2" weight="medium">
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
        <Box
          className="schedule-grid"
          style={{ borderTop: "1px solid var(--color-border-hair)", paddingTop: "16px" }}
        >
          <Text size="2" color="gray">
            Auto restart
          </Text>
          <Text size="2">
            {config ? ((config.restartEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">
            Daily restart
          </Text>
          <Text size="2">
            {displayHour}:{displayMinute}
          </Text>

          <Text size="2" color="gray">
            Timezone
          </Text>
          <Text size="2" className="mono">
            {config?.restartTz ?? "—"}
          </Text>

          <Text size="2" color="gray">
            Warning lead
          </Text>
          <Text size="2">{config ? `${config.restartWarningDurationSecs}s` : "—"}</Text>

          <Text size="2" color="gray">
            Warning frequency
          </Text>
          <Text size="2">{config ? `${config.restartWarningFrequencySecs}s` : "—"}</Text>

          <Text size="2" color="gray">
            Auto update
          </Text>
          <Text size="2">
            {config ? ((config.updateEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">
            Update apply lead
          </Text>
          <Text size="2">{config ? `${config.updateLeadSecs}s` : "—"}</Text>

          <Text size="2" color="gray">
            Auto backup
          </Text>
          <Text size="2">
            {config ? ((config.backupEnabled ?? true) ? "enabled" : "disabled") : "—"}
          </Text>

          <Text size="2" color="gray">
            Backup cron
          </Text>
          <Text size="2" className="mono">
            {config
              ? config.backupCron && config.backupCron.trim()
                ? config.backupCron
                : "disabled (manual only)"
              : "—"}
          </Text>
        </Box>
      )}

      {error && (
        <Text size="1" color="red" mt="2" style={{ display: "block" }}>
          {error}
        </Text>
      )}

      <Flex gap="2" mt="3" align="center" wrap="wrap">
        {editing ? (
          <>
            <button
              type="button"
              onClick={save}
              disabled={busy}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: busy ? "not-allowed" : "pointer",
                border: "1px solid var(--color-accent)",
                background: "var(--color-bg-panel)",
                color: "var(--color-accent-strong)",
                borderRadius: "var(--radius-1)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              {busy ? busyLabel : "Save"}
            </button>
            <button
              type="button"
              onClick={cancelEdit}
              disabled={busy}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                padding: "6px 12px",
                fontSize: "12.5px",
                cursor: busy ? "not-allowed" : "pointer",
                border: "1px solid var(--color-border-hair)",
                background: "var(--color-bg-elevated)",
                color: "var(--color-text-secondary)",
                borderRadius: "var(--radius-1)",
                transition: "all 140ms var(--ease-out)",
              }}
              className="chamfer-sm"
            >
              Cancel
            </button>
          </>
        ) : null}
        {!editing && restartRequired ? (
          <Callout.Root color="amber" size="1" style={{ padding: "4px 10px" }}>
            <Callout.Text>
              Saved values differ from what the running service loaded — restart the service to apply
              them.
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
      className="run-row bracket chamfer-sm"
      onToggle={(e) => {
        if ((e.currentTarget as HTMLDetailsElement).open) onExpand();
      }}
      style={{
        border: "1px solid var(--color-border-hair)",
        backgroundColor: "var(--color-bg-panel)",
        marginBottom: "6px",
        overflow: "hidden",
      }}
    >
      <summary
        className="run-row-summary"
        style={{
          padding: "10px 14px",
          display: "flex",
          alignItems: "center",
          gap: "12px",
          cursor: "pointer",
          userSelect: "none",
        }}
      >
        <Text size="1" className="mono" style={{ minWidth: 40, color: "var(--color-text-muted)" }}>
          #{run.id}
        </Text>
        <Text size="2" weight="medium" style={{ flex: "0 0 auto", minWidth: 140 }}>
          {run.taskId}
        </Text>
        <Badge color={statusColor(run.status)}>{run.status}</Badge>
        <Text size="1" className="mono" style={{ color: "var(--color-text-muted)" }}>
          {formatDateTime(run.startedAt)}
        </Text>
        <Text size="1" className="mono" style={{ color: "var(--color-text-muted)", marginLeft: "auto" }}>
          {run.durationMs != null ? `${(run.durationMs / 1000).toFixed(1)}s` : "—"}
        </Text>
      </summary>
      <Box
        className="run-row-body"
        p="3"
        style={{
          borderTop: "1px solid var(--color-border-hair)",
          backgroundColor: "rgba(0,0,0,0.15)",
        }}
      >
        <Flex justify="between" align="center" mb="2">
          <Text size="1" color="gray">
            {run.trigger}
            {run.dryRun ? " · dry-run" : ""}
            {run.error ? ` · error: ${run.error}` : ""}
          </Text>
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              onRefreshLogs();
            }}
            style={{
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              padding: "4px 8px",
              fontSize: "11px",
              cursor: "pointer",
              border: "1px solid var(--color-border-hair)",
              background: "var(--color-bg-elevated)",
              borderRadius: "var(--radius-1)",
              color: "var(--color-text-secondary)",
            }}
            className="chamfer-sm"
          >
            {logsState?.status === "loading" ? "Loading…" : "Refresh logs"}
          </button>
        </Flex>
        <Box
          className="run-log-box"
          style={{
            maxHeight: "320px",
            overflow: "auto",
            padding: "8px 10px",
            border: "1px solid var(--color-border-hair)",
            borderRadius: "var(--radius-2)",
            background: "var(--color-bg-base)",
            fontFamily: "var(--font-mono)",
            fontSize: "11.5px",
            lineHeight: "1.45",
          }}
        >
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
              <div key={log.id} style={{ display: "flex", gap: "6px", marginBottom: "2px" }}>
                <span className={`log-level-${log.level}`} style={{ fontWeight: 600 }}>
                  [{log.level.toUpperCase()}]
                </span>
                <span className="log-ts" style={{ color: "var(--color-text-muted)" }}>
                  {formatTime(log.createdAt)}
                </span>
                <span style={{ color: "var(--color-text-secondary)" }}>{log.message}</span>
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
