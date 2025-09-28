import type { ReactNode } from "react"
import { Folder } from "lucide-react"

import { cn } from "@/lib/utils"
import { FileIcon, getExtensionFromPath } from "@/lib/fileIcons"

function sanitizePath(value?: string | null): string | undefined {
  if (!value) return undefined;

  if (value.startsWith("\\\\?\\")) {
    const rest = value.slice(4); 

    if (rest.startsWith("UNC\\")) {
      return `\\\\${rest.slice(4)}`;
    }

    return rest;
  }

  return value;
}
function displayName(input: string, maxLength = 60): string {
  const sanitized = sanitizePath(input) ?? input
  const parts = sanitized.split(/[\\/]/)
  const name = parts.pop() ?? sanitized
  if (name.length <= maxLength) return name || sanitized
  return `${name.slice(0, maxLength - 1)}…`
}

export type UiFile = {
  id?: number | string
  name: string
  path: string
  mime?: string | null
  ext?: string | null
  isDirectory?: boolean
}

export interface FileRowProps {
  file: UiFile
  subtitle?: ReactNode
  endAccessory?: ReactNode
  className?: string
  onClick?: () => void
  children?: ReactNode
}

export function FileRow({
  file,
  subtitle,
  endAccessory,
  className,
  onClick,
  children,
}: FileRowProps) {
  const ext = file.ext ?? (file.isDirectory ? null : getExtensionFromPath(file.path))
  const name = displayName(file.name)
  const subtitleContent =
    typeof subtitle === "string"
      ? (() => {
          const clean = sanitizePath(subtitle) ?? subtitle
          return clean.length > 80 ? `${clean.slice(0, 79)}…` : clean
        })()
      : subtitle

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-md px-3 py-2 transition hover:bg-muted/40",
        onClick ? "cursor-pointer" : "",
        className,
      )}
      onClick={onClick}
    >
      {file.isDirectory ? (
        <Folder className="h-5 w-5 text-muted-foreground" />
      ) : (
        <FileIcon ext={ext} mime={file.mime} className="h-5 w-5 text-muted-foreground" />
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm text-foreground">{name}</div>
        {subtitleContent ? (
          <div className="truncate text-xs text-muted-foreground">{subtitleContent}</div>
        ) : null}
      </div>
      {endAccessory}
      {children}
    </div>
  )
}
