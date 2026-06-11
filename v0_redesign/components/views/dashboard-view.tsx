"use client"

import { ExternalLink, Plus } from "lucide-react"
import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/status-pill"
import { Panel, SectionLabel, ViewContainer } from "@/components/primitives"
import { SystemStatusHeader } from "@/components/system-status-header"
import { MetricTile, metricIcons } from "@/components/metric-tile"
import { HostHealthPanel } from "@/components/host-health-panel"
import {
  server,
  maps,
  tunnels,
  hostHealth,
  systemStatus,
  playerTrend,
  fpsTrend,
} from "@/lib/dune-data"
import { cn } from "@/lib/utils"

type Tone = "muted" | "healthy" | "warning" | "danger"

function phaseTone(phase: string): Tone {
  const p = phase.toLowerCase()
  if (["ready", "running", "healthy"].includes(p)) return "healthy"
  if (["pending", "starting", "degraded"].includes(p)) return "warning"
  if (["failed", "error", "stopped", "crashloopbackoff"].includes(p)) return "danger"
  return "muted"
}

const pillTone = (t: Tone) =>
  t === "healthy" ? "success" : t === "danger" ? "destructive" : t === "warning" ? "warning" : "muted"

function ServerStatsTable() {
  return (
    <Panel className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
        <SectionLabel>Server stats — per map</SectionLabel>
        <span className="font-mono text-[11px] text-muted-foreground">{maps.length} maps</span>
      </div>
      <div className="grid grid-cols-12 gap-2 border-b border-border px-4 py-2">
        <span className="col-span-5 font-display text-[11px] text-muted-foreground">Map</span>
        <span className="col-span-3 font-display text-[11px] text-muted-foreground">Phase</span>
        <span className="col-span-2 text-right font-display text-[11px] text-muted-foreground">Players</span>
        <span className="col-span-1 text-right font-display text-[11px] text-muted-foreground">FPS</span>
        <span className="col-span-1 text-right font-display text-[11px] text-muted-foreground">Age</span>
      </div>
      {maps.map((m) => {
        const tone = phaseTone(m.phase)
        return (
          <div
            key={m.name}
            className="grid grid-cols-12 items-center gap-2 border-b border-border px-4 py-3 transition-colors last:border-0 hover:bg-accent/40"
          >
            <span className="col-span-5 truncate font-mono text-sm text-foreground">{m.name}</span>
            <span className="col-span-3">
              <StatusPill tone={pillTone(tone)}>{m.phase}</StatusPill>
            </span>
            <span className="col-span-2 text-right font-mono text-sm text-foreground">
              {m.players}
              <span className="text-muted-foreground">/{m.maxPlayers}</span>
            </span>
            <span
              className={cn(
                "col-span-1 text-right font-mono text-sm",
                m.fps === 0 ? "text-muted-foreground" : m.fps < 25 ? "text-warning" : "text-foreground",
              )}
            >
              {m.fps || "—"}
            </span>
            <span className="col-span-1 text-right font-mono text-xs text-muted-foreground">{m.age}</span>
          </div>
        )
      })}
    </Panel>
  )
}

function ManagementServiceCard() {
  return (
    <Panel className="p-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-3">
          <h3 className="font-display text-sm text-foreground">Management service</h3>
          <StatusPill tone="success">active {server.managementVersion}</StatusPill>
          <StatusPill tone="muted" dot={false}>
            {server.managementInit}
          </StatusPill>
          <span className="text-xs text-muted-foreground">Up to date</span>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm">
            Refresh
          </Button>
          <Button variant="outline" size="sm" className="text-success hover:text-success">
            Restart
          </Button>
          <Button variant="outline" size="sm" className="text-destructive hover:text-destructive">
            Uninstall
          </Button>
        </div>
      </div>
    </Panel>
  )
}

function TunnelsPanel() {
  return (
    <Panel className="overflow-hidden">
      <div className="border-b border-border px-4 py-2.5">
        <SectionLabel>Tunnels</SectionLabel>
      </div>
      {tunnels.map((t) => (
        <div
          key={t.name}
          className="flex items-center justify-between border-b border-border px-4 py-2.5 last:border-0"
        >
          <div className="flex items-center gap-2.5">
            <span className="size-2 shrink-0 rounded-full bg-muted-foreground" aria-hidden="true" />
            <div>
              <p className="text-sm font-medium text-foreground">{t.name}</p>
              <p className="text-xs text-muted-foreground">{t.status}</p>
            </div>
          </div>
          <Button variant="outline" size="sm" className="gap-1.5">
            <ExternalLink className="size-3.5" /> Start
          </Button>
        </div>
      ))}
      <button
        type="button"
        className="flex w-full items-center gap-1.5 px-4 py-2.5 text-sm text-muted-foreground transition-colors hover:bg-accent/40 hover:text-foreground"
      >
        <Plus className="size-4" /> Add custom tunnel
      </button>
    </Panel>
  )
}

export function DashboardView() {
  const dbTone = phaseTone(server.database)
  const gwTone = phaseTone(server.gateway)
  const dirTone = phaseTone(server.director)

  return (
    <ViewContainer>
      <div className="space-y-5">
        {/* 1 — Pinned command deck: verdict + power + lifecycle */}
        <SystemStatusHeader
          verdict={systemStatus.verdict}
          detail={systemStatus.detail}
          serverName={server.name}
          host={server.host}
          uptime={server.uptime}
          activePlayers={systemStatus.activePlayers}
          capacity={systemStatus.capacity}
          playerTrend={playerTrend}
          vmName={server.vm}
          stage={server.vmStage}
          lifecycle={server.lifecycle}
        />

        {/* 2 — Bento metric grid */}
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
          <MetricTile label="Namespace" value={server.namespace} icon={metricIcons.Layers} span />
          <MetricTile label="Database" value={server.database} tone={dbTone} icon={metricIcons.Database} mono={false} />
          <MetricTile label="Gateway" value={server.gateway} tone={gwTone} icon={metricIcons.Network} mono={false} />
          <MetricTile label="Director" value={server.director} tone={dirTone} icon={metricIcons.Compass} mono={false} />
          <MetricTile
            label="Server FPS"
            value="30"
            tone="healthy"
            trend={fpsTrend}
            trendTone="success"
          />
          <MetricTile label="Uptime" value={server.uptime} icon={metricIcons.Clock} />
        </div>

        {/* 3 — Dense data: stats table + host health side by side */}
        <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
          <div className="space-y-5">
            <ServerStatsTable />
            <ManagementServiceCard />
          </div>
          <HostHealthPanel report={hostHealth} />
        </div>

        {/* 4 — Tunnels */}
        <TunnelsPanel />
      </div>
    </ViewContainer>
  )
}
