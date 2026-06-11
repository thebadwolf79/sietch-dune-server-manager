"use client"

import { useState } from "react"
import { Send, Trash2, ChevronDown } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Checkbox } from "@/components/ui/checkbox"
import { StatusPill } from "@/components/status-pill"
import { Panel, ViewContainer } from "@/components/primitives"
import { packageItems as initialItems } from "@/lib/dune-data"
import { cn } from "@/lib/utils"

export function WelcomeView() {
  const [messageEnabled, setMessageEnabled] = useState(false)
  const [packageEnabled, setPackageEnabled] = useState(true)
  const [contentsOpen, setContentsOpen] = useState(true)
  const [jsonMode, setJsonMode] = useState(false)
  const [items, setItems] = useState(initialItems)

  function updateQty(id: string, qty: number) {
    setItems((prev) => prev.map((it) => (it.id === id ? { ...it, qty } : it)))
  }
  function removeItem(id: string) {
    setItems((prev) => prev.filter((it) => it.id !== id))
  }

  return (
    <ViewContainer>
      <Panel className="overflow-hidden">
        {/* Header */}
        <div className="flex flex-col gap-3 border-b border-border px-4 py-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-base font-semibold text-foreground">Welcome automation</h3>
            <StatusPill tone={messageEnabled ? "success" : "muted"} dot={false}>
              {messageEnabled ? "message on" : "message off"}
            </StatusPill>
            <StatusPill tone={packageEnabled ? "success" : "muted"} dot={false}>
              {packageEnabled ? "package enabled" : "package off"}
            </StatusPill>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="sm">Refresh</Button>
            <Button variant="outline" size="sm">Run scan</Button>
            <Button size="sm">Save &amp; restart service</Button>
          </div>
        </div>

        {/* Welcome message */}
        <div className="space-y-4 border-b border-border px-4 py-5">
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-semibold text-foreground">Welcome message</h4>
            <Button variant="outline" size="sm" className="gap-1.5">
              <Send className="size-3.5" /> Test
            </Button>
          </div>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <Checkbox checked={messageEnabled} onCheckedChange={(v) => setMessageEnabled(Boolean(v))} />
            Enabled
          </label>
          <div className="grid gap-1.5">
            <span className="text-xs text-muted-foreground">Sender identity</span>
            <select
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring"
              defaultValue="Maren Shai (Offline) · 431C7B16E03F3F97"
            >
              <option>Maren Shai (Offline) · 431C7B16E03F3F97</option>
            </select>
          </div>
          <div className="grid gap-1.5">
            <span className="text-xs text-muted-foreground">Message</span>
            <Textarea
              placeholder="Welcome to BadWolf. Your starter kit has been delivered…"
              className="min-h-28"
              disabled={!messageEnabled}
            />
          </div>
        </div>

        {/* Welcome package */}
        <div className="space-y-4 px-4 py-5">
          <h4 className="text-sm font-semibold text-foreground">Welcome package</h4>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <Checkbox checked={packageEnabled} onCheckedChange={(v) => setPackageEnabled(Boolean(v))} />
            Enabled
          </label>

          <div className="flex items-center justify-between">
            <button
              type="button"
              onClick={() => setContentsOpen((o) => !o)}
              className="flex items-center gap-2 text-sm font-medium text-foreground"
            >
              <ChevronDown className={cn("size-4 transition-transform", !contentsOpen && "-rotate-90")} />
              Package contents
              <StatusPill tone="muted" dot={false}>{items.length} items</StatusPill>
            </button>
            <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <Checkbox checked={jsonMode} onCheckedChange={(v) => setJsonMode(Boolean(v))} />
              JSON mode
            </label>
          </div>

          {contentsOpen && !jsonMode && (
            <div className="overflow-hidden rounded-md border border-border">
              <div className="flex items-center gap-2 border-b border-border bg-background/40 px-3 py-2">
                <span className="flex-1 text-sm font-semibold text-foreground">Item</span>
                <span className="w-20 text-sm font-semibold text-foreground">Qty</span>
                <span className="w-8" />
              </div>
              <div className="max-h-[420px] overflow-y-auto">
                {items.map((item) => (
                  <div key={item.id} className="flex items-center gap-2 border-b border-border px-3 py-2 last:border-0">
                    <div className="flex-1 truncate rounded-md border border-input bg-background px-3 py-2 text-sm">
                      <span className="text-foreground">{item.label}</span>
                      <span className="text-muted-foreground"> · {item.schematic}</span>
                    </div>
                    <Input
                      type="number"
                      value={item.qty}
                      onChange={(e) => updateQty(item.id, Number(e.target.value))}
                      className="w-20 font-mono"
                    />
                    <button
                      type="button"
                      onClick={() => removeItem(item.id)}
                      className="rounded-md p-2 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                      aria-label={`Remove ${item.label}`}
                    >
                      <Trash2 className="size-4" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {contentsOpen && jsonMode && (
            <pre className="max-h-[420px] overflow-auto rounded-md border border-border bg-background/60 p-4 font-mono text-xs text-muted-foreground">
              {JSON.stringify(
                items.map((i) => ({ schematic: i.schematic, qty: i.qty })),
                null,
                2,
              )}
            </pre>
          )}
        </div>
      </Panel>
    </ViewContainer>
  )
}
