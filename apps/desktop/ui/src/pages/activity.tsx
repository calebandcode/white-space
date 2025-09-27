import * as React from "react"
import { useNavigate } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { formatBytes } from "@/hooks/useGauge"
import { useFolderStore } from "@/store/folder-store"
import type { DuplicateGroup } from "@/lib/ipc"
import type { ScanCandidate } from "@/types/folders"

type TabKey = "duplicates" | "downloads" | "screenshots"

type CandidateGroup = {
  key: string
  label: string
  totalBytes: number
  files: ScanCandidate[]
}

const TABS: { id: TabKey; label: string }[] = [
  { id: "duplicates", label: "Duplicates" },
  { id: "downloads", label: "Large Downloads" },
  { id: "screenshots", label: "Screenshots" },
]

export function Activity() {
  const navigate = useNavigate()
  const [activeTab, setActiveTab] = React.useState<TabKey>("duplicates")

  const selectedFolderId = useFolderStore((state) => state.selectedFolderId)
  const folders = useFolderStore((state) => state.folders)
  const candidates = useFolderStore((state) => state.candidates)
  const duplicateGroups = useFolderStore((state) => state.duplicates.groups)
  const duplicatesLoading = useFolderStore((state) => state.duplicates.loading)
  const duplicateError = useFolderStore((state) => state.duplicates.error)
  const loadCandidates = useFolderStore((state) => state.loadCandidates)
  const loadDuplicateGroups = useFolderStore((state) => state.loadDuplicateGroups)
  const openInSystem = useFolderStore((state) => state.openInSystem)
  const selectFolder = useFolderStore((state) => state.selectFolder)
  const stageFilesByIds = useFolderStore((state) => state.stageFilesByIds)

  const selectedFolder = React.useMemo(() => {
    return folders.find((folder) => folder.id === selectedFolderId) ?? null
  }, [folders, selectedFolderId])

  React.useEffect(() => {
    if (!selectedFolderId && folders.length > 0) {
      void selectFolder(folders[0].id)
      return
    }

    if (selectedFolderId) {
      void loadCandidates()
      void loadDuplicateGroups()
    }
  }, [folders, loadCandidates, loadDuplicateGroups, selectFolder, selectedFolderId])

  const largeDownloadGroups = React.useMemo(() => {
    return groupCandidatesByParent(candidates, "big_download")
  }, [candidates])

  const screenshotGroups = React.useMemo(() => {
    return groupCandidatesByParent(candidates, "screenshot")
  }, [candidates])

  React.useEffect(() => {
    if (activeTab !== "duplicates") return
    if (duplicateGroups.length === 0 && !duplicatesLoading && !duplicateError) {
      if (largeDownloadGroups.length) {
        setActiveTab("downloads")
      } else if (screenshotGroups.length) {
        setActiveTab("screenshots")
      }
    }
  }, [activeTab, duplicateGroups.length, duplicatesLoading, duplicateError, largeDownloadGroups.length, screenshotGroups.length])

  const handleStageDuplicateGroup = React.useCallback(
    async (group: DuplicateGroup) => {
      const fileIds = group.files
        .filter((file) => !file.isStaged && Number.isFinite(file.id) && file.id > 0)
        .map((file) => file.id)
      if (!fileIds.length) return
      await stageFilesByIds(fileIds)
    },
    [stageFilesByIds]
  )

  const handleStageCandidateGroup = React.useCallback(
    async (group: CandidateGroup) => {
      const fileIds = group.files
        .map((file) => file.fileId)
        .filter((id) => Number.isFinite(id) && id > 0)
      if (!fileIds.length) return
      await stageFilesByIds(fileIds)
    },
    [stageFilesByIds]
  )

  const handleOpenPath = React.useCallback(
    (path: string) => {
      if (!path) return
      void openInSystem(path)
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
        <Button type="button" onClick={() => navigate("/")}>
          Go to watched folders
        </Button>
      </div>
    )
  }

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-5 px-4 py-6">
      <header className="space-y-1">
        <h1 className="text-xl font-semibold text-foreground">{selectedFolder.name}</h1>
        <p className="truncate text-sm text-muted-foreground">{selectedFolder.path}</p>
      </header>

      <nav className="flex gap-2 text-sm" aria-label="Candidate buckets">
        {TABS.map((tab) => {
          const isActive = activeTab === tab.id
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`rounded-full px-3 py-1 transition ${isActive ? "bg-primary text-primary-foreground shadow" : "bg-muted text-muted-foreground hover:bg-muted/80"}`}
            >
              {tab.label}
            </button>
          )
        })}
      </nav>

      <section className="space-y-4">
        {activeTab === "duplicates" ? (
          <DuplicateGroupsView
            groups={duplicateGroups}
            loading={duplicatesLoading}
            error={duplicateError}
            onStageGroup={handleStageDuplicateGroup}
            onOpenPath={handleOpenPath}
          />
        ) : null}
        {activeTab === "downloads" ? (
          <CandidateGroupsView
            groups={largeDownloadGroups}
            emptyLabel="No large downloads flagged yet."
            onStageGroup={handleStageCandidateGroup}
            onOpenPath={handleOpenPath}
          />
        ) : null}
        {activeTab === "screenshots" ? (
          <CandidateGroupsView
            groups={screenshotGroups}
            emptyLabel="No screenshots identified yet."
            onStageGroup={handleStageCandidateGroup}
            onOpenPath={handleOpenPath}
          />
        ) : null}
      </section>
    </div>
  )
}

