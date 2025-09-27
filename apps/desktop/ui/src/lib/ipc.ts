const loadCore = () => import("@tauri-apps/api/core")

export async function invokeCommand<T = unknown>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  const { invoke } = await loadCore()
  return invoke<T>(command, args)
}

export type StageOptions = {
  cooloffDays?: number
  note?: string | null
}

export type StageOutcome = {
  success: boolean
  batchId: string | null
  stagedFiles: number
  totalBytes: number
  durationMs: number
  errors: string[]
  expiresAt: string | null
  note: string | null
}

export type StagedFileRecord = {
  recordId: number
  fileId: number
  path: string
  parentDir: string
  sizeBytes: number
  status: string
  stagedAt: string
  expiresAt: string | null
  batchId: string | null
  note: string | null
  cooloffUntil: string | null
}

export type DuplicateGroupFile = {
  id: number
  path: string
  parentDir: string
  sizeBytes: number
  lastSeenAt: string
  isStaged: boolean
  cooloffUntil: string | null
}

export type DuplicateGroup = {
  hash: string
  totalSize: number
  count: number
  files: DuplicateGroupFile[]
}

export type UndoResult = {
  batchId: string
  actionsReversed: number
  filesRestored: number
  durationMs: number
  errors: string[]
  rollbackPerformed: boolean
}

export type DeleteOutcome = {
  success: boolean
  filesProcessed: number
  totalBytesFreed: number
  durationMs: number
  errors: string[]
  toTrash: boolean
}

const toSnakeOptions = (options?: StageOptions) => {
  if (!options) return undefined
  const payload: Record<string, unknown> = {}
  if (options.cooloffDays !== undefined) {
    payload.cooloff_days = options.cooloffDays
  }
  if (options.note !== undefined) {
    payload.note = options.note
  }
  return payload
}

const mapStageOutcome = (response: any): StageOutcome => ({
  success: Boolean(response?.success),
  batchId: response?.batch_id ?? null,
  stagedFiles: response?.staged_files ?? 0,
  totalBytes: response?.total_bytes ?? 0,
  durationMs: response?.duration_ms ?? 0,
  errors: Array.isArray(response?.errors) ? response.errors : [],
  expiresAt: response?.expires_at ?? null,
  note: response?.note ?? null,
})

const mapStagedFile = (response: any): StagedFileRecord => ({
  recordId: response?.record_id ?? 0,
  fileId: response?.file_id ?? 0,
  path: response?.path ?? "",
  parentDir: response?.parent_dir ?? "",
  sizeBytes: response?.size_bytes ?? 0,
  status: response?.status ?? "staged",
  stagedAt: response?.staged_at ?? "",
  expiresAt: response?.expires_at ?? null,
  batchId: response?.batch_id ?? null,
  note: response?.note ?? null,
  cooloffUntil: response?.cooloff_until ?? null,
})

const mapDuplicateGroupFile = (response: any): DuplicateGroupFile => ({
  id: response?.id ?? 0,
  path: response?.path ?? "",
  parentDir: response?.parent_dir ?? "",
  sizeBytes: response?.size_bytes ?? 0,
  lastSeenAt: response?.last_seen_at ?? "",
  isStaged: Boolean(response?.is_staged),
  cooloffUntil: response?.cooloff_until ?? null,
})

const mapDuplicateGroup = (response: any): DuplicateGroup => ({
  hash: response?.hash ?? "",
  totalSize: response?.total_size ?? 0,
  count: response?.count ?? 0,
  files: Array.isArray(response?.files)
    ? response.files.map(mapDuplicateGroupFile)
    : [],
})

const mapUndoResult = (response: any): UndoResult => ({
  batchId: response?.batch_id ?? "",
  actionsReversed: response?.actions_reversed ?? 0,
  filesRestored: response?.files_restored ?? 0,
  durationMs: response?.duration_ms ?? 0,
  errors: Array.isArray(response?.errors) ? response.errors : [],
  rollbackPerformed: Boolean(response?.rollback_performed),
})

const mapDeleteOutcome = (response: any): DeleteOutcome => ({
  success: Boolean(response?.success),
  filesProcessed: response?.files_processed ?? 0,
  totalBytesFreed: response?.total_bytes_freed ?? 0,
  durationMs: response?.duration_ms ?? 0,
  errors: Array.isArray(response?.errors) ? response.errors : [],
  toTrash: Boolean(response?.to_trash),
})

const compactArgs = (base: Record<string, unknown>) => {
  const entries = Object.entries(base).filter(([, value]) => value !== undefined)
  return Object.fromEntries(entries)
}

export async function listStaged(statuses?: string[]): Promise<StagedFileRecord[]> {
  const args = statuses && statuses.length ? { statuses } : {}
  const response = await invokeCommand<any[]>("list_staged", args)
  if (!Array.isArray(response)) return []
  return response.map(mapStagedFile)
}

export async function stageFiles(
  fileIds: number[],
  options?: StageOptions
): Promise<StageOutcome> {
  const args = compactArgs({ fileIds, options: toSnakeOptions(options) })
  const response = await invokeCommand<Record<string, unknown>>("stage_files", args)
  return mapStageOutcome(response)
}

export async function restoreStaged(batchId: string): Promise<UndoResult> {
  const response = await invokeCommand<Record<string, unknown>>("restore_staged", { batchId })
  return mapUndoResult(response)
}

export async function emptyStaged(
  fileIds: number[],
  toTrash: boolean
): Promise<DeleteOutcome> {
  const response = await invokeCommand<Record<string, unknown>>("empty_staged", { fileIds, toTrash })
  return mapDeleteOutcome(response)
}

export async function getDuplicateGroups(limit?: number): Promise<DuplicateGroup[]> {
  const args = limit ? { limit } : {}
  const response = await invokeCommand<any[]>("get_duplicate_groups", args)
  if (!Array.isArray(response)) return []
  return response.map(mapDuplicateGroup)
}

export async function fetchScanStatus<T>(): Promise<T> {
  return invokeCommand<T>("scan_status")
}
