import { Play, RotateCcw, Square, Power, PauseCircle, PlayCircle, Users } from "lucide-react";
import Sparkline from "../ui/Sparkline";

export type VmStage = "off" | "saved" | "running";
export type LifecyclePhase = "stopped" | "starting" | "healthy" | "degraded" | "stopping";
export type Verdict = "operational" | "degraded" | "down";

const verdictMeta: Record<
  Verdict,
  { label: string; dot: string; text: string; ring: string; bg: string }
> = {
  operational: {
    label: "Operational",
    dot: "bg-success",
    text: "text-success",
    ring: "rgba(118, 184, 118, 0.3)",
    bg: "rgba(118, 184, 118, 0.1)",
  },
  degraded: {
    label: "Degraded",
    dot: "bg-warning",
    text: "text-warning",
    ring: "rgba(212, 168, 94, 0.3)",
    bg: "rgba(212, 168, 94, 0.1)",
  },
  down: {
    label: "Down",
    dot: "bg-destructive",
    text: "text-destructive",
    ring: "rgba(214, 105, 94, 0.3)",
    bg: "rgba(214, 105, 94, 0.1)",
  },
};

const stages: { id: VmStage; label: string; icon: typeof Power }[] = [
  { id: "off", label: "Off", icon: Power },
  { id: "saved", label: "Saved", icon: PauseCircle },
  { id: "running", label: "Running", icon: PlayCircle },
];

const lifecycleLabel: Record<LifecyclePhase, string> = {
  stopped: "BattleGroup stopped",
  starting: "BattleGroup starting…",
  healthy: "BattleGroup healthy",
  degraded: "BattleGroup degraded",
  stopping: "BattleGroup stopping…",
};

export type SystemStatusHeaderProps = {
  verdict: Verdict;
  detail: string;
  serverName: string;
  host: string;
  uptime: string;
  activePlayers: number;
  capacity: number;
  playerTrend: number[];
  vmName: string;
  stage: VmStage;
  lifecycle: LifecyclePhase;
  busy: boolean;
  onStartBg: () => void;
  onStopBg: () => void;
  onRestartBg: () => void;
};

