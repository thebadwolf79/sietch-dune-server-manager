"use client"

import { Power, PauseCircle, PlayCircle, RotateCcw, Square, Play } from "lucide-react"
import { Button } from "@/components/ui/button"
import { SectionLabel } from "@/components/primitives"
import { cn } from "@/lib/utils"
import type { VmStage, LifecyclePhase } from "@/lib/dune-data"

const stages: { id: VmStage; label: string; icon: typeof Power }[] = [
  { id: "off", label: "Off", icon: Power },
  { id: "saved", label: "Saved", icon: PauseCircle },
  { id: "running", label: "Running", icon: PlayCircle },
]

const lifecycleTone: Record<LifecyclePhase, string> = {
  stopped: "text-muted-foreground",
  starting: "text-warning",
  healthy: "text-success",
  degraded: "text-warning",
  stopping: "text-warning",
}

const lifecycleLabel: Record<LifecyclePhase, string> = {
  stopped: "BattleGroup stopped",
  starting: "BattleGroup starting…",
  healthy: "BattleGroup healthy",
  degraded: "BattleGroup degraded",
  stopping: "BattleGroup stopping…",
}

export function VmPowerControls({
  vmName,
  stage,
  lifecycle,
}: {
  vmName: string
  stage: VmStage
  lifecycle: LifecyclePhase
}) {
  const stageIndex = stages.findIndex((s) => s.id === stage)
  const canStartBg = stage === "running" && lifecycle === "stopped"
  const bgRunning = lifecycle === "healthy" || lifecycle === "degraded"

  return (
    <div className="rounded-lg border border-border bg-card">
      {/* VM power state */}
      <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <SectionLabel>Hyper-V virtual machine</SectionLabel>
          <p className="mt-1 truncate font-mono text-sm text-foreground">{vmName}</p>
        </div>

        {/* Segmented power stage indicator */}
        <div
          className="flex items-stretch overflow-hidden rounded-md border border-border"
          role="group"
          aria-label="VM power state"
        >
          {stages.map((s, i) => {
            const Icon = s.icon
            const active = s.id === stage
            const reached = i <= stageIndex
            return (
              <button
                key={s.id}
                type="button"
                aria-pressed={active}
                className={cn(
                  "flex items-center gap-1.5 border-r border-border px-3 py-1.5 text-xs font-medium uppercase tracking-wide transition-colors last:border-r-0",
                  active
                    ? "bg-primary/15 text-primary"
                    : reached
                      ? "bg-secondary text-foreground hover:bg-accent"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                <Icon className="size-3.5" />
                {s.label}
              </button>
            )
          })}
        </div>
      </div>

      {/* BattleGroup lifecycle */}
      <div className="flex flex-col gap-3 border-t border-border p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-2.5">
          <span
            className={cn(
              "size-2 shrink-0 rounded-full",
              lifecycle === "healthy" && "bg-success",
              lifecycle === "degraded" && "bg-warning",
              (lifecycle === "starting" || lifecycle === "stopping") && "animate-pulse bg-warning",
              lifecycle === "stopped" && "bg-muted-foreground",
            )}
            aria-hidden="true"
          />
          <span className={cn("font-mono text-sm", lifecycleTone[lifecycle])}>
            {lifecycleLabel[lifecycle]}
          </span>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="default" size="sm" className="gap-1.5" disabled={!canStartBg}>
            <Play className="size-3.5" /> Start BattleGroup
          </Button>
          <Button variant="secondary" size="sm" className="gap-1.5" disabled={!bgRunning}>
            <RotateCcw className="size-3.5" /> Restart
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={!bgRunning}
            className="gap-1.5 border-destructive/40 text-destructive hover:bg-destructive/10 hover:text-destructive"
          >
            <Square className="size-3.5" /> Stop BattleGroup
          </Button>
        </div>
      </div>
    </div>
  )
}
