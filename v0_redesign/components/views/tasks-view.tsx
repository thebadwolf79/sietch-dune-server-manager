"use client"

import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/status-pill"
import { Panel, ViewContainer } from "@/components/primitives"
import { schedule, taskRuns } from "@/lib/dune-data"

const scheduleRows: { label: string; value: string; mono?: boolean }[] = [
  { label: "Auto restart", value: schedule.autoRestart },
  { label: "Daily restart", value: schedule.dailyRestart, mono: true },
  { label: "Timezone", value: schedule.timezone, mono: true },
  { label: "Warning lead", value: schedule.warningLead },
  { label: "Warning frequency", value: schedule.warningFrequency },
  { label: "Auto update", value: schedule.autoUpdate },
  { label: "Update apply lead", value: schedule.updateApplyLead },
  { label: "Auto backup", value: schedule.autoBackup },
  { label: "Backup cron", value: schedule.backupCron, mono: true },
]

const manualTasks = [
  { label: "Backup", destructive: false },
  { label: "Welcome package scan", destructive: false },
  { label: "Check for server update", destructive: false },
  { label: "Apply server update", destructive: false },
  { label: "Restart server", destructive: false },
  { label: "Send restart notice…", destructive: false },
  { label: "Clean up database operations…", destructive: true },
]

export function TasksView() {
  return (
    <ViewContainer>
      <div className="space-y-6">
        {/* Schedule settings */}
        <Panel className="p-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-base font-semibold text-foreground">Schedule settings</h3>
              <p className="mt-1 max-w-xl text-sm leading-relaxed text-muted-foreground">
                Stored in the service&apos;s sqlite. Saving restarts the service automatically so changes take effect immediately.
              </p>
            </div>
            <Button variant="outline" size="sm">Configure</Button>
          </div>
          <dl className="mt-4 grid grid-cols-1 gap-x-8 border-t border-border pt-4 sm:grid-cols-2">
            {scheduleRows.map((row) => (
              <div
                key={row.label}
                className="flex items-center justify-between border-b border-border/60 py-2.5 last:border-0 sm:[&:nth-last-child(2)]:border-0"
              >
                <dt className="text-sm text-muted-foreground">{row.label}</dt>
                <dd className={row.mono ? "font-mono text-sm text-foreground" : "text-sm text-foreground"}>
                  {row.value}
                </dd>
              </div>
            ))}
          </dl>
        </Panel>

        {/* Manual tasks */}
        <div>
          <div className="mb-3 flex items-center justify-between gap-3">
            <p className="text-sm text-muted-foreground">
              Run the scheduled maintenance tasks manually. Each run records its own log entries below.
            </p>
            <Button variant="outline" size="sm">Refresh</Button>
          </div>
          <div className="flex flex-wrap gap-2">
            {manualTasks.map((t) => (
              <Button
                key={t.label}
                variant="outline"
                size="sm"
                className={t.destructive ? "text-destructive hover:text-destructive" : undefined}
              >
                {t.label}
              </Button>
            ))}
          </div>
        </div>

        {/* Recent runs */}
        <div>
          <h3 className="mb-2 text-sm font-semibold text-foreground">Recent runs</h3>
          <Panel className="overflow-hidden">
            {taskRuns.map((run) => (
              <div
                key={run.id}
                className="flex items-center gap-3 border-b border-border px-4 py-3 last:border-0"
              >
                <span className="w-16 shrink-0 font-mono text-xs text-muted-foreground">{run.id}</span>
                <span className="flex-1 truncate text-sm font-medium text-foreground">{run.task}</span>
                <StatusPill tone={run.status === "success" ? "success" : "destructive"} dot={false}>
                  {run.status}
                </StatusPill>
                <span className="hidden w-40 shrink-0 font-mono text-xs text-muted-foreground sm:block">
                  {run.when}
                </span>
                <span className="w-12 shrink-0 text-right font-mono text-xs text-muted-foreground">
                  {run.duration}
                </span>
              </div>
            ))}
          </Panel>
        </div>
      </div>
    </ViewContainer>
  )
}
