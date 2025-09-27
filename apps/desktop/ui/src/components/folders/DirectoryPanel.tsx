import * as React from "react"

import type { DirectoryEntry } from "@/types/folders"

interface DirectoryPanelProps {
  entries: DirectoryEntry[]
  loading?: boolean
  error?: string | null
  folderName?: string
  onRetry?: () => void
  onOpenEntry?: (entry: DirectoryEntry) => void
}

function formatBytes(value: number) {
  if (!value) return "0 B"
  const units = ["B", "KB", "MB", "GB", "TB"] as const
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  const size = value / Math.pow(1024, exponent)
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[exponent]}`
}

function formatTimestamp(value?: number) {
  if (!value) return "";
  try {
    const date = new Date(value * 1000)
    return new Intl.DateTimeFormat(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(date)
  } catch {
    return ""
  }
}

export function DirectoryPanel({
  entries,
  loading,
  error,
  folderName,
  onRetry,
  onOpenEntry,
}: DirectoryPanelProps) {
  const directories = React.useMemo(
    () => entries.filter((entry) => entry.kind === "dir"),
    [entries]
  )
  const files = React.useMemo(
    () => entries.filter((entry) => entry.kind !== "dir"),
    [entries]
  )

  if (loading) {
    return (
      <div className="flex min-h-[160px] items-center justify-center rounded-xl border border-dashed border-border/80 bg-muted/40 text-sm text-muted-foreground">
        Loading directory�
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex min-h-[160px] flex-col items-center justify-center gap-3 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-6 text-center">
        <p className="text-sm text-destructive">{error}</p>
        {onRetry ? (
          <button
            type="button"
            onClick={onRetry}
            className="rounded-md bg-destructive px-3 py-1.5 text-sm font-medium text-destructive-foreground shadow-sm transition hover:bg-destructive/90"
          >
            Try again
          </button>
        ) : null}
      </div>
    )
  }

  if (!entries.length) {
    return (
      <div className="flex min-h-[160px] flex-col items-center justify-center rounded-xl border border-dashed border-border/80 bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
        {folderName ? (
          <>
            <p>No items in <span className="font-medium text-foreground">{folderName}</span>.</p>
            <p className="mt-2">Start a scan to collect metadata.</p>
          </>
        ) : (
          <p>Select a folder to view its contents.</p>
        )}
      </div>
    )
  }

  const renderEntry = (entry: DirectoryEntry) => (
    <div
      key={entry.path}
      className="grid grid-cols-[minmax(0,1fr)_110px_140px] items-center gap-3 rounded-lg px-3 py-2 text-sm transition hover:bg-muted/60"
    >
      <div className="flex flex-col">
        <span className="font-medium text-foreground" title={entry.path}>
          {entry.name}
        </span>
        <span className="text-xs text-muted-foreground" title={entry.path}>
          {entry.path}
        </span>
      </div>
      <span className="text-xs tabular-nums text-muted-foreground">
        {entry.kind === "dir" ? "�" : formatBytes(entry.size)}
      </span>
      <div className="flex items-center justify-end gap-2">
        <span className="text-xs text-muted-foreground">{formatTimestamp(entry.modified)}</span>
        <button
          type="button"
          onClick={() => onOpenEntry?.(entry)}
          className="rounded-md border border-border px-2 py-1 text-xs font-medium text-foreground transition hover:bg-accent"
        >
          Open
        </button>
       
      </div>
    </div>
  )

  const Section = ({ title, children }: { title: string; children: React.ReactNode }) => (
    <div className="flex flex-col gap-1">
      <div className="pl-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </div>
      <div className="flex flex-col gap-1">{children}</div>
    </div>
  )

  return (
    <div className="flex flex-col gap-4 rounded-xl border border-border/60 bg-background px-3 py-4">
      {directories.length ? <Section title="Folders">{directories.map(renderEntry)}</Section> : null}
      {files.length ? <Section title="Files">{files.map(renderEntry)}</Section> : null}
    </div>
  )
}
