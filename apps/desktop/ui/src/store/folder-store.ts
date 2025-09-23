import { create } from "zustand"

import type { DirectoryEntry, ScanCandidate, WatchedFolder } from "@/types/folders"

type BackendWatchedFolder = {
  id: number
  name: string
  path: string
  is_accessible: boolean
}

type BackendDirectoryEntry = {
  name: string
  path: string
  kind: string
  size: number
  modified: number
}

type BackendPlatformInfo = {
  os: string
  open_label: string
}

type ScanProgressPayload = {
  scanned: number
  skipped: number
  errors: number
  path_sample?: string | null
}

type ScanFinishedPayload = {
  scanned: number
  skipped: number
  errors: number
  error_messages: string[]
  started_at?: string | null
  finished_at?: string | null
}

type ScanErrorPayload = {
  message: string
}

type ScanStatusPayload = {
  state: string
  scanned: number
  skipped: number
  errors: number
  started_at?: string | null
  finished_at?: string | null
  roots: number
  current_path?: string | null
  last_error?: string | null
}

type BackendGaugeState = {
  potential_today_bytes: number
  staged_week_bytes: number
  freed_week_bytes: number
}

type BackendCandidate = {
  file_id: number
  path: string
  parent_dir: string
  size_bytes: number
  reason: string
  score: number
  confidence: number
  preview_hint: string
  age_days: number
}

// Bucketed endpoint response types (minimal)
type UiCandidate = {
  id: number
  path: string
  parent: string
  size: number
  reason: string
}
type CandidatesResponse = {
  by_bucket: Record<string, UiCandidate[]>
}

type BackendUndoBatchSummary = {
  batch_id: string
  action_type: string
  file_count: number
  created_at: number
}

type BackendUndoResult = {
  batch_id: string
  actions_reversed: number
  files_restored: number
  duration_ms: number
  errors: string[]
  rollback_performed: boolean
}

type PlatformInfo = {
  os: string
  openLabel: string
}

export type ScanInfo = {
  status: "idle" | "running"
  scanned: number
  skipped: number
  errors: number
  startedAt?: string | null
  finishedAt?: string | null
  currentPath?: string | null
  errorMessages: string[]
  lastError?: string | null
}

const initialScanInfo: ScanInfo = {
  status: "idle",
  scanned: 0,
  skipped: 0,
  errors: 0,
  startedAt: null,
  finishedAt: null,
  currentPath: null,
  errorMessages: [],
  lastError: null,
}

async function openDirectoryDialog() {
  return invokeCommand<null | string>("pick_directory").then((result) => result ?? null)
}

async function invokeCommand<T = unknown>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core")
  return invoke<T>(command, args)
}

function extractErrorMessage(error: unknown): string {
  if (typeof error === "string") return error
  if (error instanceof Error) return error.message
  if (error && typeof error === "object" && "message" in error) {
    const value = (error as { message?: unknown }).message
    if (typeof value === "string") return value
  }
  return "An unexpected error occurred"
}

function mapWatchedFolder(folder: BackendWatchedFolder): WatchedFolder {
  return {
    id: String(folder.id),
    name: folder.name,
    path: folder.path,
    isAccessible: folder.is_accessible,
  }
}

function mapDirectoryEntry(entry: BackendDirectoryEntry): DirectoryEntry {
  return {
    name: entry.name,
    path: entry.path,
    kind: entry.kind === "dir" ? "dir" : "file",
    size: entry.size,
    modified: entry.modified,
  }
}

function mapCandidate(candidate: BackendCandidate): ScanCandidate {
  const reason = normalizeReason(candidate.reason)
  return {
    fileId: candidate.file_id,
    path: candidate.path,
    parentDir: candidate.parent_dir,
    sizeBytes: candidate.size_bytes,
    reason,
    score: candidate.score,
    confidence: candidate.confidence,
    previewHint: candidate.preview_hint,
    ageDays: candidate.age_days,
  }
}

