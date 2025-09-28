import * as React from "react"

import { FileRow } from "@/components/FileRow"
import { getExtensionFromPath } from "@/lib/fileIcons"
import type { DirectoryEntry } from "@/types/folders"

function formatBytes(value: number) {
  if (!value) return "0 B"
  const units = ['B', 'KB', 'MB', 'GB', 'TB'] as const
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  const size = value / Math.pow(1024, exponent)
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[exponent]}`
}

function formatTimestamp(value?: number) {
  if (!value) return ''
  try {
    const date = new Date(value * 1000)
    return new Intl.DateTimeFormat(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(date)
  } catch {
    return ''
  }
}

function formatParentDir(path: string | null | undefined): string | undefined {
  if (!path) return undefined
  const sanitized = path.startsWith('\\?\\') ? path.slice(4) : path
  const trimmed = sanitized.replace(/[\\/]+$/, '')
  const parts = trimmed.split(/[\\/]/).filter(Boolean)
  if (!parts.length) return sanitized
  parts.pop()
  return parts.pop()
}

interface DirectoryPanelProps {
  entries: DirectoryEntry[]
  loading?: boolean
  error?: string | null
  folderName?: string
  onRetry?: () => void
  onOpenEntry?: (entry: DirectoryEntry) => void
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



  const renderEntry = (entry: DirectoryEntry) => {
  const isDirectory = entry.kind === "dir"
  const sizeLabel = isDirectory ? "�" : formatBytes(entry.size)
  const ext = isDirectory ? null : getExtensionFromPath(entry.name)
  return (
    <FileRow
      key={entry.path}
      file={{
        id: entry.path,
        name: entry.name,
        path: entry.path,
        ext,
        isDirectory,
      }}
      subtitle={formatParentDir(entry.path)}
      endAccessory={
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className="tabular-nums">{sizeLabel}</span>
          <span>{formatTimestamp(entry.modified)}</span>
          <button
            type="button"
            onClick={() => onOpenEntry?.(entry)}
            className="rounded-md border border-border px-2 py-1 text-xs font-medium text-foreground transition hover:bg-accent"
          >
            Open
          </button>
        </div>
      }
    />
  )
}
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




