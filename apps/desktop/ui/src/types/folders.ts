export type WatchedFolder = {
  id: string
  name: string
  path: string
  isAccessible: boolean
  platformStyle?: "win" | "mac"
  stats?: { items: number; bytes: number }
}

export type DirectoryEntry = {
  name: string
  path: string
  kind: "dir" | "file"
  size: number
  modified: number
}

export type ScanCandidate = {
  fileId: number
  path: string
  parentDir: string
  sizeBytes: number
  reason: string
  score: number
  confidence: number
  previewHint: string
  ageDays: number
}
