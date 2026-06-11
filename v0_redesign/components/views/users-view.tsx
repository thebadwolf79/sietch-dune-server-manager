"use client"

import { useMemo, useState } from "react"
import { Search } from "lucide-react"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/status-pill"
import { Panel, ViewContainer } from "@/components/primitives"
import { users } from "@/lib/dune-data"

export function UsersView() {
  const [query, setQuery] = useState("")
  const [onlineOnly, setOnlineOnly] = useState(false)
  const [autoRefresh, setAutoRefresh] = useState(true)

  const filtered = useMemo(() => {
    return users.filter((u) => {
      if (onlineOnly && !u.online) return false
      const q = query.toLowerCase().trim()
      if (!q) return true
      return u.name.toLowerCase().includes(q) || u.flsId.toLowerCase().includes(q)
    })
  }, [query, onlineOnly])

  return (
    <ViewContainer>
      <div className="space-y-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search name or FLS id…"
              className="pl-9"
            />
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Switch checked={onlineOnly} onCheckedChange={setOnlineOnly} />
              Online only
            </label>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Switch checked={autoRefresh} onCheckedChange={setAutoRefresh} />
              Auto-refresh
            </label>
            <Button variant="ghost" size="sm">Refresh</Button>
            <span className="text-sm text-muted-foreground">
              {filtered.length} of {users.length}
            </span>
          </div>
        </div>

        <Panel className="overflow-hidden">
          <div className="hidden grid-cols-12 gap-2 border-b border-border px-4 py-2.5 sm:grid">
            <span className="col-span-3 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Name</span>
            <span className="col-span-3 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">FLS ID</span>
            <span className="col-span-1 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Level</span>
            <span className="col-span-1 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Part.</span>
            <span className="col-span-1 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Status</span>
            <span className="col-span-2 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Last seen</span>
            <span className="col-span-1 text-right text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Actions</span>
          </div>
          {filtered.length === 0 ? (
            <p className="px-4 py-8 text-center text-sm text-muted-foreground">No users match your filters.</p>
          ) : (
            filtered.map((u) => (
              <div
                key={u.flsId}
                className="grid grid-cols-2 gap-2 border-b border-border px-4 py-3 last:border-0 sm:grid-cols-12 sm:items-center"
              >
                <span className="col-span-2 text-sm font-medium text-foreground sm:col-span-3">{u.name}</span>
                <span className="col-span-2 font-mono text-xs text-muted-foreground sm:col-span-3">{u.flsId}</span>
                <span className="font-mono text-sm text-muted-foreground sm:col-span-1">{u.level}</span>
                <span className="text-right font-mono text-sm text-foreground sm:col-span-1 sm:text-left">{u.partition}</span>
                <span className="col-span-2 sm:col-span-1">
                  <StatusPill tone={u.online ? "success" : "muted"} dot={false}>
                    {u.online ? "Online" : "Offline"}
                  </StatusPill>
                </span>
                <span className="col-span-2 font-mono text-xs text-muted-foreground sm:col-span-2">{u.lastSeen}</span>
                <span className="col-span-2 text-right sm:col-span-1">
                  <button type="button" className="text-sm text-primary hover:underline">
                    Actions
                  </button>
                </span>
              </div>
            ))
          )}
        </Panel>
      </div>
    </ViewContainer>
  )
}
