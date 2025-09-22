export const GB = 1024 ** 3

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const

export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '0 B'
  const exponent = Math.min(
    Math.floor(Math.log(value) / Math.log(1024)),
    UNITS.length - 1
  )
  const num = value / 1024 ** exponent
  const formatter = num >= 100 ? Math.round(num) : Math.round(num * 10) / 10
  return `${formatter} ${UNITS[exponent]}`
}

export function clamp(value: number, min = 0, max = 1) {
  return Math.min(Math.max(value, min), max)
}

export function thresholds(stagedBytes: number) {
  return {
    soft: stagedBytes >= 3 * GB,
    hard: stagedBytes >= 4 * GB,
  }
}

export function segmentRatios({
  potentialBytes,
  stagedBytes,
  freedBytes,
  maxCapacity = 4 * GB,
}: {
  potentialBytes: number
  stagedBytes: number
  freedBytes: number
  maxCapacity?: number
}) {
  const capacity = Math.max(maxCapacity, 1)
  const freedRatio = clamp(freedBytes / capacity)
  const stagedRatio = clamp((freedBytes + stagedBytes) / capacity)
  const potentialRatio = clamp((freedBytes + stagedBytes + potentialBytes) / capacity)

  return {
    freedRatio,
    stagedRatio,
    potentialRatio,
  }
}
