import * as React from "react"

import { cn } from "@/lib/utils"
import { FolderContextMenu } from "./FolderContextMenu"
import type { WatchedFolder } from "@/types/folders"

import "./folder-styles.css"

export interface FolderItemProps {
  folder: WatchedFolder
  selected?: boolean
  onSelect?: (id: string, multi?: boolean) => void
  onOpen?: (id: string) => void
  onReveal?: (id: string) => void
  onRename?: (id: string, name: string) => void
  onRemove?: (id: string) => void
  openLabel?: string
  revealLabel?: string
}

export function FolderItem({
  folder,
  selected,
  onSelect,
  onOpen,
  onReveal,
  onRename,
  onRemove,
  openLabel,
  revealLabel,
}: FolderItemProps) {
  const [isRenaming, setIsRenaming] = React.useState(false)
  const itemRef = React.useRef<HTMLDivElement>(null)
  const inputRef = React.useRef<HTMLInputElement>(null)
  const hasClearedRef = React.useRef(false)

  const disabled = !folder.isAccessible

  const platformStyle = React.useMemo(() => {
    if (folder.platformStyle) return folder.platformStyle
    if (typeof navigator !== "undefined") {
      const platform = navigator.platform.toLowerCase()
      if (platform.includes("mac")) return "mac"
      if (platform.includes("win")) return "win"
    }
    return "win"
  }, [folder.platformStyle])

  const beginRename = React.useCallback(() => {
    if (disabled) return
    setIsRenaming(true)
  }, [disabled])

  const cancelRename = React.useCallback(() => {
    if (inputRef.current) {
      inputRef.current.value = folder.name
    }
    setIsRenaming(false)
  }, [folder.name])

  const commitRename = React.useCallback(() => {
    const value = (inputRef.current?.value ?? folder.name).trim()
    if (!value) {
      cancelRename()
      return
    }
    if (value !== folder.name) {
      onRename?.(folder.id, value)
    }
    setIsRenaming(false)
  }, [cancelRename, folder.id, folder.name, onRename])

  const commitRef = React.useRef(commitRename)
  React.useEffect(() => {
    commitRef.current = commitRename
  }, [commitRename])

  React.useEffect(() => {
    if (isRenaming && inputRef.current) {
      inputRef.current.value = folder.name
      hasClearedRef.current = false
    }
  }, [folder.name, isRenaming])

  React.useEffect(() => {
    if (!isRenaming) return
    const input = inputRef.current
    if (!input) return

    input.value = folder.name
    input.focus({ preventScroll: true })
    input.setSelectionRange(0, input.value.length)

    const handleClickOutside = (event: MouseEvent) => {
      if (!itemRef.current?.contains(event.target as Node)) {
        commitRef.current()
      }
    }

    window.addEventListener("mousedown", handleClickOutside)
    return () => window.removeEventListener("mousedown", handleClickOutside)
  }, [folder.name, isRenaming])

  const handleGridNavigation = React.useCallback(
    (direction: "ArrowRight" | "ArrowLeft" | "ArrowUp" | "ArrowDown") => {
      const node = itemRef.current
      if (!node) return
      const grid = node.closest<HTMLElement>("[data-folder-grid]")
      if (!grid) return
      const items = Array.from(
        grid.querySelectorAll<HTMLElement>("[data-folder-item]")
      )
      const index = items.indexOf(node)
      if (index < 0) return
      const columns = Number(grid.dataset.columns ?? "2")
      let nextIndex = index
      switch (direction) {
        case "ArrowRight":
          nextIndex = index + 1
          break
        case "ArrowLeft":
          nextIndex = index - 1
          break
        case "ArrowDown":
          nextIndex = index + columns
          break
        case "ArrowUp":
          nextIndex = index - columns
          break
      }
      const nextNode = items[nextIndex]
      if (nextNode) {
        nextNode.focus()
      }
    },
    []
  )

  const handleSelect = React.useCallback(
    (event: React.MouseEvent | React.KeyboardEvent, overrideMulti?: boolean) => {
      if (disabled) return
      const multi = overrideMulti ?? (("metaKey" in event && event.metaKey) || ("ctrlKey" in event && event.ctrlKey) || ("shiftKey" in event && event.shiftKey))
      onSelect?.(folder.id, multi)
    },
    [disabled, folder.id, onSelect]
  )

  const handleClick = React.useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      handleSelect(event)
    },
    [handleSelect]
  )

  const handleDoubleClick = React.useCallback(() => {
    if (disabled) return
    onSelect?.(folder.id, false)
    onOpen?.(folder.id)
  }, [disabled, folder.id, onOpen, onSelect])

  const handleKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.defaultPrevented) return

      switch (event.key) {
        case "Enter":
        case " ":
          event.preventDefault()
          if (!disabled) {
            onSelect?.(folder.id, false)
            onOpen?.(folder.id)
          }
          break
        case "F2":
          event.preventDefault()
          beginRename()
          break
        case "Escape":
          if (isRenaming) {
            event.preventDefault()
            cancelRename()
          }
          break
        case "ArrowRight":
        case "ArrowLeft":
        case "ArrowUp":
        case "ArrowDown":
          event.preventDefault()
          handleGridNavigation(event.key)
          break
      }
    },
    [beginRename, cancelRename, disabled, folder.id, handleGridNavigation, isRenaming, onOpen, onSelect]
  )

  const handleContextMenu = React.useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (disabled) return
      handleSelect(event, true)
    },
    [disabled, handleSelect]
  )

  const label = `${folder.name} - ${folder.path}`

  return (
    <FolderContextMenu
      folder={folder}
      disabled={disabled}
      openLabel={openLabel}
      revealLabel={revealLabel}
      onOpen={onOpen}
      onReveal={onReveal}
      onRename={() => beginRename()}
      onRemove={onRemove}
      onSelect={(id, multi) => {
        if (id === folder.id) {
          onSelect?.(id, multi ?? true)
        }
      }}
    >
      <div
        ref={itemRef}
        data-folder-item
        data-selected={selected ? "true" : "false"}
        data-disabled={disabled ? "true" : "false"}
        className={cn(
          "folder-item group relative flex w-full flex-col items-center rounded-xl px-3 py-2 transition-transform focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary focus-visible:outline-offset-2",
          disabled ? "cursor-default" : "cursor-pointer"
        )}
        role="listitem"
        tabIndex={disabled ? -1 : 0}
        aria-selected={selected ? "true" : "false"}
        aria-label={label}
        aria-disabled={disabled ? "true" : "false"}
        title={folder.name}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onKeyDown={handleKeyDown}
        onContextMenu={handleContextMenu}
      >
        <div
          className={cn(
            "folder-icon",
            platformStyle === "mac" ? "folder--mac" : "folder--win"
          )}
          aria-hidden="true"
        />
        <div className="folder-name mt-2 w-full truncate text-sm font-medium text-foreground">
          {isRenaming ? (
            <input
              ref={inputRef}
              defaultValue={folder.name}
              onBlur={commitRename}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault()
                  commitRename()
                }
                if (event.key === "Escape") {
                  event.preventDefault()
                  cancelRename()
                }
              }}
              className="w-full rounded-md border border-border bg-background px-2 py-1 text-center text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              aria-label="Rename folder"
              autoFocus
            />
          ) : (
            <span title={folder.name}>{folder.name}</span>
          )}
        </div>
        {/* <div className="mt-1 w-full truncate text-xs text-muted-foreground" title={folder.path}>
          {folder.path}
        </div> */}
        {!folder.isAccessible ? (
          <span className="mt-2 rounded-full bg-destructive/10 px-2 py-0.5 text-[11px] font-medium text-destructive">
            Not accessible
          </span>
        ) : null}
        {folder.stats ? (
          <span className="mt-2 text-xs text-muted-foreground">
            {folder.stats.items} items
          </span>
        ) : null}
      </div>
    </FolderContextMenu>
  )
}






