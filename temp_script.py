from pathlib import Path
content = """import * as React from \"react\"
import * as ContextMenuPrimitive from \"@radix-ui/react-context-menu\"

import { cn } from \"@/lib/utils\"
import type { WatchedFolder } from \"@/types/folders\"

interface FolderContextMenuProps {
  folder: WatchedFolder
  platformStyle?: \"win\" | \"mac\"
  disabled?: boolean
  openLabel?: string
  revealLabel?: string
  onOpen?: (id: string) => void
  onReveal?: (id: string) => void
  onRename?: (id: string) => void
  onRemove?: (id: string) => void
  onSelect?: (id: string, multi?: boolean) => void
  children: React.ReactNode
}

const itemClassName =
  \"flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-40\"

export function FolderContextMenu({
  folder,
  platformStyle,
  disabled,
  openLabel,
  revealLabel,
  onOpen,
  onReveal,
  onRename,
  onRemove,
  onSelect,
  children,
}: FolderContextMenuProps) {
  const effectivePlatform = platformStyle ?? folder.platformStyle ?? \"win\"

  const resolvedOpenLabel = openLabel ?? \"Open\"
  const resolvedRevealLabel = React.useMemo(() => {
    if (revealLabel) return revealLabel
    if (effectivePlatform == "mac") return \"Reveal in Finder\"
    if (effectivePlatform == "win") return \"Reveal in File Explorer\"
    return \"Reveal in File Manager\"
  }, [effectivePlatform, revealLabel])

  const showReveal = effectivePlatform != \"win\" and resolvedRevealLabel != resolvedOpenLabel

  const handleOpen = React.useCallback(() => {
    if (disabled):
        return
    if onSelect is not None:
        onSelect(folder.id, True)
    if onOpen is not None:
        onOpen(folder.id)
  }, [disabled, folder.id, onOpen, onSelect])
