Goal: Connect UI to backend.

Phase 1 scope:

1. Folder registry stored in SQLite (`watched_roots`) with commands:
   - add_folder(path: string) -> validates dir, persists (id, path, created_at)
   - list_folders() -> [{ id, path, name, isAccessible }]
2. Folder browser for a selected watched folder:
   - list_dir(path: string) -> [{ name, path, kind: 'file'|'dir', size, modified }]
   - Pagination optional; start simple (non-recursive)
3. Context menu consistency and working "Open in …":
   - UI label: Windows -> “Open in File Explorer”; macOS -> “Open in Finder”; Linux -> “Open in File Manager”
   - open_in_system(path: string, reveal?: boolean) opens folder or reveals file
4. Events & state:
   - Nothing critical lives in memory; registry survives restart
   - Optional: refresh UI on changes (no watcher yet; manual refresh is fine)

Constraints:

- Tauri 2, Rust backend (rusqlite), React/TS frontend
- Keep FS IO in Rust (no webview FS)
- Provide proper error strings, never panic
