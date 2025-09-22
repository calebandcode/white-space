You’re helping build **Empty Space** — a local-first desktop app for gentle digital decluttering.

PRINCIPLES

- Local-first, no uploads, no tracking.
- Archive-first safety (delete only after a 7-day cooling-off).
- Calm, progressive disclosure (summary → buckets → details).
- Single segmented gauge: Potential → Staged → Freed (¾ arc).
- Rolling 7-day window for metrics (or user “tidy day”).
- Strong safety rails: skip dev repos, path keywords, recent sibling activity, user deny/allow lists.

STACK

- Tauri 2 + React 18 + TypeScript + Vite
- Tailwind + shadcn/ui + framer-motion
- Zustand (+ React Query optional)
- SQLite (rusqlite, WAL) with refinery migrations
- Lemon Squeezy Licensing
- Tauri v2 Capabilities for FS access (no v1 allowlist)

IPC CONTRACT (call with @tauri-apps/api/core invoke)

- gauge_state() -> { potential: number; staged: number; freed: number } // bytes
- daily_candidates({ maxTotal?: number, bucket?: string }) -> Candidate[]
- archive_files({ fileIds: number[] }) -> { bytesFreed: number; count: number; batchId: string }
- get_review_items({ olderThanDays: number }) -> StagedFile[]
- delete_files({ fileIds: number[] }) -> { bytesFreed: number; count: number; batchId: string }
- undo_last() -> { ok: boolean }
- preview_folder({ paths: string[] }) -> { bytes: number; counts: Record<string, number> }
- register_watch_dirs({ paths: string[] }) -> void

TYPES
type Candidate = { id: number; path: string; displayName: string; sizeBytes: number; ageDays: number; reason: "screenshot"|"big_download"|"desktop"|"duplicate"; previewUrl?: string };
type StagedFile = { id: number; displayName: string; sizeBytes: number; archivedAt: number; reason?: string; path: string };

RULES

- Prefer smallest, safest path; clear errors; one-click Undo.
- No Node-only libs in UI runtime.
- Respect Tauri scopes (read/write only within configured paths).
- Loading skeletons, accessible labels, reduced-motion support.
- If IPC fails, show demo fallback data (tiny, obvious “Demo data” chip).
