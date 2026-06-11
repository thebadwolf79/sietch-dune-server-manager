"use client"

import { useState } from "react"
import { ChevronRight } from "lucide-react"
import { StatusPill } from "@/components/status-pill"
import { Panel, SectionLabel, ViewContainer } from "@/components/primitives"
import { systemPods, mapPods, type PodGroup } from "@/lib/dune-data"
import { cn } from "@/lib/utils"

function toneFor(status: PodGroup["status"]) {
  if (status === "ready") return "success" as const
  if (status === "problem") return "destructive" as const
  return "muted" as const
}

function PodRow({ pod }: { pod: PodGroup }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="border-b border-border last:border-0">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-accent/50"
      >
        <ChevronRight
          className={cn("size-4 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
        />
        <span className="text-sm font-medium text-foreground">{pod.name}</span>
        <span className="font-mono text-xs text-muted-foreground">{pod.slug}</span>
        <div className="ml-auto flex items-center gap-3">
          <StatusPill tone={toneFor(pod.status)}>
            {pod.status === "ready" ? "Ready" : pod.status === "problem" ? "Problem" : "Stopped"}
          </StatusPill>
          <span className="hidden text-sm text-muted-foreground sm:block">{pod.detail}</span>
        </div>
      </button>
      {open && (
        <div className="border-t border-border bg-background/40 px-4 py-3 pl-11">
          <p className="text-sm text-muted-foreground">{pod.detail}</p>
          <div className="mt-2 flex flex-wrap gap-x-6 gap-y-1 font-mono text-xs text-muted-foreground">
            <span>namespace: {pod.slug}</span>
            <span>restarts: 0</span>
            <span>image: dune/{pod.slug}:latest</span>
          </div>
        </div>
      )}
    </div>
  )
}

export function PodsView() {
  return (
    <ViewContainer>
      <div className="space-y-6">
        <div>
          <SectionLabel className="mb-2">System pods</SectionLabel>
          <Panel className="overflow-hidden">
            {systemPods.map((pod) => (
              <PodRow key={pod.slug} pod={pod} />
            ))}
          </Panel>
        </div>
        <div>
          <SectionLabel className="mb-2">Map server pods</SectionLabel>
          <Panel className="overflow-hidden">
            {mapPods.map((pod) => (
              <PodRow key={pod.slug} pod={pod} />
            ))}
          </Panel>
        </div>
      </div>
    </ViewContainer>
  )
}
