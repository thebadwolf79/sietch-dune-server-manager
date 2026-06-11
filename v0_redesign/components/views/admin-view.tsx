"use client"

import { useMemo, useState } from "react"
import { AlertTriangle, Send, Users2, Search, Terminal, Check } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { StatusPill } from "@/components/status-pill"
import { Panel, SectionLabel, ViewContainer } from "@/components/primitives"
import { commandCatalog, commandPlayers, publishes, type CommandSpec, type FieldSpec } from "@/lib/dune-data"
import { cn } from "@/lib/utils"

type FormValues = Record<string, string>

function defaultsFor(cmd: CommandSpec): FormValues {
  const v: FormValues = {}
  for (const f of cmd.fields) {
    if (f.kind === "bool") v[f.key] = "false"
    else if (f.kind === "select") v[f.key] = f.options?.[0]?.value ?? ""
    else v[f.key] = ""
  }
  return v
}

function Field({
  spec,
  value,
  onChange,
}: {
  spec: FieldSpec
  value: string
  onChange: (v: string) => void
}) {
  const id = `field-${spec.key}`
  return (
    <div>
      <label htmlFor={id} className="mb-1 block text-xs font-medium text-foreground">
        {spec.label}
        {spec.required && <span className="ml-1 text-destructive">*</span>}
      </label>
      {spec.kind === "text" ? (
        <Textarea
          id={id}
          rows={3}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="font-mono text-sm"
          placeholder={spec.helper}
        />
      ) : spec.kind === "bool" ? (
        <select
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring"
        >
          <option value="false">false</option>
          <option value="true">true</option>
        </select>
      ) : spec.kind === "select" ? (
        <select
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring"
        >
          {spec.options?.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      ) : (
        <Input
          id={id}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          inputMode={spec.kind === "int" || spec.kind === "float" ? "decimal" : "text"}
          className="font-mono text-sm"
          placeholder={spec.helper}
        />
      )}
      {spec.helper && (spec.kind === "text" || spec.kind === "bool" || spec.kind === "select") && (
        <p className="mt-1 text-[11px] text-muted-foreground">{spec.helper}</p>
      )}
    </div>
  )
}

/** Live, read-only assembly of the command as the operator fills the form. */
function CommandPreview({
  cmd,
  target,
  values,
}: {
  cmd: CommandSpec
  target: string
  values: FormValues
}) {
  const args = cmd.fields
    .filter((f) => values[f.key] !== "" && values[f.key] !== undefined)
    .map((f) => `--${f.key}=${JSON.stringify(values[f.key])}`)
  const targetArg = cmd.needsPlayer ? `--target=${target}` : null
  const parts = [cmd.id, targetArg, ...args].filter(Boolean) as string[]

  return (
    <div className="rounded-md border border-border bg-background/60 p-3">
      <div className="mb-1.5 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">
        <Terminal className="size-3" /> Command preview
      </div>
      <code className="block whitespace-pre-wrap break-all font-mono text-xs leading-relaxed text-foreground">
        <span className="text-primary">publish</span>{" "}
        {parts.map((p, i) => (
          <span key={i} className={i === 0 ? "text-foreground" : "text-muted-foreground"}>
            {p}{" "}
          </span>
        ))}
      </code>
    </div>
  )
}

function StepBadge({ n, label, done }: { n: number; label: string; done: boolean }) {
  return (
    <div className="flex items-center gap-2">
      <span
        className={cn(
          "flex size-5 items-center justify-center rounded-full border text-[11px] font-semibold",
          done ? "border-primary bg-primary/15 text-primary" : "border-border text-muted-foreground",
        )}
      >
        {done ? <Check className="size-3" /> : n}
      </span>
      <span className={cn("text-xs font-medium uppercase tracking-wide", done ? "text-foreground" : "text-muted-foreground")}>
        {label}
      </span>
    </div>
  )
}

function CommandForm({ cmd }: { cmd: CommandSpec }) {
  const [allPlayers, setAllPlayers] = useState(false)
  const [target, setTarget] = useState(commandPlayers[0]?.flsId ?? "")
  const [values, setValues] = useState<FormValues>(() => defaultsFor(cmd))
  const [confirm, setConfirm] = useState(false)

  const targetLabel = allPlayers ? "ALL_PLAYERS" : target
  const requiredFilled = cmd.fields
    .filter((f) => f.required)
    .every((f) => (values[f.key] ?? "") !== "")
  const targetReady = !cmd.needsPlayer || allPlayers || !!target
  const canPublish = requiredFilled && targetReady && (!cmd.destructive || confirm)

  const set = (k: string, v: string) => setValues((prev) => ({ ...prev, [k]: v }))

  return (
    <Panel className="overflow-hidden">
      {/* Header */}
      <div className="border-b border-border p-4">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-sm font-semibold text-foreground">{cmd.label}</h3>
          <StatusPill tone="muted" dot={false}>
            {cmd.group}
          </StatusPill>
          {cmd.destructive && (
            <StatusPill tone="destructive" dot={false}>
              destructive
            </StatusPill>
          )}
        </div>
        <p className="mt-1.5 text-sm text-muted-foreground">{cmd.describe}</p>

        {/* Step rail */}
        <div className="mt-3 flex flex-wrap items-center gap-x-5 gap-y-2">
          <StepBadge n={1} label="Command" done />
          {cmd.needsPlayer && <StepBadge n={2} label="Target" done={targetReady} />}
          <StepBadge n={cmd.needsPlayer ? 3 : 2} label="Parameters" done={requiredFilled} />
        </div>
      </div>

      <div className="space-y-4 p-4">
        {/* Player picker */}
        {cmd.needsPlayer && (
          <div>
            <div className="mb-1 flex items-center justify-between">
              <SectionLabel>Target player</SectionLabel>
              {cmd.allowAllPlayers && (
                <button
                  type="button"
                  onClick={() => setAllPlayers((v) => !v)}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium uppercase tracking-wide transition-colors",
                    allPlayers
                      ? "border-primary/40 bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:text-foreground",
                  )}
                >
                  <Users2 className="size-3.5" /> All players
                </button>
              )}
            </div>
            <select
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              disabled={allPlayers}
              className="h-9 w-full rounded-md border border-input bg-background px-2 text-sm text-foreground outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
            >
              {commandPlayers.map((p) => (
                <option key={p.flsId} value={p.flsId}>
                  {p.name} · {p.flsId} {p.online ? "(online)" : "(offline)"}
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Dynamic fields */}
        {cmd.fields.length > 0 && (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {cmd.fields.map((f) => (
              <div key={f.key} className={f.kind === "text" ? "sm:col-span-2" : undefined}>
                <Field spec={f} value={values[f.key] ?? ""} onChange={(v) => set(f.key, v)} />
              </div>
            ))}
          </div>
        )}

        {/* Live preview */}
        <CommandPreview cmd={cmd} target={targetLabel} values={values} />

        {/* Destructive confirm + publish */}
        <div className="flex flex-col gap-3 border-t border-border pt-4">
          {cmd.destructive && (
            <label className="flex items-start gap-2 text-sm text-warning">
              <input
                type="checkbox"
                checked={confirm}
                onChange={(e) => setConfirm(e.target.checked)}
                className="mt-0.5 size-4 rounded border-border accent-[var(--destructive)]"
              />
              <span className="flex items-center gap-1.5">
                <AlertTriangle className="size-3.5" />
                I understand this action is irreversible.
              </span>
            </label>
          )}
          <div className="flex items-center justify-between gap-3">
            <span className="text-[11px] text-muted-foreground">
              {canPublish ? "Ready to publish." : "Complete required fields to publish."}
            </span>
            <Button
              size="sm"
              className={cn("gap-1.5", cmd.destructive && "bg-destructive text-white hover:bg-destructive/90")}
              disabled={!canPublish}
            >
              <Send className="size-3.5" /> Publish command
            </Button>
          </div>
        </div>
      </div>
    </Panel>
  )
}

export function AdminView() {
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [query, setQuery] = useState("")

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return commandCatalog
    return commandCatalog
      .map((g) => ({
        ...g,
        items: g.items.filter(
          (i) => i.label.toLowerCase().includes(q) || i.group.toLowerCase().includes(q),
        ),
      }))
      .filter((g) => g.items.length > 0)
  }, [query])

  const selected = useMemo(() => {
    for (const group of commandCatalog) {
      const found = group.items.find((i) => i.id === selectedId)
      if (found) return found
    }
    return null
  }, [selectedId])

  return (
    <ViewContainer>
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[280px_1fr]">
        {/* Command catalog */}
        <Panel className="h-fit overflow-hidden">
          <div className="border-b border-border p-3">
            <h3 className="mb-2 text-sm font-semibold text-foreground">Command catalog</h3>
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search commands…"
                className="h-8 pl-8 text-sm"
              />
            </div>
          </div>
          <div className="max-h-[70vh] space-y-4 overflow-y-auto p-3">
            {filtered.map((group) => (
              <div key={group.group}>
                <p className="px-1 pb-1.5 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">
                  {group.group}
                </p>
                <ul className="space-y-1">
                  {group.items.map((item) => (
                    <li key={item.id}>
                      <button
                        type="button"
                        onClick={() => setSelectedId(item.id)}
                        className={cn(
                          "flex w-full items-center justify-between gap-2 rounded-md border px-2.5 py-1.5 text-left text-sm transition-colors",
                          selectedId === item.id
                            ? "border-primary/40 bg-primary/10 text-primary"
                            : item.destructive
                              ? "border-destructive/30 text-destructive hover:bg-destructive/10"
                              : "border-border text-foreground hover:bg-accent",
                        )}
                      >
                        <span className="truncate">{item.label}</span>
                        {item.destructive && <AlertTriangle className="size-3.5 shrink-0 opacity-70" />}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
            {filtered.length === 0 && (
              <p className="px-1 py-2 text-sm text-muted-foreground">No commands match.</p>
            )}
          </div>
        </Panel>

        {/* Form + recent publishes */}
        <div className="space-y-5">
          {selected ? (
            <CommandForm key={selected.id} cmd={selected} />
          ) : (
            <Panel className="flex min-h-40 flex-col items-center justify-center gap-2 p-8 text-center">
              <Terminal className="size-6 text-muted-foreground" />
              <p className="text-sm text-muted-foreground">
                Select a command to configure and publish it. The command assembles live as you fill the form.
              </p>
            </Panel>
          )}

          <div>
            <h3 className="mb-2 text-sm font-semibold text-foreground">Recent publishes</h3>
            <Panel className="overflow-hidden">
              <div className="grid grid-cols-12 gap-2 border-b border-border px-4 py-2.5">
                <span className="col-span-7 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">Cmd</span>
                <span className="col-span-2 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">OK</span>
                <span className="col-span-3 text-right text-[11px] font-medium uppercase tracking-widest text-muted-foreground">When</span>
              </div>
              <div className="max-h-[420px] overflow-y-auto">
                {publishes.map((p, i) => (
                  <div
                    key={i}
                    className="grid grid-cols-12 items-center gap-2 border-b border-border px-4 py-2.5 transition-colors last:border-0 hover:bg-accent/40"
                  >
                    <span className="col-span-7 truncate font-mono text-sm text-foreground">{p.cmd}</span>
                    <span className="col-span-2">
                      <StatusPill tone={p.ok ? "success" : "destructive"} dot={false}>
                        {p.ok ? "ok" : "fail"}
                      </StatusPill>
                    </span>
                    <span className="col-span-3 text-right font-mono text-xs text-muted-foreground">{p.when}</span>
                  </div>
                ))}
              </div>
            </Panel>
          </div>
        </div>
      </div>
    </ViewContainer>
  )
}
