"use client"

import { Play, RotateCcw, Square, Power, PauseCircle, PlayCircle, Users } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Sparkline } from "@/components/sparkline"
import { cn } from "@/lib/utils"
import type { VmStage, LifecyclePhase, Verdict } from "@/lib/dune-data"

const verdictMeta: Record<
  Verdict,
  { label: string; dot: string; text: string; ring: string; bg: string }
> = {
  operational: {
    label: "Operational",
    dot: "bg-success",
    text: "text-success",
    ring: "ring-success/30",
    bg: "bg-success/10",
  },
  degraded: {
    label: "Degraded",
    dot: "bg-warning",
    text: "text-warning",
    ring: "ring-warning/30",
    bg: "bg-warning/10",
  },
  down: {
    label: "Down",
    dot: "bg-destructive",
    text: "text-destructive",
    ring: "ring-destructive/30",
    bg: "bg-destructive/10",
  },
}

const stages: { id: VmStage; label: string; icon: typeof Power }[] = [
  { id: "off", label: "Off", icon: Power },
  { id: "saved", label: "Saved", icon: PauseCircle },
  { id: "running", label: "Running", icon: PlayCircle },
]

const lifecycleLabel: Record<LifecyclePhase, string> = {
  stopped: "BattleGroup stopped",
  starting: "BattleGroup starting…",
  healthy: "BattleGroup healthy",
  degraded: "BattleGroup degraded",
  stopping: "BattleGroup stopping…",
}

export function SystemStatusHeader({
  verdict,
  detail,
  serverName,
  host,
  uptime,
  activePlayers,
  capacity,
  playerTrend,
  vmName,
  stage,
  lifecycle,
}: {
  verdict: Verdict
  detail: string
  serverName: string
  host: string
  uptime: string
  activePlayers: number
  capacity: number
  playerTrend: number[]
  vmName: string
  stage: VmStage
  lifecycle: LifecyclePhase
}) {
  const v = verdictMeta[verdict]
  const stageIndex = stages.findIndex((s) => s.id === stage)
  const canStartBg = stage === "running" && lifecycle === "stopped"
  const bgRunning = lifecycle === "healthy" || lifecycle === "degraded"
  const trendTone = verdict === "down" ? "destructive" : verdict === "degraded" ? "warning" : "success"

  return (
    <section
      className={cn(
        "bracket chamfer border border-border bg-card bg-gradient-to-br from-dusk/12 via-card to-card p-4 ring-1 ring-inset md:p-5",
        v.ring,
      )}
      aria-label="System status"
    >
      {/* Top row: verdict + identity + players */}
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div className="flex items-center gap-3.5">
          <span className={cn("flex size-11 items-center justify-center rounded-lg", v.bg)}>
            <span className="relative flex size-3">
              {verdict !== "operational" && (
                <span className={cn("absolute inline-flex size-full animate-ping rounded-full opacity-60", v.dot)} />
              )}
              <span className={cn("relative inline-flex size-3 rounded-full", v.dot)} />
            </span>
          </span>
          <div className="min-w-0">
            <div className="flex items-center gap-2.5">
              <h2 className="font-display text-xl tracking-wide text-foreground">{serverName}</h2>
              <span className={cn("font-display text-sm", v.text)}>{v.label}</span>
            </div>
            <p className="truncate font-mono text-xs text-muted-foreground">
              {host} · up {uptime}
            </p>
          </div>
        </div>

        {/* Live players with trend */}
        <div className="flex items-center gap-4 border border-border bg-background/40 px-4 py-2.5">
          <div>
            <div className="flex items-center gap-1.5 font-display text-[11px] text-muted-foreground">
              <Users className="size-3" /> Players
            </div>
            <p className="mt-0.5 font-mono text-xl leading-none text-foreground">
              {activePlayers}
              <span className="text-sm text-muted-foreground">/{capacity}</span>
            </p>
          </div>
          <Sparkline data={playerTrend} tone={trendTone} width={104} height={32} aria-label="Player count trend" />
        </div>
      </div>

      {/* Detail line */}
      <p className="mt-3 border-t border-border pt-3 text-sm text-muted-foreground">{detail}</p>

      {/* Control deck: VM power rail + lifecycle actions */}
      <div className="mt-4 flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="flex flex-wrap items-center gap-3">
          <span className="font-display text-[11px] text-muted-foreground">
            {vmName}
          </span>
          <div
            className="chamfer-sm flex items-stretch overflow-hidden border border-border"
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
          <span className="flex items-center gap-2 text-xs">
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
            <span className="font-mono text-muted-foreground">{lifecycleLabel[lifecycle]}</span>
          </span>
        </div>

        {/* Lifecycle actions — destructive isolated to the right */}
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" className="gap-1.5" disabled={!canStartBg}>
            <Play className="size-3.5" /> Start
          </Button>
          <Button size="sm" variant="secondary" className="gap-1.5" disabled={!bgRunning}>
            <RotateCcw className="size-3.5" /> Restart
          </Button>
          <span className="mx-1 hidden h-5 w-px bg-border sm:block" aria-hidden="true" />
          <Button
            size="sm"
            variant="outline"
            disabled={!bgRunning}
            className="gap-1.5 border-destructive/40 text-destructive hover:bg-destructive/10 hover:text-destructive"
          >
            <Square className="size-3.5" /> Stop
          </Button>
        </div>
      </div>
    </section>
  )
}
