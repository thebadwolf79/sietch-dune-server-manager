"use client"

import { useState } from "react"
import { ShieldCheck, ShieldAlert, AlertTriangle, Info, Wrench } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Panel, SectionLabel } from "@/components/primitives"
import { cn } from "@/lib/utils"
import type { HealthSeverity, HostHealthReport } from "@/lib/dune-data"

const sevMeta: Record<
  HealthSeverity,
  { label: string; text: string; bg: string; border: string; icon: typeof Info }
> = {
  ok: { label: "OK", text: "text-success", bg: "bg-success/15", border: "border-success/30", icon: ShieldCheck },
  info: { label: "Info", text: "text-foreground", bg: "bg-muted", border: "border-border", icon: Info },
  warning: { label: "Warning", text: "text-warning", bg: "bg-warning/15", border: "border-warning/30", icon: AlertTriangle },
  critical: { label: "Critical", text: "text-destructive", bg: "bg-destructive/15", border: "border-destructive/30", icon: ShieldAlert },
}

// critical/warning sort first
const order: Record<HealthSeverity, number> = { critical: 0, warning: 1, info: 2, ok: 3 }

export function HostHealthPanel({ report }: { report: HostHealthReport }) {
  const [confirming, setConfirming] = useState<string | null>(null)
  const overall = sevMeta[report.overallSeverity]
  const OverallIcon = overall.icon
  const findings = [...report.findings].sort((a, b) => order[a.severity] - order[b.severity])

  return (
    <Panel className="overflow-hidden">
      {/* Header */}
      <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-3">
          <span className={cn("chamfer-sm flex size-9 items-center justify-center", overall.bg)}>
            <OverallIcon className={cn("size-5", overall.text)} />
          </span>
          <div>
            <h3 className="font-display text-sm text-foreground">Host Health &amp; Hardening</h3>
            <p className="text-xs text-muted-foreground">{report.summary}</p>
          </div>
        </div>
        <span
          className={cn(
            "inline-flex w-fit items-center gap-1.5 rounded-full border px-2.5 py-0.5 font-display text-[11px] tracking-wide",
            overall.bg,
            overall.text,
            overall.border,
          )}
        >
          <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
          {overall.label}
        </span>
      </div>

      {/* Metric chips */}
      <div className="flex flex-wrap gap-2 border-b border-border p-4">
        {report.metrics.map((m) => {
          const meta = sevMeta[m.severity]
          return (
            <div
              key={m.label}
              className={cn("rounded-md border px-3 py-2", meta.border, m.severity === "ok" ? "bg-card" : meta.bg)}
            >
              <SectionLabel>{m.label}</SectionLabel>
              <p className={cn("mt-0.5 font-mono text-sm", m.severity === "ok" ? "text-foreground" : meta.text)}>
                {m.value}
              </p>
            </div>
          )
        })}
      </div>

      {/* Findings */}
      <ul>
        {findings.map((f) => {
          const meta = sevMeta[f.severity]
          const Icon = meta.icon
          const isConfirming = confirming === f.id
          return (
            <li key={f.id} className="border-b border-border p-4 last:border-0">
              <div className="flex gap-3">
                <Icon className={cn("mt-0.5 size-4 shrink-0", meta.text)} />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium text-foreground">{f.title}</span>
                    <span
                      className={cn(
                        "chamfer-sm border px-1.5 py-px font-display text-[10px]",
                        meta.border,
                        meta.text,
                      )}
                    >
                      {meta.label}
                    </span>
                  </div>
                  <p className="mt-1 text-sm text-muted-foreground">{f.detail}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    <span className="font-medium text-foreground/80">Recommendation: </span>
                    {f.recommendation}
                  </p>

                  {f.fixLabel && (
                    <div className="mt-2.5">
                      {isConfirming ? (
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-xs text-warning">Apply this fix on the host?</span>
                          <Button
                            size="sm"
                            className="h-7 gap-1.5"
                            onClick={() => setConfirming(null)}
                          >
                            Confirm
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-7"
                            onClick={() => setConfirming(null)}
                          >
                            Cancel
                          </Button>
                        </div>
                      ) : (
                        <Button
                          size="sm"
                          variant="outline"
                          className="h-7 gap-1.5"
                          onClick={() => setConfirming(f.id)}
                        >
                          <Wrench className="size-3.5" /> {f.fixLabel}
                        </Button>
                      )}
                    </div>
                  )}
                </div>
              </div>
            </li>
          )
        })}
      </ul>
    </Panel>
  )
}
