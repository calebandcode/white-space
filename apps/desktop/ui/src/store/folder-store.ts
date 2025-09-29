import { create } from "zustand"

import {
  invokeCommand,
  listStaged,
  stageFiles as invokeStageFiles,
  restoreStaged as invokeRestoreStaged,
  emptyStaged as invokeEmptyStaged,
  getDuplicateGroups as invokeDuplicateGroups,
  fetchScanStatus,
  type StageOptions as StageOptionsInput,
  type StagedFileRecord as StageRecord,
  type DuplicateGroup as DuplicateGroupResult,
} from "@/lib/ipc"
import type { DirectoryEntry, ScanCandidate, WatchedFolder } from "@/types/folders"

import { notifySweepReady } from "@/lib/notify"
import { toast } from "@/components/ui/use-toast"

const SOFT_SWEEP_THRESHOLD = 3 * 1024 ** 3
const HARD_SWEEP_THRESHOLD = 4 * 1024 ** 3

type SweepLevel = "none" | "soft" | "hard"

export type StageBucket = {
  key: string
  batchId: string | null
  records: StageRecord[]
  fileIds: number[]
  totalBytes: number
  stagedAt: string | null
  readyAt: string | null
}

export type StageBucketGroups = {
  cooling: StageBucket[]
  ready: StageBucket[]
}

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
  computed_at?: string | null
  window_start?: string | null
  window_end?: string | null
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

type StagedState = {
  items: StageRecord[]
  loading: boolean
  error: string | null
}

type DuplicateState = {
  groups: DuplicateGroupResult[]
  loading: boolean
  error: string | null
}

