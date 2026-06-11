"use client"

import { ChevronRight, Trash2, FileDown } from "lucide-react"
import { logs } from "@/lib/dune-data"
import { cn } from "@/lib/utils"

export function LogsPanel({
  open,
  onToggle,
}: {
  open: boolean
  onToggle: () => void
}) {
  if (!open) {
    return (
      <button
        type="button"
        onClick={onToggle}
        className="hidden h-full w-9 shrink-0 items-start justify-center border-l border-border bg-sidebar pt-4 text-muted-foreground transition-colors hover:text-foreground lg:flex"
        aria-label="Open logs panel"
      >
        <span className="rotate-180 [writing-mode:vertical-rl] text-xs font-medium uppercase tracking-widest">
          Logs
        </span>
      </button>
    )
  }

  return (
    <aside className="hidden h-full w-80 shrink-0 flex-col border-l border-border bg-sidebar lg:flex">
      <div className="flex items-start justify-between gap-2 border-b border-border px-4 py-3">
        <div>
          <h2 className="text-sm font-semibold text-foreground">Logs</h2>
          <p className="text-xs text-muted-foreground">{logs.length} entries</p>
        </div>
        <button
          type="button"
          onClick={onToggle}
          className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          aria-label="Collapse logs panel"
        >
          <ChevronRight className="size-4" />
        </button>
      </div>

      <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-2.5">
        <div className="flex items-center gap-2">
          <select
            className="rounded-md border border-border bg-background px-2 py-1 text-xs text-foreground outline-none focus:ring-1 focus:ring-ring"
            defaultValue="Info"
            aria-label="Log level filter"
          >
            <option>Info</option>
            <option>Warn</option>
            <option>Error</option>
          </select>
          <span className="rounded-md border border-primary/40 bg-primary/10 px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-primary">
            This server
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label="Export logs"
          >
            <FileDown className="size-4" />
          </button>
          <button
            type="button"
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label="Clear logs"
          >
            <Trash2 className="size-4" />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-2">
        <ul className="space-y-2.5">
          {logs.map((log, i) => (
            <li key={i} className="font-mono text-xs leading-relaxed">
              <div className="flex gap-2">
                <span className="shrink-0 text-muted-foreground">{log.time}</span>
                <span
                  className={cn(
                    "shrink-0 font-semibold",
                    log.level === "INFO" && "text-primary",
                    log.level === "WARN" && "text-warning",
                    log.level === "ERROR" && "text-destructive",
                  )}
                >
                  {log.level}
                </span>
              </div>
              <p className="mt-0.5 pl-0 text-muted-foreground">{log.message}</p>
            </li>
          ))}
        </ul>
      </div>
    </aside>
  )
}
