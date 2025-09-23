import * as React from 'react'

import { Button } from '@/components/ui/button'
import { Loader2, PauseCircle } from 'lucide-react'
import GaugeBar from '@/components/GaugeBar'
import { notifySweepReady } from '@/lib/notify'
import { formatBytes, thresholds, GB } from '@/hooks/useGauge'

export interface BottomBarProps {
  scanning: boolean
  scanSummary?: string
  currentPathDisplay?: string
  finishedAtISO?: string
  lastScanSummary?: string
  errorMessages?: string[]
  folderError?: string | null
  hasEmptyState?: boolean
  isLoadingFolders?: boolean
  potentialBytes: number
  stagedBytes: number
  freedBytes: number
  maxCapacity?: number
  onOpenReview: () => void
}

export default function BottomBar({
  scanning,
  scanSummary,
  currentPathDisplay,
  finishedAtISO,
  lastScanSummary,
  errorMessages = [],
  folderError,
  hasEmptyState,
  isLoadingFolders,
  potentialBytes,
  stagedBytes,
  freedBytes,
  maxCapacity = 4 * GB,
  onOpenReview,
}: BottomBarProps) {
  const { soft, hard } = thresholds(stagedBytes)
  const prevBuckets = React.useRef({ soft: false, hard: false })

  React.useEffect(() => {
    const next = { soft, hard }
    if (next.hard && !prevBuckets.current.hard) {
      void notifySweepReady(stagedBytes)
      onOpenReview()
    } else if (next.soft && !prevBuckets.current.soft) {
      void notifySweepReady(stagedBytes)
    }
    prevBuckets.current = next
  }, [hard, onOpenReview, soft, stagedBytes])

  const statusContent = React.useMemo(() => {
    if (scanning) {
      return (
        <div className="flex min-w-0 items-center gap-2 text-xs text-primary" role="status" aria-live="polite">
          <Loader2 className="h-4 w-4 animate-spin" />
          <div className="min-w-0">
            <span className="font-medium">Scanning</span>
            {scanSummary ? <span className="ml-2 text-primary/70">{scanSummary}</span> : null}
            {/* {currentPathDisplay ? (
              <p className="truncate text-[11px] text-primary/60" title={currentPathDisplay}>
                {currentPathDisplay.slice(0, 30) + "..."}
              </p>
            ) : null} */}
          </div>
        </div>
      )
    }

    if (finishedAtISO) {
      const date = new Date(finishedAtISO)
      return (
        <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground" role="status" aria-live="polite">
          <PauseCircle className="h-4 w-4" />
          <span className="font-medium text-foreground">Last scan</span>
          <time dateTime={finishedAtISO}>{date.toLocaleString()}</time>
          {lastScanSummary ? <span className="hidden sm:inline text-muted-foreground/80">{lastScanSummary}</span> : null}
        </div>
      )
    }

    return null
  }, [currentPathDisplay, finishedAtISO, lastScanSummary, scanSummary, scanning])

  const hasErrors = errorMessages.length > 0

  return (
    <div className="sticky bottom-0 z-40 border-t border-border/60 bg-background/80 mt-15 pt-4 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
        <div className="flex flex-1 items-center gap-2 text-xs">
          {statusContent}
          {hasErrors ? (
            <div className="flex min-w-0 flex-col gap-1 text-destructive" aria-live="polite">
              {errorMessages.slice(-3).map((message, index) => (
                <span key={`error-${index}`} className="truncate">
                  {message}
                </span>
              ))}
            </div>
          ) : null}
          {folderError ? (
            <span className="truncate text-xs text-destructive" aria-live="polite">{folderError}</span>
          ) : null}
          {isLoadingFolders ? (
            <div className="h-3 w-24 animate-pulse rounded-full bg-muted" aria-hidden />
          ) : null}
          {hasEmptyState ? (
            <span className="text-muted-foreground">No watched folders yet. Use Add folder.</span>
          ) : null}
        </div>

        <div className="flex w-full flex-col gap-1 md:mx-4 md:w-auto md:flex-1">
          <GaugeBar
            potentialBytes={potentialBytes}
            stagedBytes={stagedBytes}
            freedBytes={freedBytes}
            maxCapacity={maxCapacity}
          />
          <div className="flex justify-between text-[9px] uppercase tracking-wide text-muted-foreground">
            <span>Potential</span>
            <span>Staged</span>
            <span>Freed</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {hard ? (
            <Button variant="destructive" size="sm" onClick={onOpenReview}>
              Review & delete
            </Button>
          ) : soft ? (
            <span className="rounded-full border border-primary/40 px-2.5 py-1 text-xs text-primary">
              Weekly sweep ready
            </span>
          ) : (
            <span className="text-[11px] text-muted-foreground">
              {formatBytes(stagedBytes)} staged this week
            </span>
          )}
        </div>
      </div>
    </div>
  )
}
