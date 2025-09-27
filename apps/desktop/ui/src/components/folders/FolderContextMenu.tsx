import * as React from "react"
import * as ContextMenuPrimitive from "@radix-ui/react-context-menu"

import { cn } from "@/lib/utils"
import type { WatchedFolder } from "@/types/folders"

interface FolderContextMenuProps {
  folder: WatchedFolder
  disabled?: boolean
  openLabel?: string
  onOpen?: (id: string) => void
  onRename?: (id: string) => void
  onRemove?: (id: string) => void
  onSelect?: (id: string, multi?: boolean) => void
  children: React.ReactNode
}

const itemClassName =
  "flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-40"

export function FolderContextMenu({
  folder,
  disabled,
  openLabel,
  onOpen,
  onRename,
  onRemove,
  onSelect,
  children,
}: FolderContextMenuProps) {
  const resolvedOpenLabel = openLabel ?? "Open"

  const handleOpen = React.useCallback(() => {
    if (disabled) return
    onSelect?.(folder.id, true)
    onOpen?.(folder.id)
  }, [disabled, folder.id, onOpen, onSelect])

  const handleRename = React.useCallback(() => {
    if (disabled) return
    onSelect?.(folder.id, true)
    onRename?.(folder.id)
  }, [disabled, folder.id, onRename, onSelect])

  const handleRemove = React.useCallback(() => {
    if (disabled) return
    onSelect?.(folder.id, true)
    onRemove?.(folder.id)
  }, [disabled, folder.id, onRemove, onSelect])

  return (
    <ContextMenuPrimitive.Root>
      <ContextMenuPrimitive.Trigger asChild>
        {children}
      </ContextMenuPrimitive.Trigger>
      <ContextMenuPrimitive.Content
        className="z-50 min-w-[180px] rounded-md border bg-popover p-1 text-popover-foreground shadow-md focus:outline-none"
        collisionPadding={8}
      >
        <ContextMenuPrimitive.Item
          className={cn(itemClassName)}
          data-disabled={disabled}
          onSelect={handleOpen}
        >
          {resolvedOpenLabel}
        </ContextMenuPrimitive.Item>
        <ContextMenuPrimitive.Separator className="my-1 h-px bg-border" />
        <ContextMenuPrimitive.Item
          className={cn(itemClassName)}
          data-disabled={disabled || !onRename}
          onSelect={handleRename}
        >
          Rename
        </ContextMenuPrimitive.Item>
        <ContextMenuPrimitive.Item
          className={cn(itemClassName, "text-destructive focus:bg-destructive/10 focus:text-destructive")}
          data-disabled={disabled || !onRemove}
          onSelect={handleRemove}
        >
          Remove from watched
        </ContextMenuPrimitive.Item>
      </ContextMenuPrimitive.Content>
    </ContextMenuPrimitive.Root>
  )
}