export default function SystemStatusHeader({
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
  busy,
  onStartBg,
  onStopBg,
  onRestartBg,
}: SystemStatusHeaderProps) {
  const v = verdictMeta[verdict];
  const stageIndex = stages.findIndex((s) => s.id === stage);
  const canStartBg = stage === "running" && lifecycle === "stopped" && !busy;
  const bgRunning = (lifecycle === "healthy" || lifecycle === "degraded") && !busy;
  const trendTone = verdict === "down" ? "destructive" : verdict === "degraded" ? "warning" : "success";

  return (
    <section
      className="bracket chamfer"
      style={{
        border: "1px solid var(--color-border-hair)",
        backgroundColor: "var(--color-bg-panel)",
        padding: "16px",
        boxShadow: `inset 0 0 0 1px ${v.ring}`,
      }}
      aria-label="System status"
    >
      {/* Top row: verdict + identity + players */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "16px",
        }}
        className="lg-row-layout"
      >
        <div style={{ display: "flex", alignItems: "center", gap: "14px", flex: 1 }}>
          <span
            className="chamfer-sm"
            style={{
              display: "flex",
              width: "44px",
              height: "44px",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "8px",
              backgroundColor: v.bg,
            }}
          >
            <span
              className="status-dot"
              data-pulse={verdict !== "operational" ? "true" : "false"}
              style={{
                width: "12px",
                height: "12px",
                borderRadius: "50%",
                backgroundColor: verdict === "operational" ? "var(--color-ok)" : verdict === "degraded" ? "var(--color-warn)" : "var(--color-err)",
              }}
            />
          </span>
          <div style={{ minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
              <h2
                style={{
                  fontFamily: "var(--font-display)",
                  fontSize: "20px",
                  margin: 0,
                  letterSpacing: "0.02em",
                  color: "var(--color-text-primary)",
                }}
              >
                {serverName}
              </h2>
              <span className={`font-display ${v.text}`} style={{ fontSize: "14px" }}>
                {v.label}
              </span>
            </div>
            <p className="truncate" style={{ fontFamily: "var(--font-mono)", fontSize: "12px", margin: "4px 0 0 0", color: "var(--color-text-muted)" }}>
              {host} {uptime ? `· up ${uptime}` : ""}
            </p>
          </div>
        </div>

        {/* Live players with trend */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "16px",
            border: "1px solid var(--color-border-hair)",
            backgroundColor: "rgba(0, 0, 0, 0.2)",
            padding: "10px 14px",
          }}
        >
          <div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "6px",
                fontFamily: "var(--font-display)",
                fontSize: "11px",
                color: "var(--color-text-muted)",
              }}
            >
              <Users style={{ width: 12, height: 12 }} /> Players
            </div>
            <p
              style={{
                marginTop: "4px",
                fontFamily: "var(--font-mono)",
                fontSize: "20px",
                margin: 0,
                lineHeight: 1,
                color: "var(--color-text-primary)",
              }}
            >
              {activePlayers}
              <span style={{ fontSize: "14px", color: "var(--color-text-muted)" }}>
                /{capacity}
              </span>
            </p>
          </div>
          <Sparkline
            data={playerTrend}
            tone={trendTone}
            width={104}
            height={32}
            aria-label="Player count trend"
          />
        </div>
      </div>

      {/* Detail line */}
      <p
        style={{
          marginTop: "12px",
          borderTop: "1px solid var(--color-border-hair)",
          paddingTop: "12px",
          fontSize: "14px",
          margin: "12px 0 0 0",
          color: "var(--color-text-muted)",
        }}
      >
        {detail}
      </p>

      {/* Control deck: VM power rail + lifecycle actions */}
      <div
        style={{
          marginTop: "16px",
          display: "flex",
          flexDirection: "column",
          gap: "16px",
        }}
        className="xl-row-layout"
      >
        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "12px" }}>
          <span style={{ fontFamily: "var(--font-display)", fontSize: "11px", color: "var(--color-text-muted)" }}>
            {vmName}
          </span>
          <div
            className="chamfer-sm"
            style={{
              display: "flex",
              alignItems: "stretch",
              overflow: "hidden",
              border: "1px solid var(--color-border-hair)",
            }}
            role="group"
            aria-label="VM power state"
          >
            {stages.map((s, i) => {
              const Icon = s.icon;
              const active = s.id === stage;
              const reached = i <= stageIndex;
              return (
                <button
                  key={s.id}
                  type="button"
                  disabled
                  aria-pressed={active}
                  className="action-btn"
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    border: "0",
                    borderRight: i < stages.length - 1 ? "1px solid var(--color-border-hair)" : "0",
                    borderRadius: "0",
                    padding: "6px 12px",
                    fontSize: "12px",
                    fontFamily: "var(--font-sans)",
                    textTransform: "uppercase",
                    letterSpacing: "0.04em",
                    cursor: "default",
                    backgroundColor: active
                      ? "rgba(217, 119, 87, 0.15)"
                      : reached
                      ? "var(--color-bg-elevated)"
                      : "transparent",
                    color: active
                      ? "var(--color-accent-strong)"
                      : reached
                      ? "var(--color-text-primary)"
                      : "var(--color-text-muted)",
                  }}
                >
                  <Icon style={{ width: 14, height: 14 }} />
                  {s.label}
                </button>
              );
            })}
          </div>
          <span style={{ display: "flex", alignItems: "center", gap: "8px", fontSize: "12px" }}>
            <span
              className={`status-dot ${
                lifecycle === "healthy"
                  ? "bg-success"
                  : lifecycle === "degraded"
                  ? "bg-warning"
                  : lifecycle === "stopped"
                  ? ""
                  : "bg-warning"
              }`}
              data-pulse={lifecycle === "starting" || lifecycle === "stopping" ? "true" : "false"}
              style={{
                width: "8px",
                height: "8px",
                borderRadius: "50%",
                backgroundColor: lifecycle === "stopped" ? "var(--color-text-muted)" : undefined,
              }}
              aria-hidden="true"
            />
            <span style={{ fontFamily: "var(--font-mono)", color: "var(--color-text-muted)" }}>
              {lifecycleLabel[lifecycle]}
            </span>
          </span>
        </div>

        {/* Lifecycle actions — destructive isolated to the right */}
        <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px" }}>
          <button
            type="button"
            className="action-btn"
            disabled={!canStartBg}
            onClick={onStartBg}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              fontSize: "12.5px",
              borderColor: canStartBg ? "var(--color-accent)" : undefined,
              color: canStartBg ? "var(--color-accent-strong)" : undefined,
            }}
          >
            <Play style={{ width: 14, height: 14 }} /> Start
          </button>
          <button
            type="button"
            className="action-btn"
            disabled={!bgRunning}
            onClick={onRestartBg}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              fontSize: "12.5px",
            }}
          >
            <RotateCcw style={{ width: 14, height: 14 }} /> Restart
          </button>
          <span
            style={{
              display: "inline-block",
              width: "1px",
              height: "20px",
              backgroundColor: "var(--color-border-hair)",
              margin: "0 4px",
            }}
            className="hidden-sm-divider"
          />
          <button
            type="button"
            className="action-btn"
            disabled={!bgRunning}
            onClick={onStopBg}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 12px",
              fontSize: "12.5px",
              borderColor: bgRunning ? "rgba(214, 105, 94, 0.4)" : undefined,
              color: bgRunning ? "var(--color-err)" : undefined,
            }}
          >
            <Square style={{ width: 14, height: 14 }} /> Stop
          </button>
        </div>
      </div>
    </section>
  );
}