export type ScanInfo = {
  status: "idle" | "running" | "queued"
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

function determineSweepLevel(stagedBytes: number): SweepLevel {
  if (stagedBytes >= HARD_SWEEP_THRESHOLD) return "hard"
  if (stagedBytes >= SOFT_SWEEP_THRESHOLD) return "soft"
  return "none"
}

function parseIsoDate(value: string | null | undefined): number | null {
  if (!value) return null
  const ts = Date.parse(value)
  if (Number.isNaN(ts)) return null
  return ts
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
  gauge: {
    potentialBytes: number
    stagedBytes: number
    freedBytes: number
    computedAt: string | null
    windowStart: string | null
    windowEnd: string | null
    sweepLevel: SweepLevel
  }
  scan: ScanInfo
  staged: StagedState
  duplicates: DuplicateState
  queuedRoots: number
  loadPlatform: () => Promise<void>
  loadFolders: () => Promise<void>
  addFolder: () => Promise<void>
  removeFolder: (id: string) => Promise<void>
  selectFolder: (id: string | null) => Promise<void>
  loadDir: (folderId: string, pathOverride?: string) => Promise<void>
  loadGauge: () => Promise<void>
  openInSystem: (path: string) => Promise<void>
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
  stageFilesByIds: (fileIds: number[], options?: StageOptionsInput) => Promise<void>
  undoLast: () => Promise<void>
  listUndoableBatches: () => Promise<BackendUndoBatchSummary[]>
  undoBatch: (batchId: string) => Promise<BackendUndoResult>
  handleScanProgress: (payload: ScanProgressPayload) => void
  handleScanDone: (payload: ScanFinishedPayload) => Promise<void>
  loadStaged: (statuses?: string[]) => Promise<void>
  loadDuplicateGroups: () => Promise<void>
  stageSelected: (options?: StageOptionsInput) => Promise<void>
  restoreStageBatch: (batchId: string) => Promise<void>
  emptyStageFiles: (fileIds: number[], toTrash?: boolean) => Promise<void>
  handleScanQueued: (payload: { roots: number }) => void
  handleScanError: (payload: ScanErrorPayload) => void
  getStagedBuckets: () => StageBucketGroups
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
  gauge: {
    potentialBytes: 0,
    stagedBytes: 0,
    freedBytes: 0,
    computedAt: null,
    windowStart: null,
    windowEnd: null,
    sweepLevel: "none",
  },
  scan: initialScanInfo,
  staged: { items: [], loading: false, error: null },
  duplicates: { groups: [], loading: false, error: null },
  queuedRoots: 0,

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
      toast({ title: "Failed to load folders", description: message })
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
      const result = await invokeCommand<BackendGaugeState>("gauge_state")
      const stagedBytes = result.staged_week_bytes ?? 0
      const nextLevel = determineSweepLevel(stagedBytes)
      const currentLevel = get().gauge.sweepLevel

      if (nextLevel !== "none" && nextLevel !== currentLevel) {
        void notifySweepReady(stagedBytes)
      }

      set({
        gauge: {
          potentialBytes: result.potential_today_bytes ?? 0,
          stagedBytes,
          freedBytes: result.freed_week_bytes ?? 0,
          computedAt: result.computed_at ?? new Date().toISOString(),
          windowStart: result.window_start ?? null,
          windowEnd: result.window_end ?? null,
          sweepLevel: nextLevel,
        },
      })
    } catch (error) {
      console.error("Failed to load gauge state", error)
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
      toast({ title: "Add folder failed", description: message })
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
      toast({ title: "Remove folder failed", description: message })
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
      toast({ title: "Failed to load directory", description: message })
      set({
        isLoadingEntries: false,
        entryError: message,
        entries: [],
      })
    }
  },

  async openInSystem(path) {
    if (!path) return
    try {
      await invokeCommand("open_in_system", { path, reveal: false })
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to open in system", error)
      toast({ title: "Open failed", description: message })
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
      const status = get().scan.status
      if (status === "running" || status === "queued") {
        return
      }
      if ((!paths || paths.length === 0) && get().folders.length === 0) {
        toast({ title: "Scan failed", description: "ERR_VALIDATION: No scan roots configured" })
        return
      }
      await invokeCommand("start_scan", { paths: paths ?? null })
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to start scan", error)
      toast({ title: "Scan failed", description: message })
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
      const status = get().scan.status
      if (status === "running" || status === "queued") {
        return
      }
      if (get().folders.length === 0) {
        toast({ title: "Rescan all failed", description: "ERR_VALIDATION: No scan roots configured" })
        return
      }
      await invokeCommand("rescan_all", {})
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to rescan all", error)
      toast({ title: "Rescan all failed", description: message })
      set((state) => ({
        scan: { ...state.scan, errorMessages: [...state.scan.errorMessages, message], lastError: message },
      }))
    }
  },

  async rescanFolder(path) {
    if (!path) return
    try {
      const status = get().scan.status
      if (status === "running" || status === "queued") {
        return
      }
      await invokeCommand("rescan_folder", { path })
      await get().refreshScanStatus()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to rescan folder", error)
      toast({ title: "Rescan folder failed", description: message })
      set((state) => ({
        scan: { ...state.scan, errorMessages: [...state.scan.errorMessages, message], lastError: message },
      }))
    }
  },

  async refreshScanStatus() {
    try {
      const payload = await fetchScanStatus<ScanStatusPayload>()
      const { scan: prevScan, queuedRoots: prevQueued } = get()
      const nextStatus = payload.state === "running" ? "running" : "idle"
      const wasRunning = prevScan.status === "running"
      const nowIdle = wasRunning && nextStatus === "idle"
      const nextQueuedRoots = nextStatus === "running" ? 0 : prevQueued

      set(() => ({
        scan: {
          status: nextStatus,
          scanned: payload.scanned,
          skipped: payload.skipped,
          errors: payload.errors,
          startedAt: payload.started_at ?? null,
          finishedAt: payload.finished_at ?? null,
          currentPath: payload.current_path ?? null,
          errorMessages: prevScan.errorMessages,
          lastError: payload.last_error ?? null,
        },
        queuedRoots: nextQueuedRoots,
      }))

      if (payload.last_error && payload.last_error !== prevScan.lastError) {
        toast({ title: "Scan error", description: payload.last_error })
      }

      if (nowIdle) {
        const selectedFolderId = get().selectedFolderId
        await get().loadFolders()
        await Promise.all([
          get().loadGauge(),
          get().loadStaged(),
          get().loadDuplicateGroups(),
        ])
        if (selectedFolderId) {
          await get().loadDir(selectedFolderId)
        }
        await get().loadCandidates()
      }
    } catch (error) {
      console.error("Failed to refresh scan status", error)
      toast({ title: "Scan status error", description: extractErrorMessage(error) })
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
      toast({ title: "Failed to load candidates", description: extractErrorMessage(error) })
    }
  },

  async loadStaged(statuses?: string[]) {
    set((state) => ({
      staged: { ...state.staged, loading: true, error: null },
    }))
    try {
      const records = await listStaged(statuses)
      set((state) => ({
        staged: { ...state.staged, items: records, loading: false, error: null },
      }))
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to load staged entries", error)
      toast({ title: "Failed to load staged", description: message })
      set((state) => ({
        staged: { ...state.staged, loading: false, error: message },
      }))
    }
  },

  async loadDuplicateGroups() {
    set((state) => ({
      duplicates: { ...state.duplicates, loading: true, error: null },
    }))
    try {
      const groups = await invokeDuplicateGroups(50)
      set((state) => ({
        duplicates: { ...state.duplicates, groups, loading: false, error: null },
      }))
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to load duplicate groups", error)
      toast({ title: "Failed to load duplicates", description: message })
      set((state) => ({
        duplicates: { ...state.duplicates, loading: false, error: message },
      }))
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

  async stageSelected(options?: StageOptionsInput) {
    const ids = get().selectedCandidateIds
    if (!ids.length) return
    await get().stageFilesByIds(ids, options)
    set({ selectedCandidateIds: [] })
  },

  async stageFilesByIds(fileIds, options) {
    if (!Array.isArray(fileIds) || fileIds.length === 0) return
    try {
      await invokeStageFiles(fileIds, options)
      await Promise.all([
        get().loadGauge(),
        get().loadStaged(),
        get().loadDuplicateGroups(),
      ])
      await get().loadCandidates()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to stage files", error)
      toast({ title: "Stage failed", description: message })
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
      toast({ title: "Archive failed", description: message })
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
      toast({ title: "Delete failed", description: message })
      set((state) => ({
        scan: {
          ...state.scan,
          errorMessages: [...state.scan.errorMessages, message],
          lastError: message,
        },
      }))
    }
  },

  async restoreStageBatch(batchId: string) {
    if (!batchId) return
    try {
      await invokeRestoreStaged(batchId)
      await Promise.all([
        get().loadGauge(),
        get().loadStaged(),
        get().loadDuplicateGroups(),
      ])
      await get().loadCandidates()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to restore staged batch", error)
      toast({ title: "Restore failed", description: message })
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

  async emptyStageFiles(fileIds: number[], toTrash = false) {
    if (!fileIds.length) return
    try {
      await invokeEmptyStaged(fileIds, toTrash)
      await Promise.all([
        get().loadGauge(),
        get().loadStaged(),
        get().loadDuplicateGroups(),
      ])
      await get().loadCandidates()
    } catch (error) {
      const message = extractErrorMessage(error)
      console.error("Failed to empty staged files", error)
      toast({ title: "Empty staged failed", description: message })
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

  async undoLast() {
    try {
      await invokeCommand<BackendUndoResult>("undo_last")
      await get().loadGauge()
      await get().loadCandidates()
    } catch (error) {
      console.error("Failed to undo last batch", error)
      toast({ title: "Undo failed", description: extractErrorMessage(error) })
    }
  },

  async listUndoableBatches() {
    try {
      const result = await invokeCommand<BackendUndoBatchSummary[]>("list_undoable_batches")
      return result
    } catch (error) {
      console.error("Failed to list undoable batches", error)
      toast({ title: "Load undoable batches failed", description: extractErrorMessage(error) })
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
      toast({ title: "Undo batch failed", description: message })
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

  handleScanQueued(payload) {
    const roots = Math.max(0, payload?.roots ?? 0)
    set((state) => ({
      scan: {
        ...state.scan,
        status: state.scan.status === "running" ? state.scan.status : "queued",
      },
      queuedRoots: roots,
    }))
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
      queuedRoots: 0,
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
      queuedRoots: 0,
    }))

    await get().loadFolders()
    await Promise.all([
      get().loadGauge(),
      get().loadStaged(),
      get().loadDuplicateGroups(),
    ])
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
    toast({ title: "Scan error", description: payload.message })
  },

  getStagedBuckets() {
    const items = get().staged.items.filter((record) => record.status === "staged")
    if (!items.length) {
      return { cooling: [], ready: [] }
    }

    const grouped = new Map<string, StageRecord[]>()
    for (const record of items) {
      const key = record.batchId ?? `file-${record.recordId}`
      const collection = grouped.get(key)
      if (collection) {
        collection.push(record)
      } else {
        grouped.set(key, [record])
      }
    }

    const now = Date.now()
    const cooling: StageBucket[] = []
    const ready: StageBucket[] = []

    for (const [key, records] of grouped) {
      const fileIds = records
        .map((record) => record.fileId)
        .filter((id) => Number.isFinite(id) && id > 0)

      const totalBytes = records.reduce((sum, record) => sum + (record.sizeBytes ?? 0), 0)

      const stagedTimes = records
        .map((record) => parseIsoDate(record.stagedAt))
        .filter((value): value is number => value !== null)

      const earliestStaged = stagedTimes.length ? new Date(Math.min(...stagedTimes)).toISOString() : null

      const cooloffTimes = records
        .map((record) => parseIsoDate(record.cooloffUntil))
        .filter((value): value is number => value !== null)

      const latestCooloff = cooloffTimes.length ? new Date(Math.max(...cooloffTimes)).toISOString() : null
      const isCooling = latestCooloff !== null && parseIsoDate(latestCooloff)! > now

      const bucket: StageBucket = {
        key,
        batchId: records[0]?.batchId ?? null,
        records,
        fileIds,
        totalBytes,
        stagedAt: earliestStaged,
        readyAt: latestCooloff,
      }

      if (isCooling) {
        cooling.push(bucket)
      } else {
        ready.push(bucket)
      }
    }

    const byReadyTime = (a: StageBucket, b: StageBucket) => {
      const aReady = parseIsoDate(a.readyAt)
      const bReady = parseIsoDate(b.readyAt)
      if (aReady === null && bReady === null) return 0
      if (aReady === null) return -1
      if (bReady === null) return 1
      return aReady - bReady
    }

    cooling.sort(byReadyTime)
    ready.sort((a, b) => {
      const aStaged = parseIsoDate(a.stagedAt)
      const bStaged = parseIsoDate(b.stagedAt)
      if (aStaged === null && bStaged === null) return 0
      if (aStaged === null) return 1
      if (bStaged === null) return -1
      return bStaged - aStaged
    })

    return { cooling, ready }
  },
}))
