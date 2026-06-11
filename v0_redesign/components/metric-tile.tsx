"use client"

import { Database, Network, Compass, Clock, Layers, type LucideIcon } from "lucide-react"
import { Sparkline } from "@/components/sparkline"
import { cn } from "@/lib/utils"

type Tone = "muted" | "healthy" | "warning" | "danger"

const toneMeta: Record<Tone, { value: string; dot: string; iconBg: string; icon: string }> = {
  muted: { value: "text-foreground", dot: "bg-muted-foreground", iconBg: "bg-muted", icon: "text-muted-foreground" },
  healthy: { value: "text-success", dot: "bg-success", iconBg: "bg-success/15", icon: "text-success" },
  warning: { value: "text-warning", dot: "bg-warning", iconBg: "bg-warning/15", icon: "text-warning" },
  danger: { value: "text-destructive", dot: "bg-destructive", iconBg: "bg-destructive/15", icon: "text-destructive" },
}

export function MetricTile({
  label,
  value,
  tone = "muted",
  icon: Icon,
  mono = true,
  trend,
  trendTone = "accent",
  span,
}: {
  label: string
  value: string
  tone?: Tone
  icon?: LucideIcon
  mono?: boolean
  trend?: number[]
  trendTone?: "accent" | "success" | "warning" | "destructive" | "muted"
  span?: boolean
}) {
  const meta = toneMeta[tone]
  return (
    <div
      className={cn(
        "chamfer-sm flex min-w-0 flex-col justify-between border border-border bg-card p-3.5 transition-colors hover:border-primary/30",
        span && "sm:col-span-2",
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="flex items-center gap-1.5 font-display text-[11px] text-muted-foreground">
          {Icon && (
            <span className={cn("flex size-5 items-center justify-center rounded", meta.iconBg)}>
              <Icon className={cn("size-3", meta.icon)} />
            </span>
          )}
          {label}
        </span>
        {!Icon && tone !== "muted" && <span className={cn("size-1.5 rounded-full", meta.dot)} aria-hidden="true" />}
      </div>
      <div className="mt-2 flex items-end justify-between gap-2">
        <p className={cn("truncate text-sm", mono && "font-mono", meta.value)}>{value}</p>
        {trend && <Sparkline data={trend} tone={trendTone} width={64} height={22} aria-label={`${label} trend`} />}
      </div>
    </div>
  )
}

export const metricIcons = { Database, Network, Compass, Clock, Layers }
