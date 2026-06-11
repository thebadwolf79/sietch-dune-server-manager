import type { ComponentType } from "react";
import Sparkline from "./Sparkline";

export type MetricTone = "muted" | "healthy" | "warning" | "danger";

type ToneMeta = {
  valueClass: string;
  dotClass: string;
  iconBgClass: string;
  iconClass: string;
};

const toneMetaMap: Record<MetricTone, ToneMeta> = {
  muted: {
    valueClass: "",
    dotClass: "bg-muted-dim",
    iconBgClass: "bg-muted-dim",
    iconClass: "text-muted-foreground",
  },
  healthy: {
    valueClass: "text-success",
    dotClass: "bg-success",
    iconBgClass: "bg-success-dim",
    iconClass: "text-success",
  },
  warning: {
    valueClass: "text-warning",
    dotClass: "bg-warning",
    iconBgClass: "bg-warning-dim",
    iconClass: "text-warning",
  },
  danger: {
    valueClass: "text-destructive",
    dotClass: "bg-destructive",
    iconBgClass: "bg-destructive-dim",
    iconClass: "text-destructive",
  },
};

export type MetricTileProps = {
  label: string;
  value: string;
  tone?: MetricTone;
  icon?: ComponentType<any>;
  mono?: boolean;
  trend?: number[];
  trendTone?: "accent" | "success" | "warning" | "destructive" | "muted";
  span?: boolean;
};

export default function MetricTile({
  label,
  value,
  tone = "muted",
  icon: Icon,
  mono = true,
  trend,
  trendTone = "accent",
  span = false,
}: MetricTileProps) {
  const meta = toneMetaMap[tone];

  return (
    <div
      className={`metric-tile chamfer-sm ${span ? "sm:col-span-2" : ""}`}
      style={span ? { gridColumn: "span 2" } : undefined}
    >
      <div className="metric-tile-header">
        <span className="metric-tile-label">
          {Icon && (
            <span className={`metric-tile-icon-bg ${meta.iconBgClass}`}>
              <Icon className={`icon ${meta.iconClass}`} style={{ width: 12, height: 12 }} />
            </span>
          )}
          {label}
        </span>
        {!Icon && tone !== "muted" && (
          <span
            className={`status-dot ${meta.dotClass}`}
            style={{ width: 6, height: 6, borderRadius: "50%" }}
            aria-hidden="true"
          />
        )}
      </div>
      <div className="metric-tile-body">
        <p
          className={`metric-tile-value ${meta.valueClass}`}
          style={{
            fontFamily: mono ? "var(--font-mono)" : "inherit",
            margin: 0,
            lineHeight: 1.2,
          }}
        >
          {value}
        </p>
        {trend && (
          <Sparkline
            data={trend}
            tone={trendTone}
            width={64}
            height={22}
            aria-label={`${label} trend`}
          />
        )}
      </div>
    </div>
  );
}
