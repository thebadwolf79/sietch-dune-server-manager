"use client"

import { RefreshCw, Info, PanelRightOpen } from "lucide-react"
import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/status-pill"
import { server } from "@/lib/dune-data"
import type { ViewId } from "@/components/sidebar"

const titles: Record<ViewId, string> = {
  dashboard: "Dashboard",
  update: "Update",
  pods: "Pods",
  users: "Users",
  admin: "Admin",
  welcome: "Welcome Package",
  tasks: "Automated tasks",
}

export function Topbar({
  view,
  logsOpen,
  onToggleLogs,
}: {
  view: ViewId
  logsOpen: boolean
  onToggleLogs: () => void
}) {
  return (
    <header className="shrink-0 border-b border-border bg-background/80 backdrop-blur">
      {/* Window-ish bar */}
      <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <h1 className="truncate font-display text-sm text-foreground">{titles[view]}</h1>
        </div>
        <div className="flex items-center gap-2">
          <StatusPill tone="warning" dot={false} className="hidden sm:inline-flex">
            Not checked
          </StatusPill>
          <Button variant="outline" size="sm" className="h-8 gap-1.5">
            <RefreshCw className="size-3.5" />
            <span className="hidden sm:inline">Check for updates</span>
          </Button>
          <Button variant="ghost" size="icon" className="size-8" aria-label="About">
            <Info className="size-4" />
          </Button>
          {!logsOpen && (
            <Button
              variant="ghost"
              size="icon"
              className="size-8 lg:hidden"
              onClick={onToggleLogs}
              aria-label="Open logs"
            >
              <PanelRightOpen className="size-4" />
            </Button>
          )}
        </div>
      </div>

      {/* Server identity strip */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
        <div className="flex items-center gap-2.5">
          <span className="h-7 w-1 bg-success" aria-hidden="true" />
          <span className="font-display text-lg tracking-wide text-foreground">{server.name}</span>
          <StatusPill tone="success">{server.state}</StatusPill>
        </div>
        <p className="font-mono text-xs text-muted-foreground">
          {server.host} · {server.battlegroup} · up {server.uptime}
        </p>
        <div className="ml-auto flex items-center gap-2">
          <Button variant="secondary" size="sm" className="h-8 gap-1.5">
            <RefreshCw className="size-3.5" />
            Refresh
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
          >
            Forget
          </Button>
        </div>
      </div>
    </header>
  )
}
