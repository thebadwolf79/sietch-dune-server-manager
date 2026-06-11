"use client"

import { cn } from "@/lib/utils"

type SparkTone = "accent" | "success" | "warning" | "destructive" | "muted"

const stroke: Record<SparkTone, string> = {
  accent: "var(--primary)",
  success: "var(--success)",
  warning: "var(--warning)",
  destructive: "var(--destructive)",
  muted: "var(--muted-foreground)",
}

/**
 * Tiny dependency-free trend line. Renders an SVG polyline + soft area fill,
 * normalized to the data range. Portable: maps cleanly onto a Radix panel.
 */
export function Sparkline({
  data,
  tone = "accent",
  width = 96,
  height = 28,
  className,
  "aria-label": ariaLabel,
}: {
  data: number[]
  tone?: SparkTone
  width?: number
  height?: number
  className?: string
  "aria-label"?: string
}) {
  if (!data.length) return null

  const pad = 2
  const min = Math.min(...data)
  const max = Math.max(...data)
  const range = max - min || 1
  const stepX = (width - pad * 2) / Math.max(data.length - 1, 1)

  const points = data.map((v, i) => {
    const x = pad + i * stepX
    const y = pad + (height - pad * 2) * (1 - (v - min) / range)
    return [x, y] as const
  })

  const line = points.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ")
  const area = `${pad},${height - pad} ${line} ${(width - pad).toFixed(1)},${height - pad}`
  const [lastX, lastY] = points[points.length - 1]
  const color = stroke[tone]
  const gradId = `spark-${tone}`

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={cn("overflow-visible", className)}
      role="img"
      aria-label={ariaLabel}
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
  )
}
