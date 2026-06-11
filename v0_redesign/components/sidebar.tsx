"use client"

import {
  LayoutDashboard,
  ArrowUpCircle,
  Boxes,
  Users,
  Terminal,
  Gift,
  CalendarClock,
  Server,
  Plus,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { server } from "@/lib/dune-data"

export type ViewId =
  | "dashboard"
  | "update"
  | "pods"
  | "users"
  | "admin"
  | "welcome"
  | "tasks"

const nav: { id: ViewId; label: string; icon: typeof LayoutDashboard }[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "update", label: "Update", icon: ArrowUpCircle },
  { id: "pods", label: "Pods", icon: Boxes },
  { id: "users", label: "Users", icon: Users },
  { id: "admin", label: "Admin", icon: Terminal },
  { id: "welcome", label: "Welcome Package", icon: Gift },
  { id: "tasks", label: "Automated tasks", icon: CalendarClock },
]

export function Sidebar({
  active,
  onSelect,
}: {
  active: ViewId
  onSelect: (id: ViewId) => void
}) {
  return (
    <aside className="flex h-full w-16 shrink-0 flex-col border-r border-border bg-sidebar md:w-60">
      {/* Brand */}
      <div className="flex items-center gap-2.5 border-b border-border px-3 py-3.5 md:px-4">
        <div className="chamfer-sm flex size-8 shrink-0 items-center justify-center bg-primary font-display text-base text-primary-foreground">
          D
        </div>
        <div className="hidden min-w-0 md:block">
          <p className="truncate font-display text-sm leading-tight text-foreground">
            Dune Server Manager
          </p>
          <p className="font-display text-[10px] text-muted-foreground">
            Operator Console
          </p>
        </div>
      </div>

      {/* Server switcher */}
      <div className="border-b border-border px-2 py-3 md:px-3">
        <p className="hidden px-1 pb-1.5 text-[10px] font-medium uppercase tracking-widest text-muted-foreground md:block">
          Servers (1)
        </p>
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-md bg-accent px-2 py-2 text-left transition-colors"
        >
          <span className="relative flex size-2 shrink-0">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-success/60" />
            <span className="relative inline-flex size-2 rounded-full bg-success" />
          </span>
          <span className="hidden truncate text-sm font-medium text-foreground md:block">
            {server.name}
          </span>
        </button>
        <button
          type="button"
          className="mt-1.5 flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Plus className="size-4 shrink-0" />
          <span className="hidden text-sm md:block">Add server</span>
        </button>
      </div>

      {/* Nav */}
      <nav className="flex-1 overflow-y-auto px-2 py-3 md:px-3">
        <ul className="space-y-0.5">
          {nav.map((item) => {
            const Icon = item.icon
            const isActive = active === item.id
            return (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => onSelect(item.id)}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-md px-2.5 py-2 text-sm font-medium transition-colors md:px-3",
                    isActive
                      ? "bg-primary/10 text-primary"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground",
                  )}
                  title={item.label}
                >
                  <Icon className="size-4 shrink-0" />
                  <span className="hidden truncate md:block">{item.label}</span>
                </button>
              </li>
            )
          })}
        </ul>
      </nav>

      {/* Footer */}
      <div className="border-t border-border px-3 py-3">
        <div className="hidden items-center gap-2 md:flex">
          <Server className="size-4 text-muted-foreground" />
          <div className="min-w-0">
            <p className="truncate font-mono text-[11px] text-muted-foreground">
              {server.host}
            </p>
            <p className="text-[11px] text-muted-foreground">up {server.uptime}</p>
          </div>
        </div>
        <Server className="mx-auto size-4 text-muted-foreground md:hidden" />
      </div>
    </aside>
  )
}
