import * as React from "react"
import { useNavigate } from "react-router-dom"

import { DirectoryPanel } from "@/components/folders/DirectoryPanel"
import type { DirectoryEntry } from "@/types/folders"
import { useFolderStore } from "@/store/folder-store"

export function Activity() {
  const navigate = useNavigate()
  const selectedFolderId = useFolderStore((state) => state.selectedFolderId)
  const folders = useFolderStore((state) => state.folders)
  const entries = useFolderStore((state) => state.entries)
  const isLoadingEntries = useFolderStore((state) => state.isLoadingEntries)
  const entryError = useFolderStore((state) => state.entryError)
  const loadDir = useFolderStore((state) => state.loadDir)
  const openInSystem = useFolderStore((state) => state.openInSystem)
  const selectFolder = useFolderStore((state) => state.selectFolder)

  const selectedFolder = React.useMemo(() => {
    return folders.find((folder) => folder.id === selectedFolderId)
  }, [folders, selectedFolderId])

  React.useEffect(() => {
    if (!selectedFolderId && folders.length > 0) {
      void selectFolder(folders[0].id)
      return
    }

    if (selectedFolderId) {
      void loadDir(selectedFolderId)
    }
  }, [folders, loadDir, selectFolder, selectedFolderId])

  const handleRetryEntries = React.useCallback(() => {
    if (selectedFolderId) {
      void loadDir(selectedFolderId)
    }
  }, [loadDir, selectedFolderId])

  const handleOpenEntry = React.useCallback(
    (entry: DirectoryEntry) => {
      void openInSystem(entry.path, entry.kind !== "dir")
    },
    [openInSystem]
  )

  const handleRevealEntry = React.useCallback(
    (entry: DirectoryEntry) => {
      void openInSystem(entry.path, true)
    },
    [openInSystem]
  )

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

      <DirectoryPanel
        entries={entries}
        loading={isLoadingEntries}
        error={entryError}
        folderName={selectedFolder.name}
        onRetry={handleRetryEntries}
        onOpenEntry={handleOpenEntry}
        onRevealEntry={handleRevealEntry}
      />
    </div>
  )
}

export default Activity
