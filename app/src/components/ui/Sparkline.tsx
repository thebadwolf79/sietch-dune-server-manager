type SparkTone = "accent" | "success" | "warning" | "destructive" | "muted";

const strokeColorMap: Record<SparkTone, string> = {
  accent: "var(--color-accent)",
  success: "var(--color-ok)",
  warning: "var(--color-warn)",
  destructive: "var(--color-err)",
  muted: "var(--color-text-muted)",
};

export type SparklineProps = {
  data: number[];
  tone?: SparkTone;
  width?: number;
  height?: number;
  className?: string;
  "aria-label"?: string;
};

/**
 * Tiny dependency-free trend line. Renders an SVG polyline + soft area fill,
 * normalized to the data range.
 */
export default function Sparkline({
  data,
  tone = "accent",
  width = 96,
  height = 28,
  className,
  "aria-label": ariaLabel,
}: SparklineProps) {
  if (!data || !data.length) return null;

  const pad = 2;
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  const stepX = (width - pad * 2) / Math.max(data.length - 1, 1);

  const points = data.map((v, i) => {
    const x = pad + i * stepX;
    const y = pad + (height - pad * 2) * (1 - (v - min) / range);
    return [x, y] as const;
  });

  const line = points.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = `${pad},${height - pad} ${line} ${(width - pad).toFixed(1)},${height - pad}`;
  const [lastX, lastY] = points[points.length - 1];
  const color = strokeColorMap[tone];
  const gradId = `spark-${tone}-${Math.random().toString(36).substr(2, 9)}`;

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className}
      role="img"
      aria-label={ariaLabel}
      style={{ overflow: "visible" }}
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.22" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon points={area} fill={`url(#${gradId})`} />
      <polyline
        points={line}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx={lastX} cy={lastY} r={2} fill={color} />
    </svg>
  );
}