export default Activity

function DuplicateGroupsView({
  groups,
  loading,
  error,
  onStageGroup,
  onOpenPath,
}: {
  groups: DuplicateGroup[]
  loading: boolean
  error: string | null
  onStageGroup: (group: DuplicateGroup) => void | Promise<void>
  onOpenPath: (path: string) => void
}) {
  if (loading) {
    return (
      <div className="rounded-lg border border-border/50 bg-muted/30 px-4 py-6 text-sm text-muted-foreground">
        Loading duplicate groups…
      </div>
    )
  }

  if (error) {
    return (
      <div className="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-6 text-sm text-destructive">
        {error}
      </div>
    )
  }

  if (!groups.length) {
    return (
      <div className="rounded-lg border border-border/40 bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
        No duplicate groups available yet. Run a scan to refresh suggestions.
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {groups.map((group) => {
        const unstaged = group.files.filter((file) => !file.isStaged)
        const totalBytes = group.files.reduce((sum, file) => sum + file.sizeBytes, 0)

        return (
          <div key={group.hash} className="rounded-lg border border-border/60 bg-background px-4 py-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-semibold text-foreground">{group.count} duplicate{group.count === 1 ? "" : "s"}</h3>
                <p className="text-xs text-muted-foreground">Total size {formatBytes(totalBytes)}</p>
              </div>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!unstaged.length}
                  onClick={() => void onStageGroup(group)}
                >
                  {unstaged.length ? `Stage ${unstaged.length}` : "All staged"}
                </Button>
              </div>
            </div>

            <ul className="mt-3 space-y-2">
              {group.files.map((file) => (
                <li key={file.id} className="flex flex-wrap items-center gap-3 text-sm">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-foreground" title={file.path}>
                      {file.path.split(/[\\/]/).pop()?.slice(0, 50) ?? file.path.slice(0, 2) + "..."} 
                    </p>
                    {/* <p className="truncate text-xs text-muted-foreground" title={file.path}>
                      {file.parentDir}
                    </p> */}
                  </div>
                  <span className="text-xs text-muted-foreground">{formatBytes(file.sizeBytes)}</span>
                  <Button variant="ghost" size="sm" onClick={() => onOpenPath(file.path)}>
                    Open
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        )
      })}
    </div>
  )
}

function CandidateGroupsView({
  groups,
  emptyLabel,
  onStageGroup,
  onOpenPath,
}: {
  groups: CandidateGroup[]
  emptyLabel: string
  onStageGroup: (group: CandidateGroup) => void | Promise<void>
  onOpenPath: (path: string) => void
}) {
  if (!groups.length) {
    return (
      <div className="rounded-lg border border-border/40 bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
        {emptyLabel}
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {groups.map((group) => (
        <div key={group.key} className="rounded-lg border border-border/60 bg-background px-4 py-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <h3 className="text-sm font-semibold text-foreground" title={group.label}>
                {group.label.slice(0, 30) + "..." || "Unknown location"}
              </h3>
              <p className="text-xs text-muted-foreground">
                {group.files.length} item{group.files.length === 1 ? "" : "s"} • {formatBytes(group.totalBytes)}
              </p>
            </div>
            <Button variant="outline" size="sm" onClick={() => void onStageGroup(group)}>
              Stage group
            </Button>
          </div>

          <ul className="mt-3 space-y-2">
            {group.files.map((file) => (
              <li key={file.fileId} className="flex flex-wrap items-center gap-3 text-sm">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-foreground" title={file.path}>
                    {file.path.split(/[\\/]/).pop() ?? file.path.slice(0, 2)}
                  </p>
                  {/* <p className="truncate text-xs text-muted-foreground" title={file.path}>
                    {file.parentDir.slice(0, 2)}
                  </p> */}
                </div>
                <span className="text-xs text-muted-foreground">{formatBytes(file.sizeBytes)}</span>
                <Button variant="ghost" size="sm" onClick={() => onOpenPath(file.path)}>
                  Open
                </Button>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  )
}

function groupCandidatesByParent(candidates: ScanCandidate[], reason: string): CandidateGroup[] {
  const buckets = new Map<string, CandidateGroup>()
  for (const candidate of candidates) {
    if (candidate.reason !== reason) continue
    const key = candidate.parentDir || "(unknown)"
    const existing = buckets.get(key)
    if (existing) {
      existing.files.push(candidate)
      existing.totalBytes += candidate.sizeBytes
    } else {
      buckets.set(key, {
        key: `${reason}:${key}`,
        label: key,
        totalBytes: candidate.sizeBytes,
        files: [candidate],
      })
    }
  }
  const groups = Array.from(buckets.values())
  groups.sort((a, b) => b.totalBytes - a.totalBytes)
  return groups
}
