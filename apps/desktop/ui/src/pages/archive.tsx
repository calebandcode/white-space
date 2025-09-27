import * as React from "react"

import { Button } from "@/components/ui/button"
import { formatBytes } from "@/hooks/useGauge"
import { useFolderStore, type StageBucket } from "@/store/folder-store"

function formatDateLabel(value?: string | null) {
  if (!value) return "Unknown date"
  try {
    const date = new Date(value)
    if (!Number.isFinite(date.getTime())) return "Unknown date"
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(date)
  } catch {
    return "Unknown date"
  }
}

export function Archive() {
  const staged = useFolderStore((state) => state.staged)
  const loadStaged = useFolderStore((state) => state.loadStaged)
  const restoreStageBatch = useFolderStore((state) => state.restoreStageBatch)
  const emptyStageFiles = useFolderStore((state) => state.emptyStageFiles)
  const getStagedBuckets = useFolderStore((state) => state.getStagedBuckets)

  React.useEffect(() => {
    void loadStaged()
  }, [loadStaged])

  const buckets = React.useMemo(() => getStagedBuckets(), [getStagedBuckets, staged.items])

  const handleRestore = React.useCallback(
    async (batchId: string | null) => {
      if (!batchId) return
      await restoreStageBatch(batchId)
    },
    [restoreStageBatch]
  )

  const handleEmpty = React.useCallback(
    async (fileIds: number[]) => {
      if (!fileIds.length) return
      await emptyStageFiles(fileIds, true)
    },
    [emptyStageFiles]
  )

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-6 px-4 py-6">
      <header className="space-y-1">
        <h1 className="text-xl font-semibold text-foreground">Archive</h1>
        <p className="text-sm text-muted-foreground">Review staged files before they are emptied for good.</p>
      </header>

      {staged.loading ? (
        <div className="rounded-lg border border-border/60 bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
          Loading staged files…
        </div>
      ) : null}

      {!staged.loading && staged.error ? (
        <div className="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-6 text-sm text-destructive">
          {staged.error}
        </div>
      ) : null}

      {!staged.loading && !staged.error && !staged.items.length ? (
        <div className="rounded-lg border border-border/40 bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
          Nothing is staged right now. Stage items from Activity to populate the archive.
        </div>
      ) : null}

      {buckets.cooling.length ? (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-foreground">Cooling off</h2>
          {buckets.cooling.map((bucket) => (
            <ArchiveBucketCard
              key={bucket.key}
              title={`${bucket.records.length} item${bucket.records.length === 1 ? "" : "s"}`}
              subtitle={`Ready ${formatDateLabel(bucket.readyAt)}`}
              bucket={bucket}
              primaryActionLabel={bucket.batchId ? "Restore" : undefined}
              onPrimaryAction={bucket.batchId ? () => handleRestore(bucket.batchId) : undefined}
            />
          ))}
        </section>
      ) : null}

      {buckets.ready.length ? (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-foreground">Ready to empty</h2>
          {buckets.ready.map((bucket) => (
            <ArchiveBucketCard
              key={bucket.key}
              title={`${bucket.records.length} item${bucket.records.length === 1 ? "" : "s"}`}
              subtitle={`Staged ${formatDateLabel(bucket.stagedAt)}`}
              bucket={bucket}
              primaryActionLabel={bucket.batchId ? "Restore" : undefined}
              onPrimaryAction={bucket.batchId ? () => handleRestore(bucket.batchId) : undefined}
              secondaryActionLabel={bucket.fileIds.length ? "Empty" : undefined}
              onSecondaryAction={bucket.fileIds.length ? () => handleEmpty(bucket.fileIds) : undefined}
            />
          ))}
        </section>
      ) : null}
    </div>
  )
}

export default Archive

function ArchiveBucketCard({
  bucket,
  title,
  subtitle,
  primaryActionLabel,
  secondaryActionLabel,
  onPrimaryAction,
  onSecondaryAction,
}: {
  bucket: StageBucket
  title: string
  subtitle: string
  primaryActionLabel?: string
  secondaryActionLabel?: string
  onPrimaryAction?: () => void
  onSecondaryAction?: () => void
}) {
  const totalBytes = bucket.records.reduce((sum, record) => sum + (record.sizeBytes ?? 0), 0)

  return (
    <div className="rounded-lg border border-border/60 bg-background px-4 py-3 text-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="font-semibold text-foreground">{title}</h3>
          <p className="text-xs text-muted-foreground">{subtitle}</p>
          <p className="text-xs text-muted-foreground">{formatBytes(totalBytes)} total</p>
        </div>
        <div className="flex gap-2">
          {secondaryActionLabel && onSecondaryAction ? (
            <Button variant="destructive" size="sm" onClick={onSecondaryAction}>
              {secondaryActionLabel}
            </Button>
          ) : null}
          {primaryActionLabel && onPrimaryAction ? (
            <Button variant="outline" size="sm" onClick={onPrimaryAction}>
              {primaryActionLabel}
            </Button>
          ) : null}
        </div>
      </div>

      <ul className="mt-3 space-y-2">
        {bucket.records.map((record) => (
          <li key={record.recordId} className="flex flex-wrap items-center gap-3 text-xs">
            <div className="min-w-0 flex-1">
              <p className="truncate text-foreground" title={record.path}>
                {record.path.split(/[\\/]/).pop() ?? record.path}
              </p>
              <p className="truncate text-muted-foreground" title={record.parentDir}>
                {record.parentDir}
              </p>
            </div>
            <span className="text-muted-foreground">{formatBytes(record.sizeBytes)}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
