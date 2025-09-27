import * as React from "react"

import { formatBytes } from "@/hooks/useGauge"

export interface GaugeBarProps {
  potentialBytes: number
  stagedBytes: number
  freedBytes: number
  windowStart?: string | null
  windowEnd?: string | null
}

export default function GaugeBar({
  potentialBytes,
  stagedBytes,
  freedBytes,
  windowStart,
  windowEnd,
}: GaugeBarProps) {
  const windowLabel = React.useMemo(() => formatWindowRange(windowStart, windowEnd), [windowEnd, windowStart])

  return (
    <div className="grid gap-3 rounded-lg border border-border/60 bg-muted/20 p-3 text-sm text-foreground">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground uppercase tracking-wide">
        <span>Rolling Window</span>
        <span>{windowLabel}</span>
      </div>
      <StatRow label="Potential today" value={formatBytes(potentialBytes)} />
      <StatRow label="Staged this window" value={formatBytes(stagedBytes)} />
      <StatRow label="Freed this window" value={formatBytes(freedBytes)} />
    </div>
  )
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground">{value}</span>
    </div>
  )
}

function formatWindowRange(windowStart?: string | null, windowEnd?: string | null): string {
  if (!windowStart || !windowEnd) return "Window unavailable"
  try {
    const start = new Date(windowStart)
    const end = new Date(windowEnd)
    if (!Number.isFinite(start.getTime()) || !Number.isFinite(end.getTime())) {
      return "Window unavailable"
    }

    const sameYear = start.getFullYear() === end.getFullYear()
    const startFormatter = new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: sameYear ? undefined : "numeric",
    })
    const endFormatter = new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    })

    const startLabel = startFormatter.format(start)
    const endLabel = endFormatter.format(end)

    return `${startLabel} – ${endLabel}`
  } catch (error) {
    console.warn("Failed to format gauge window", error)
    return "Window unavailable"
  }
}
