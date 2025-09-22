import * as React from "react"
import { useNavigate } from "react-router-dom"

import type { DirectoryEntry } from "@/types/folders"
import { useFolderStore } from "@/store/folder-store"

export function Activity() {
  const navigate = useNavigate()
  const selectedFolderId = useFolderStore((state) => state.selectedFolderId)
  const folders = useFolderStore((state) => state.folders)
  const candidates = useFolderStore((state) => state.candidates)
  const selectedCandidateIds = useFolderStore((state) => state.selectedCandidateIds)
  const loadCandidates = useFolderStore((state) => state.loadCandidates)
  const openInSystem = useFolderStore((state) => state.openInSystem)
  const selectFolder = useFolderStore((state) => state.selectFolder)
  const toggleCandidate = useFolderStore((state) => state.toggleCandidate)
  const selectAllCandidates = useFolderStore((state) => state.selectAllCandidates)
  const clearSelection = useFolderStore((state) => state.clearSelection)
  const archiveSelected = useFolderStore((state) => state.archiveSelected)
  const deleteSelected = useFolderStore((state) => state.deleteSelected)

  const selectedFolder = React.useMemo(() => {
    return folders.find((folder) => folder.id === selectedFolderId)
  }, [folders, selectedFolderId])

  React.useEffect(() => {
    if (!selectedFolderId && folders.length > 0) {
      void selectFolder(folders[0].id)
      return
    }

    if (selectedFolderId) {
      void loadCandidates()
    }
  }, [folders, loadCandidates, selectFolder, selectedFolderId])

  const handleOpenPath = React.useCallback((path: string, reveal = false) => {
    void openInSystem(path, reveal)
  }, [openInSystem])

  if (!selectedFolder) {
    return (
      <div className="mx-auto flex w-full max-w-4xl flex-1 flex-col items-center justify-center gap-4 px-4 py-6 text-center">
        <div className="space-y-1">
          <h2 className="text-lg font-semibold text-foreground">No folder selected</h2>
          <p className="text-sm text-muted-foreground">
            Pick a watched folder on the Home screen to inspect its contents and sync activity.
          </p>
        </div>
        <button
          type="button"
          onClick={() => navigate("/")}
          className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm transition hover:bg-primary/90"
        >
          Go to watched folders
        </button>
      </div>
    )
  }

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-4 px-4 py-6">
      <header className="space-y-1">
        <h1 className="text-xl font-semibold text-foreground">{selectedFolder.name}</h1>
        <p className="truncate text-sm text-muted-foreground">{selectedFolder.path}</p>
      </header>

      <section className="space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <button type="button" onClick={selectAllCandidates} className="rounded bg-secondary px-2 py-1 text-xs">Select all</button>
            <button type="button" onClick={clearSelection} className="rounded bg-secondary px-2 py-1 text-xs">Clear</button>
          </div>
          <div className="flex items-center gap-2">
            <button type="button" onClick={() => void archiveSelected()} className="rounded bg-primary px-3 py-1 text-xs text-primary-foreground">Archive selected</button>
            <button type="button" onClick={() => void deleteSelected(true)} className="rounded bg-destructive px-3 py-1 text-xs text-destructive-foreground">Delete to trash</button>
          </div>
        </div>
        <div className="text-xs text-muted-foreground">{selectedCandidateIds.length} selected</div>
      </section>

      <section className="space-y-4">
        {renderBucket("duplicates", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
        {renderBucket("big_download", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
        {renderBucket("old_desktop", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
        {renderBucket("screenshot", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
        {renderBucket("executable", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
        {renderBucket("other", candidates, toggleCandidate, selectedCandidateIds, handleOpenPath)}
      </section>
    </div>
  )
}

export default Activity

type CandidateRow = {
  fileId: number
  path: string
  parentDir: string
  sizeBytes: number
  reason: string
}

function renderBucket(
  bucketKey: string,
  items: CandidateRow[],
  toggle: (id: number) => void,
  selectedIds: number[],
  openPath: (path: string, reveal?: boolean) => void
) {
  const bucketItems = items.filter((c) => c.reason === bucketKey)
  if (!bucketItems.length) return null

  return (
    <div className="space-y-2">
      <h3 className="text-sm font-medium capitalize">{bucketKey.replace("_", " ")}</h3>
      <ul className="divide-y divide-border rounded-md border">
        {bucketItems.map((c) => {
          const checked = selectedIds.includes(c.fileId)
          return (
            <li key={c.fileId} className="flex items-center gap-3 px-3 py-2">
              <input type="checkbox" checked={checked} onChange={() => toggle(c.fileId)} />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm text-foreground">{c.path.split("/").pop() ?? c.path}</div>
                <div className="truncate text-xs text-muted-foreground">{c.parentDir}</div>
              </div>
              <div className="text-xs tabular-nums text-muted-foreground">{formatSize(c.sizeBytes)}</div>
              <div className="flex gap-2">
                <button className="rounded bg-secondary px-2 py-1 text-xs" onClick={() => openPath(c.path, false)}>Open</button>
                <button className="rounded bg-secondary px-2 py-1 text-xs" onClick={() => openPath(c.path, true)}>Reveal</button>
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

function formatSize(bytes: number) {
  const units = ["B","KB","MB","GB","TB"]
  let i = 0
  let n = bytes
  while (n >= 1024 && i < units.length - 1) {
    n = n / 1024
    i++
  }
  return `${n.toFixed(1)} ${units[i]}`
}
