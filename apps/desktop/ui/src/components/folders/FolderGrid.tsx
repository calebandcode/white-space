import * as React from "react"

import { cn } from "@/lib/utils"
import { FolderItem } from "./FolderItem"
import type { WatchedFolder } from "@/types/folders"

export interface FolderGridProps {
  folders: WatchedFolder[]
  selectedIds?: string[]
  onSelect?: (id: string, multi?: boolean) => void
  onOpen?: (id: string) => void
  onReveal?: (id: string) => void
  onRename?: (id: string, name: string) => void
  onRemove?: (id: string) => void
  openLabel?: string
  revealLabel?: string
  onAddFolder: () => void
}

export function FolderGrid({
  folders,
  selectedIds,
  onSelect,
  onOpen,
  onReveal,
  onRename,
  onRemove,
  openLabel,
  revealLabel,
  onAddFolder,
}: FolderGridProps) {
  const [internalSelection, setInternalSelection] = React.useState<string | null>(null)

  React.useEffect(() => {
    if (internalSelection && !folders.some((folder) => folder.id === internalSelection)) {
      setInternalSelection(null)
    }
  }, [folders, internalSelection])

  const effectiveSelection = React.useMemo(() => {
    if (selectedIds) return selectedIds
    return internalSelection ? [internalSelection] : []
  }, [internalSelection, selectedIds])

  const handleSelect = React.useCallback(
    (id: string, multi?: boolean) => {
      if (!selectedIds) {
        setInternalSelection(id)
      }
      onSelect?.(id, multi)
    },
    [onSelect, selectedIds]
  )

  const handleAddFolder = React.useCallback(() => {
    onAddFolder()
  }, [onAddFolder])

  const columns = 3

  return (
    <div className="flex justify-center">
      <div
        className="grid w-full max-w-[420px] grid-cols-3 gap-3 px-3 pb-3 pt-2 md:gap-10 mb-2.5"
        role="list"
        data-folder-grid
        data-columns={columns}
      >
        {folders.map((folder) => (
          <FolderItem
            key={folder.id}
            folder={folder}
            selected={effectiveSelection.includes(folder.id)}
            onSelect={handleSelect}
            onOpen={onOpen}
            onReveal={onReveal}
            onRename={onRename}
            onRemove={onRemove}
            openLabel={openLabel}
            revealLabel={revealLabel}
          />
        ))}
        <div role="listitem" className="flex">
          <button
            type="button"
            aria-label="Add folder"
            onClick={handleAddFolder}
            className={cn(
              "group relative flex h-full w-full flex-col items-center justify-start rounded-xl border border-dashed border-border/80 bg-muted/40 px-4 py-4 text-center text-sm font-medium text-muted-foreground transition hover:border-primary/60 hover:text-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary focus-visible:outline-offset-2"
            )}
          >
            <div className="folder-icon folder-add-icon folder-add-tile mb-2 text-primary" aria-hidden="true" />
            <span>Add folder</span>
          </button>
        </div>
      </div>
    </div>
  )
}

