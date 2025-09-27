import * as React from "react"
import { useNavigate } from "react-router-dom"

import BottomBar from "@/components/BottomBar"
import { FolderGrid } from "@/components/folders/FolderGrid"
import { useScanEvents } from "@/hooks/use-scan-events"
import { useFolderStore } from "@/store/folder-store"

function useFolderSelectors() {
  const folders = useFolderStore((state) => state.folders)
  const selectedFolderId = useFolderStore((state) => state.selectedFolderId)
  const isLoadingFolders = useFolderStore((state) => state.isLoadingFolders)
  const folderError = useFolderStore((state) => state.folderError)
  const platform = useFolderStore((state) => state.platform)
  const scan = useFolderStore((state) => state.scan)
  const gauge = useFolderStore((state) => state.gauge)
  const queuedRoots = useFolderStore((state) => state.queuedRoots)

  return {
    folders,
    selectedFolderId,
    isLoadingFolders,
    folderError,
    platform,
    scan,
    gauge,
    queuedRoots,
  }
}

export function Home() {
  useScanEvents()

  const navigate = useNavigate()
  const {
    folders,
    selectedFolderId,
    isLoadingFolders,
    folderError,
    platform,
    scan,
    gauge,
    queuedRoots,
  } = useFolderSelectors()

  const loadPlatform = useFolderStore((state) => state.loadPlatform)
  const loadFolders = useFolderStore((state) => state.loadFolders)
  const loadGauge = useFolderStore((state) => state.loadGauge)
  const loadStaged = useFolderStore((state) => state.loadStaged)
  const loadDuplicateGroups = useFolderStore((state) => state.loadDuplicateGroups)
  const addFolder = useFolderStore((state) => state.addFolder)
  const removeFolder = useFolderStore((state) => state.removeFolder)
  const selectFolder = useFolderStore((state) => state.selectFolder)
  const openInSystem = useFolderStore((state) => state.openInSystem)

  React.useEffect(() => {
    void loadPlatform()
    void loadFolders()
    void loadGauge()
    void loadStaged()
    void loadDuplicateGroups()
  }, [loadDuplicateGroups, loadFolders, loadGauge, loadPlatform, loadStaged])

  const openLabel = React.useMemo(() => platform?.openLabel ?? "Open", [platform])

  const handleSelectFolder = React.useCallback(
    async (id: string, multi?: boolean) => {
      await selectFolder(id)
      if (!multi) {
        navigate("/activity")
      }
    },
    [navigate, selectFolder]
  )

  const handleOpenFolder = React.useCallback(
    (id: string) => {
      const folder = folders.find((item) => item.id === id)
      if (!folder) return
      void openInSystem(folder.path)
    },
    [folders, openInSystem]
  )

  const handleOpenArchive = React.useCallback(() => {
    navigate("/archive")
  }, [navigate])

  const scanning = scan.status === "running" || scan.status === "queued"

  const currentPathDisplay = React.useMemo(() => {
    if (!scan.currentPath) return null

    let stripped = scan.currentPath
    if (stripped.startsWith("\\\\?\\")) {
      stripped = stripped.slice(4)
    }

    if (stripped.startsWith("UNC\\")) {
      return `\\\\${stripped.slice(4)}`
    }

    return stripped
  }, [scan.currentPath])

  const scanSummary = React.useMemo(() => {
    if (scan.status === "queued") {
      if (queuedRoots > 0) {
        const label = queuedRoots === 1 ? "folder" : "folders"
        return `${queuedRoots} ${label} queued`
      }
      return "Scan queued"
    }
    return `${scan.scanned.toLocaleString()} processed, ${scan.skipped.toLocaleString()} skipped, ${scan.errors.toLocaleString()} errors`
  }, [queuedRoots, scan.errors, scan.scanned, scan.skipped, scan.status])

  const lastScanSummary = React.useMemo(() => {
    return `${scan.scanned.toLocaleString()} processed, ${scan.errors.toLocaleString()} errors`
  }, [scan.errors, scan.scanned])

  const hasEmptyState = !isLoadingFolders && folders.length === 0
  const finishedAtISO = scan.finishedAt ?? undefined

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col px-4 pb-20 pt-6">
      <div className="flex flex-1 flex-col gap-6">
        <FolderGrid
          folders={folders}
          selectedIds={selectedFolderId ? [selectedFolderId] : []}
          onAddFolder={() => void addFolder()}
          onSelect={(id, multi) => void handleSelectFolder(id, multi)}
          onOpen={handleOpenFolder}
          onRemove={(id) => void removeFolder(id)}
          openLabel={openLabel}
        />
      </div>

      <BottomBar
        scanning={scanning}
        scanSummary={scanSummary}
        currentPathDisplay={currentPathDisplay ?? undefined}
        finishedAtISO={finishedAtISO}
        lastScanSummary={lastScanSummary}
        errorMessages={scan.errorMessages}
        folderError={folderError}
        hasEmptyState={hasEmptyState}
        isLoadingFolders={isLoadingFolders}
        potentialBytes={gauge.potentialBytes}
        stagedBytes={gauge.stagedBytes}
        freedBytes={gauge.freedBytes}
        windowStart={gauge.windowStart}
        windowEnd={gauge.windowEnd}
        sweepLevel={gauge.sweepLevel}
        onOpenArchive={handleOpenArchive}
      />
    </div>
  )
}

export default Home
