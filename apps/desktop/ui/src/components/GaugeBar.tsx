import * as React from 'react'

import { clamp, formatBytes, segmentRatios, GB } from '@/hooks/useGauge'

export interface GaugeBarProps {
  potentialBytes: number
  stagedBytes: number
  freedBytes: number
  maxCapacity?: number
}

export default function GaugeBar({
  potentialBytes,
  stagedBytes,
  freedBytes,
  maxCapacity = 4 * GB,
}: GaugeBarProps) {
  const { freedRatio, stagedRatio, potentialRatio } = React.useMemo(() => {
    return segmentRatios({
      potentialBytes,
      stagedBytes,
      freedBytes,
      maxCapacity,
    })
  }, [freedBytes, maxCapacity, potentialBytes, stagedBytes])

  const freedWidth = `${clamp(freedRatio) * 100}%`
  const stagedWidth = `${clamp(Math.max(stagedRatio, freedRatio)) * 100}%`
  const stagedOffset = `${clamp(freedRatio) * 100}%`
  const potentialWidth = `${clamp(potentialRatio) * 100}%`

  const label = React.useMemo(() => {
    const target = formatBytes(maxCapacity)
    return [
      `Potential: ${formatBytes(potentialBytes)}`,
      `Staged: ${formatBytes(stagedBytes)}`,
      `Freed: ${formatBytes(freedBytes)}`,
      `Target: ${target}`
    ].join(', ')
  }, [freedBytes, maxCapacity, potentialBytes, stagedBytes])

  return (
    <div className="relative h-2.5 w-full overflow-hidden rounded-full bg-muted" aria-label={label}>
      <div
        className="absolute left-0 top-0 h-full rounded-full bg-emerald-500/80 transition-all duration-500 ease-out"
        style={{ width: freedWidth }}
      />
      <div
        className="absolute top-0 h-full rounded-full bg-amber-400/80 transition-all duration-500 ease-out"
        style={{ left: stagedOffset, width: `calc(${stagedWidth} - ${stagedOffset})` }}
      />
      <div
        className="absolute left-0 top-0 h-full rounded-full opacity-70 transition-all duration-500 ease-out"
        style={{ width: potentialWidth, backgroundImage: 'repeating-linear-gradient(90deg, rgba(59,130,246,0.35) 0, rgba(59,130,246,0.35) 6px, transparent 6px, transparent 12px)' }}
      />
    </div>
  )
}
