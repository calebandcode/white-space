import type { ReactElement } from "react"
import {
  File as IconFile,
  FileArchive,
  FileAudio,
  FileCode,
  FileCog,
  FileImage,
  FilePenLine,
  FileSpreadsheet,
  FileText,
  FileType,
  FileVideo,
  Image as ImageIcon,
} from "lucide-react"

type IconFactory = (props: { className?: string }) => ReactElement

const mapByExt: Record<string, IconFactory> = {
  pdf: (props) => <FileText {...props} />,
  png: (props) => <FileImage {...props} />,
  jpg: (props) => <FileImage {...props} />,
  jpeg: (props) => <FileImage {...props} />,
  gif: (props) => <FileImage {...props} />,
  webp: (props) => <FileImage {...props} />,
  svg: (props) => <ImageIcon {...props} />,
  mp4: (props) => <FileVideo {...props} />,
  mov: (props) => <FileVideo {...props} />,
  mkv: (props) => <FileVideo {...props} />,
  mp3: (props) => <FileAudio {...props} />,
  wav: (props) => <FileAudio {...props} />,
  flac: (props) => <FileAudio {...props} />,
  zip: (props) => <FileArchive {...props} />,
  "7z": (props) => <FileArchive {...props} />,
  rar: (props) => <FileArchive {...props} />,
  exe: (props) => <FileCog {...props} />,
  dmg: (props) => <FileCog {...props} />,
  msu: (props) => <FileCog {...props} />,
  sh: (props) => <FileCode {...props} />,
  js: (props) => <FileCode {...props} />,
  ts: (props) => <FileCode {...props} />,
  jsx: (props) => <FileCode {...props} />,
  tsx: (props) => <FileCode {...props} />,
  py: (props) => <FileCode {...props} />,
  rs: (props) => <FileCode {...props} />,
  go: (props) => <FileCode {...props} />,
  css: (props) => <FileCode {...props} />,
  html: (props) => <FileCode {...props} />,
  doc: (props) => <FilePenLine {...props} />,
  docx: (props) => <FilePenLine {...props} />,
  ppt: (props) => <FileType {...props} />,
  pptx: (props) => <FileType {...props} />,
  xls: (props) => <FileSpreadsheet {...props} />,
  xlsx: (props) => <FileSpreadsheet {...props} />,
}

export function getExtensionFromPath(path?: string | null): string | null {
  if (!path) return null
  const normalized = path.replace(/\\/g, "/")
  const segment = normalized.split("/").pop() ?? ""
  const lastDot = segment.lastIndexOf(".")
  if (lastDot <= 0) return null
  const ext = segment.slice(lastDot + 1).toLowerCase()
  return ext || null
}

export function FileIcon({
  ext,
  mime,
  className = "h-5 w-5",
}: {
  ext?: string | null
  mime?: string | null
  className?: string
}) {
  const lowerExt = (ext ?? "").toLowerCase()
  if (lowerExt && mapByExt[lowerExt]) {
    return mapByExt[lowerExt]({ className })
  }

  if (mime?.startsWith("image/")) return <FileImage className={className} />
  if (mime?.startsWith("video/")) return <FileVideo className={className} />
  if (mime?.startsWith("audio/")) return <FileAudio className={className} />
  if (mime === "application/zip" || mime?.includes("compressed")) {
    return <FileArchive className={className} />
  }
  if (mime?.includes("pdf")) return <FileText className={className} />

  return <IconFile className={className} />
}