function normalizeReason(value: string): string {
  const lower = value.toLowerCase()
  if (lower === "screenshots") return "screenshot"
  if (lower === "big downloads") return "big_download"
  if (lower === "old desktop") return "old_desktop"
  if (lower === "duplicates" || lower === "duplicate") return "duplicates"
  if (lower === "executables" || lower === ".exe" || lower === "exe") return "executable"
  return lower.replace(/\s+/g, "_")
}

interface FolderStoreState {
  platform: PlatformInfo | null
  folders: WatchedFolder[]
  entries: DirectoryEntry[]
  candidates: ScanCandidate[]
  selectedCandidateIds: number[]
  selectedFolderId: string | null
  isLoadingFolders: boolean
  isLoadingEntries: boolean
  folderError: string | null
  entryError: string | null
  gauge: { potentialBytes: number; stagedBytes: number; freedBytes: number; computedAt?: string | null }
  scan: ScanInfo
  loadPlatform: () => Promise<void>
  loadFolders: () => Promise<void>
  addFolder: () => Promise<void>
  removeFolder: (id: string) => Promise<void>
  selectFolder: (id: string | null) => Promise<void>
  loadDir: (folderId: string, pathOverride?: string) => Promise<void>
  loadGauge: () => Promise<void>
  openInSystem: (path: string, reveal?: boolean) => Promise<void>
  startScan: (paths?: string[]) => Promise<void>
  rescanAll: () => Promise<void>
  rescanFolder: (path: string) => Promise<void>
  refreshScanStatus: () => Promise<void>
  loadCandidates: () => Promise<void>
  toggleCandidate: (fileId: number) => void
  clearSelection: () => void
  selectAllCandidates: () => void
  archiveSelected: () => Promise<void>
  deleteSelected: (toTrash?: boolean) => Promise<void>
  undoLast: () => Promise<void>
  listUndoableBatches: () => Promise<BackendUndoBatchSummary[]>
  undoBatch: (batchId: string) => Promise<BackendUndoResult>
  handleScanProgress: (payload: ScanProgressPayload) => void
  handleScanDone: (payload: ScanFinishedPayload) => Promise<void>
  handleScanError: (payload: ScanErrorPayload) => void
}

export const useFolderStore = create<FolderStoreState>((set, get) => ({
  platform: null,
  folders: [],
  entries: [],
  candidates: [],
  selectedCandidateIds: [],
  selectedFolderId: null,
  isLoadingFolders: false,
  isLoadingEntries: false,
  folderError: null,
  entryError: null,
  gauge: { potentialBytes: 0, stagedBytes: 0, freedBytes: 0, computedAt: null },
  scan: initialScanInfo,

  async loadPlatform() {
    try {
      const result = await invokeCommand<BackendPlatformInfo>("get_platform_info")
      set({
        platform: {
          os: result.os,
          openLabel: result.open_label,
        },
      })
    } catch (error) {
      console.error("Failed to load platform info", error)
    }
  },

  async loadFolders() {
    set({ isLoadingFolders: true, folderError: null })
    try {
      const data = await invokeCommand<BackendWatchedFolder[]>("list_folders")
      const folders = data.map(mapWatchedFolder)
      set((state) => ({
        folders,
        isLoadingFolders: false,
        folderError: null,
        selectedFolderId: state.selectedFolderId && folders.some((f) => f.id === state.selectedFolderId)
          ? state.selectedFolderId
          : folders.length ? folders[0].id : null,
      }))
      const { selectedFolderId } = get()
      if (selectedFolderId) {
        void get().loadDir(selectedFolderId)
      }
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to load folders", error)
      set({
        isLoadingFolders: false,
        folderError: message,
        folders: [],
        selectedFolderId: null,
      })
    }
  },

  async loadGauge() {
    try {
      const result = await invokeCommand<BackendGaugeState>('gauge_state')
      set({
        gauge: {
          potentialBytes: result.potential_today_bytes ?? 0,
          stagedBytes: result.staged_week_bytes ?? 0,
          freedBytes: result.freed_week_bytes ?? 0,
          computedAt: new Date().toISOString(),
        },
      })
    } catch (error) {
      console.error('Failed to load gauge state', error)
    }
  },

  async addFolder() {
    try {
      const selection = await openDirectoryDialog()
      if (!selection || Array.isArray(selection)) return
      const folder = await invokeCommand<BackendWatchedFolder>("add_folder", { path: selection })
      const mapped = mapWatchedFolder(folder)
      set((state) => {
        const existingIndex = state.folders.findIndex((f) => f.id === mapped.id)
        const nextFolders = existingIndex >= 0
          ? state.folders.map((f, index) => (index === existingIndex ? mapped : f))
          : [...state.folders, mapped]
        return {
          folders: nextFolders,
          selectedFolderId: mapped.id,
        }
      })
      await get().loadDir(mapped.id)
      // Kick a focused scan on the newly added root so candidates populate
      await get().startScan([mapped.path])
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to add folder", error)
      set({ folderError: message })
    }
  },

  async removeFolder(id) {
    try {
      const numericId = Number(id)
      if (!Number.isFinite(numericId) || numericId <= 0) return
      await invokeCommand("remove_folder", { id: numericId })
      await get().loadFolders()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to remove folder", error)
      set({ folderError: message })
    }
  },

  async selectFolder(id) {
    if (!id) {
      set({ selectedFolderId: null, entries: [], entryError: null })
      return
    }
    set({ selectedFolderId: id, entryError: null })
    await get().loadDir(id)
  },

  async loadDir(folderId, pathOverride) {
    const folder = get().folders.find((f) => f.id === folderId)
    if (!folder) {
      set({ entryError: "Folder not found", entries: [] })
      return
    }
    if (!folder.isAccessible) {
      set({ entryError: "Folder is not accessible", entries: [] })
      return
    }
    const rootPath = pathOverride ?? folder.path
    set({ isLoadingEntries: true, entryError: null })
    try {
      const data = await invokeCommand<BackendDirectoryEntry[]>("list_dir", { rootPath })
      set({
        entries: data.map(mapDirectoryEntry),
        isLoadingEntries: false,
        entryError: null,
      })
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to load directory entries", error)
      set({
        isLoadingEntries: false,
        entryError: message,
        entries: [],
      })
    }
  },

  async openInSystem(path, reveal = false) {
    if (!path) return
    try {
      await invokeCommand("open_in_system", { path, reveal })
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to open in system", error)
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
    }
  },

  async startScan(paths) {
    try {
      await invokeCommand("start_scan", { paths: paths ?? null })
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to start scan", error)
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
    }
  },

  async rescanAll() {
    try {
      await invokeCommand("rescan_all", {})
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to rescan all", error)
      set((state) => ({
        scan: { ...state.scan, errorMessages: [...state.scan.errorMessages, message], lastError: message },
      }))
    }
  },

  async rescanFolder(path) {
    if (!path) return
    try {
      await invokeCommand("rescan_folder", { path })
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to rescan folder", error)
      set((state) => ({
        scan: { ...state.scan, errorMessages: [...state.scan.errorMessages, message], lastError: message },
      }))
    }
  },

  async refreshScanStatus() {
    try {
      const payload = await invokeCommand<ScanStatusPayload>("scan_status")
      const prev = get().scan
      const nextStatus = payload.state === "running" ? "running" : "idle"
      const wasRunning = prev.status === "running"
      const nowIdle = wasRunning && nextStatus === "idle"

      set(() => ({
        scan: {
          status: nextStatus,
          scanned: payload.scanned,
          skipped: payload.skipped,
          errors: payload.errors,
          startedAt: payload.started_at ?? null,
          finishedAt: payload.finished_at ?? null,
          currentPath: payload.current_path ?? null,
          errorMessages: prev.errorMessages,
          lastError: payload.last_error ?? null,
        },
      }))

      if (nowIdle) {
        // Scan just finished: refresh views and candidates
        const selectedFolderId = get().selectedFolderId
        await get().loadFolders()
        await get().loadGauge()
        if (selectedFolderId) {
          await get().loadDir(selectedFolderId)
        }
        await get().loadCandidates()
      }
    } catch (error) {
      console.error("Failed to refresh scan status", error)
    }
  },

  async loadCandidates() {
    try {
      const state = get()
      const folder = state.folders.find((f) => f.id === state.selectedFolderId)
      if (!folder) {
        console.error('No folder selected')
        return
      }
      
      // Use the folder's path as root_path to scope the candidates
      const response = await invokeCommand<CandidatesResponse>(
        "get_candidates_bucketed",
        { 
          params: { 
            root_path: folder.path,
            limit: 100, 
            offset: 0, 
            sort: "size_desc",
            // Include all bucket types
            buckets: ["duplicate", "big_download", "old_desktop", "screenshot", "executable", "other"]
          } 
        }
      )
      const flattened: BackendCandidate[] = []
      for (const [bucket, arr] of Object.entries(response.by_bucket ?? {})) {
        for (const c of arr) {
          flattened.push({
            file_id: c.id || 0,
            path: c.path,
            parent_dir: c.parent,
            size_bytes: c.size,
            reason: bucket,
            score: 0,
            confidence: 0,
            preview_hint: "",
            age_days: 0,
          })
        }
      }
      set({ candidates: flattened.map(mapCandidate), selectedCandidateIds: [] })
    } catch (error) {
      console.error("Failed to load candidates", error)
    }
  },

  toggleCandidate(fileId) {
    set((state) => {
      const exists = state.selectedCandidateIds.includes(fileId)
      return {
        selectedCandidateIds: exists
          ? state.selectedCandidateIds.filter((id) => id !== fileId)
          : [...state.selectedCandidateIds, fileId],
      }
    })
  },

  clearSelection() {
    set({ selectedCandidateIds: [] })
  },

  selectAllCandidates() {
    set((state) => ({ selectedCandidateIds: state.candidates.map((c) => c.fileId) }))
  },

  async archiveSelected() {
    const ids = get().selectedCandidateIds
    if (!ids.length) return
    try {
      await invokeCommand("archive_files", { fileIds: ids })
      set({ selectedCandidateIds: [] })
      await get().loadGauge()
      await get().loadCandidates()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to archive files", error)
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
    }
  },

  async deleteSelected(toTrash = true) {
    const ids = get().selectedCandidateIds
    if (!ids.length) return
    try {
      await invokeCommand("delete_files", { fileIds: ids, toTrash })
      set({ selectedCandidateIds: [] })
      await get().loadGauge()
      await get().loadCandidates()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to delete files", error)
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
    }
  },

  async undoLast() {
    try {
      await invokeCommand<BackendUndoResult>("undo_last")
      await get().loadGauge()
      await get().loadCandidates()
    } catch (error) {
      console.error("Failed to undo last batch", error)
    }
  },

  async listUndoableBatches() {
    try {
      const result = await invokeCommand<BackendUndoBatchSummary[]>("list_undoable_batches")
      return result
    } catch (error) {
      console.error("Failed to list undoable batches", error)
      return []
    }
  },

  async undoBatch(batchId) {
    try {
      const result = await invokeCommand<BackendUndoResult>("undo_batch", { batchId })
      await get().loadGauge()
      await get().loadCandidates()
      return result
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to undo batch", error)
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
      throw error
    }
  },

  handleScanProgress(payload) {
    set((state) => ({
      scan: {
        ...state.scan,
        status: "running",
        scanned: payload.scanned,
        skipped: payload.skipped,
        errors: payload.errors,
        currentPath: payload.path_sample ?? null,
      },
    }))
  },

  async handleScanDone(payload) {
    set((state) => ({
      scan: {
        status: "idle",
        scanned: payload.scanned,
        skipped: payload.skipped,
        errors: payload.errors,
        startedAt: payload.started_at ?? state.scan.startedAt ?? null,
        finishedAt: payload.finished_at ?? new Date().toISOString(),
        currentPath: null,
        errorMessages: payload.error_messages,
        lastError: payload.error_messages.at(-1) ?? null,
      },
    }))

    await get().loadFolders()
    await get().loadGauge()
    const { selectedFolderId } = get()
    if (selectedFolderId) {
      await get().loadDir(selectedFolderId)
    }
    await get().loadCandidates()
  },

  handleScanError(payload) {
    set((state) => ({
      scan: {
        ...state.scan,
        errorMessages: [...state.scan.errorMessages, payload.message],
        lastError: payload.message,
      },
    }))
  },
}))
