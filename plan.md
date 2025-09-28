```
Goal: Connect UI to backend.

Phase 1 scope:
1) Folder registry stored in SQLite (`watched_roots`) with commands:
   - add_folder(path: string) -> validates dir, persists (id, path, created_at)
   - list_folders() -> [{ id, path, name, isAccessible }]
2) Folder browser for a selected watched folder:
   - list_dir(path: string) -> [{ name, path, kind: 'file'|'dir', size, modified }]
   - Pagination optional; start simple (non-recursive)
3) Context menu consistency and working "Open in …":
   - UI label: Windows -> “Open in File Explorer”; macOS -> “Open in Finder”; Linux -> “Open in File Manager”
   - open_in_system(path: string, reveal?: boolean) opens folder or reveals file
4) Events & state:
   - Nothing critical lives in memory; registry survives restart
   - Optional: refresh UI on changes (no watcher yet; manual refresh is fine)

Constraints:
- Tauri 2, Rust backend (rusqlite), React/TS frontend
- Keep FS IO in Rust (no webview FS)
- Provide proper error strings, never panic
```

---

# 🗃️ 1) DB migration for `watched_roots`

```
Add a SQLite migration for table watched_roots.

Fields:
- id INTEGER PRIMARY KEY
- path TEXT UNIQUE NOT NULL
- name TEXT NOT NULL        -- last segment (folder name)
- created_at INTEGER NOT NULL  -- unix ts
- disabled INTEGER DEFAULT 0   -- future use

Indexes:
- unique(path)

Also ensure files/actions tables exist (don’t modify their schema now).

Generate migration file at src-tauri/migrations/VXX__watched_roots.sql and update Rust DB init to run migrations at startup.
```

**Acceptance**

- App boots with DB present; re-running doesn’t error.
- In SQLite browser you can see `watched_roots`.

---

# 🧩 2) Backend state wiring (DB in State)

**Prompt**

```
Add a small DB wrapper that’s Send + Sync.

- Use rusqlite::Connection in a connection pool (r2d2_sqlite) or Arc<Mutex<Connection>> if we keep it single-threaded for now.
- Expose it as tauri::State<Db>.
- Provide helper: Db::execute / Db::query_map ergonomics.

File: src-tauri/src/db/mod.rs and src-tauri/src/db/sql.rs
Update main.rs to manage this State and pass to commands.
```

**Stub**

```rust
// src-tauri/src/db/mod.rs
use rusqlite::{Connection, params, Result as SqlResult};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
  pub fn new(conn: Connection) -> Self { Self(Arc::new(Mutex::new(conn))) }
  pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> { self.0.lock().unwrap() }
}
```

**Acceptance**

- Build runs; commands can access `State<Db>`.

---

# ➕ 3) Commands: add_folder / list_folders

```
Implement Tauri commands:

#[tauri::command]
async fn add_folder(path: String, db: State<Db>) -> Result<WatchedFolder, String>

Rules:
- Validate path exists AND is a directory.
- Normalize to absolute path; trim trailing separators.
- Derive name = last path segment.
- INSERT OR IGNORE into watched_roots; return the row (id, path, name).
- If duplicate path, return existing row (no error).

#[tauri::command]
async fn list_folders(db: State<Db>) -> Result<Vec<WatchedFolder>, String>

Type:
#[derive(Serialize)]
struct WatchedFolder { id: i64, path: String, name: String, is_accessible: bool }

is_accessible = std::fs::metadata(&path).is_ok()

Add both to invoke_handler in main.rs.
```

**Rust stub**

```rust
// src-tauri/src/commands/folders.rs
use crate::db::Db;
use serde::Serialize;
use tauri::State;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct WatchedFolder {
  pub id: i64,
  pub path: String,
  pub name: String,
  pub is_accessible: bool,
}

fn normalize_dir(p: &str) -> Result<PathBuf, String> {
  let pb = PathBuf::from(p);
  let abs = dunce::canonicalize(&pb).map_err(|_| "Path not found or not accessible")?;
  if !abs.is_dir() { return Err("Not a directory".into()); }
  Ok(abs)
}

#[tauri::command]
pub async fn add_folder(path: String, db: State<'_, Db>) -> Result<WatchedFolder, String> {
  let abs = normalize_dir(&path)?;
  let name = abs.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
  if name.is_empty() { return Err("Cannot add root drive".into()); }

  let path_str = abs.to_string_lossy().to_string();
  let ts = chrono::Utc::now().timestamp();

  {
    let conn = db.conn();
    conn.execute(
      "INSERT OR IGNORE INTO watched_roots(path, name, created_at) VALUES (?1, ?2, ?3)",
      rusqlite::params![path_str, name, ts],
    ).map_err(|e| e.to_string())?;
  }

  // read back row id
  let mut id = -1i64;
  {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id FROM watched_roots WHERE path = ?1").map_err(|e| e.to_string())?;
    id = stmt.query_row([&path_str], |row| row.get(0)).map_err(|e| e.to_string())?;
  }

  Ok(WatchedFolder { id, path: path_str.clone(), name, is_accessible: true })
}

#[tauri::command]
pub async fn list_folders(db: State<'_, Db>) -> Result<Vec<WatchedFolder>, String> {
  let conn = db.conn();
  let mut stmt = conn.prepare("SELECT id, path, name FROM watched_roots WHERE disabled = 0 ORDER BY created_at ASC")
    .map_err(|e| e.to_string())?;
  let rows = stmt.query_map([], |r| {
    let path: String = r.get(1)?;
    Ok(WatchedFolder {
      id: r.get(0)?, path: path.clone(), name: r.get(2)?,
      is_accessible: std::fs::metadata(&path).is_ok(),
    })
  }).map_err(|e| e.to_string())?;

  let mut out = Vec::new();
  for r in rows { out.push(r.map_err(|e| e.to_string())?); }
  Ok(out)
}
```

**Acceptance**

- Add same folder twice → returns existing, no crash.
- Restart app → list persists.

---

# 🗂️ 4) Command: list_dir (non-recursive folder browser)

```
Add command list_dir(root_path: String) -> Result<Vec<DirEntry>, String>.

- Validate: root_path must exist and be a directory.
- Read non-recursively.
- Return entries sorted: directories first, then files; then alpha.
- For each entry: name, path, kind ('dir'|'file'), size (files only), modified (unix ts).

Type:
#[derive(Serialize)] struct DirEntry { name: String, path: String, kind: String, size: u64, modified: i64 }
```

**Stub**

```rust
// src-tauri/src/commands/fs.rs
use serde::Serialize;
use chrono::Utc;

#[derive(Serialize)]
pub struct DirEntry {
  pub name: String,
  pub path: String,
  pub kind: String, // "dir" | "file"
  pub size: u64,
  pub modified: i64,
}

#[tauri::command]
pub async fn list_dir(root_path: String) -> Result<Vec<DirEntry>, String> {
  let p = std::path::Path::new(&root_path);
  if !p.is_dir() { return Err("Not a directory".into()); }

  let mut items = Vec::new();
  let read = std::fs::read_dir(p).map_err(|_| "Cannot read directory")?;
  for ent in read {
    if let Ok(ent) = ent {
      if let Ok(md) = ent.metadata() {
        let name = ent.file_name().to_string_lossy().to_string();
        let path = ent.path().to_string_lossy().to_string();
        let modified = md.modified().ok()
          .and_then(|t| t.elapsed().ok())
          .map(|e| (Utc::now() - chrono::Duration::from_std(e).unwrap()).timestamp())
          .unwrap_or(Utc::now().timestamp());

        let kind = if md.is_dir() { "dir" } else { "file" };
        let size = if md.is_file() { md.len() } else { 0 };
        items.push(DirEntry { name, path, kind: kind.into(), size, modified });
      }
    }
  }
  items.sort_by(|a,b| match (a.kind.as_str(), b.kind.as_str()) {
    ("dir","file") => std::cmp::Ordering::Less,
    ("file","dir") => std::cmp::Ordering::Greater,
    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
  });
  Ok(items)
}
```

**Acceptance**

- Clicking a watched folder in UI shows its contents reliably.
- Large folders (e.g., Downloads) return quickly (non-recursive).

---

# 🔗 5) Command: open_in_system (Explorer / Finder / File Manager)

```
Add cross-platform open_in_system(path: String, reveal: bool) -> Result<(), String>.

Rules:
- If reveal = true and path is file: reveal it in Explorer/Finder if possible (Windows: explorer /select, path; macOS: open -R path).
- Else open the folder/file with the OS default handler (Windows: explorer path; macOS: open path; Linux: xdg-open path).
- Validate path exists before spawning.
- Return user-friendly errors.

Use #[cfg(target_os = "...")] splits.
```

**Stub**

```rust
// src-tauri/src/commands/open.rs
#[tauri::command]
pub async fn open_in_system(path: String, reveal: bool) -> Result<(), String> {
  let p = std::path::Path::new(&path);
  if !p.exists() { return Err("Path does not exist".into()); }

  #[cfg(target_os = "windows")]
  {
    use std::process::Command;
    if reveal && p.is_file() {
      Command::new("explorer").args(["/select,", &path]).spawn().map_err(|e| e.to_string())?;
    } else {
      Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    return Ok(());
  }

  #[cfg(target_os = "macos")]
  {
    use std::process::Command;
    if reveal && p.is_file() {
      Command::new("open").args(["-R", &path]).spawn().map_err(|e| e.to_string())?;
    } else {
      Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    return Ok(());
  }

  #[cfg(target_os = "linux")]
  {
    use std::process::Command;
    Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    return Ok(());
  }
}
```

**Acceptance**

- From context menu, “Open in File Explorer/Finder” opens the right thing.
- “Reveal” works on files (optional now; keeps API ready for later).

---

# 🖥️ 6) Frontend wiring (React)

```
Add a WatchedFolders store + folder content panel.

- On mount of Home page: invoke("list_folders") → set state.
- “Add Folder” button: directory picker (plugin-dialog), then invoke("add_folder", { path }), then refresh list.
- On clicking a folder tile: invoke("list_dir", { rootPath: folder.path }) and render entries (icon, name, meta).
- Context menu label:
   - Use @tauri-apps/api/os to detect platform and display:
     Windows -> "Open in File Explorer"
     macOS -> "Open in Finder"
     Linux -> "Open in File Manager"
- Context menu action → invoke("open_in_system", { path: entry.path, reveal: entry.kind === 'file' })
- Ensure errors are toasts, not silent.
```

**JS stubs**

```ts
// src/lib/osLabel.ts
import { platform } from "@tauri-apps/api/os";
export async function openLabel() {
  const p = await platform();
  if (p === "windows") return "Open in File Explorer";
  if (p === "macos") return "Open in Finder";
  return "Open in File Manager";
}
```

```ts
// src/features/folders/useFolders.ts
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";

type WatchedFolder = {
  id: number;
  path: string;
  name: string;
  is_accessible: boolean;
};
type DirEntry = {
  name: string;
  path: string;
  kind: "dir" | "file";
  size: number;
  modified: number;
};

interface FolderState {
  folders: WatchedFolder[];
  entries: DirEntry[];
  activePath: string | null;
  loadFolders: () => Promise<void>;
  addFolder: () => Promise<void>;
  loadDir: (path: string) => Promise<void>;
  openInSystem: (path: string, reveal?: boolean) => Promise<void>;
}

export const useFolders = create<FolderState>((set, get) => ({
  folders: [],
  entries: [],
  activePath: null,
  loadFolders: async () => {
    const data = await invoke<WatchedFolder[]>("list_folders");
    set({ folders: data });
  },
  addFolder: async () => {
    const dir = await open({ directory: true, multiple: false });
    if (!dir || typeof dir !== "string") return;
    await invoke("add_folder", { path: dir });
    await get().loadFolders();
  },
  loadDir: async (path) => {
    const items = await invoke<DirEntry[]>("list_dir", { rootPath: path });
    set({ entries: items, activePath: path });
  },
  openInSystem: async (path, reveal = false) => {
    await invoke("open_in_system", { path, reveal });
  },
}));
```

**Acceptance**

- Add folder works; after adding, it appears and clicking it lists contents.
- Context menu labels match OS and **open** action works.

---

# 🧪 7) Minimal UI acceptance tests (manual)

- Add Desktop/Downloads → shows in left rail; persists after restart.
- Click “Downloads” → list loads fast; folders on top, then files; names look right.
- Right-click an entry → “Open in File Explorer/Finder” opens correct view.
- Failure cases:

  - Add same folder twice → no crash, duplicates not created.
  - Remove or rename watched folder externally → list shows `is_accessible=false` (style it grey/with warning).

---

# 🛡️ 8) Guardrails & polish

```
- In add_folder: reject system roots like C:\ or / (return clear error).
- In list_dir: wrap read_dir in error mapping; if permission denied, return error string “Permission denied” to UI.
- In UI: show empty state “No permission / not accessible”; add a “Open in system” button to help user grant access.
- Add unit tests for normalize_dir and for add_folder duplicate handling.
```

---

# 🔧 9) Arrange your “Current Backend Flow” into README sections

**Paste this into README.md**

```
## Backend Flow (current)

1) Folder registry
   - `add_folder(path)` validates and persists into `watched_roots`
   - `list_folders()` reads from DB
   - Nothing lives in memory → survives restarts

2) Scanning workflow
   - `start_scan(paths?)`: use given paths or stored roots; dedup; spawn worker
   - Worker walks with WalkDir, skips (.git, node_modules, .DS_Store, Thumbs.db)
   - Collects size + c/m/a timestamps, guesses MIME, upserts into `files`
   - Emits events every ~250 files (`scan://progress`, `scan://done`)
   - Duplicate pipeline: size groups → partial SHA1 (256 KB) → full SHA1 on conflicts

3) Candidate heuristics
   - `get_candidates()` ranks “screenshots”, “big downloads”, “old desktop”, “duplicates”
   - Uses hashes + timestamps from the scan

4) Persistence
   - `files(path, size_bytes, created_at, modified_at, accessed_at, partial_sha1, sha1, first_seen_at, last_seen_at, ...)`
   - `watched_roots(id, path, name, created_at, disabled)`
   - `actions`/`metrics` continue to power archive/delete flows

## Integration Plan (UI)

- State bootstrap: call `list_folders()` on launch; subscribe to `scan://progress` & `scan://done`; expose `scan_status`.
- Folder mgmt: “Add folder” uses native dir picker → `add_folder` → (optional) `start_scan`.
- Scanning UX: “Scan all” (no args) and “Scan this folder” (single path); show progress; disable destructive actions until complete.
- Data consumption: after `scan://done`, refresh `get_candidates()`; duplicates/unused widgets read from this.
- Notifications: poll `scan_status` on app focus; emit `scan://error` for surfaced failures (planned).
- Future hooks: incremental/resume scans; FS watcher (`notify`) to queue re-scans; detail commands like `list_duplicates`, `list_large_files`.
```

---

# 🧱 10) Known pitfalls

- **Windows long paths**: avoid building your own `\\?\` logic for now; just surface a friendly error if path is too long.
- **Explorer `/select,` comma**: keep comma after `/select,` exactly.
- **Permission errors**: don’t crash; return `"Permission denied"` strings to UI.
- **Blocking UI**: list_dir is non-recursive; keep scanning in the background worker.

---

## Next

- “Scan this folder” button on each watched folder.
- Progress chip in the sidebar (reads `scan_status`).
- Optional “Reveal in Archive” once archive ops land.

Plan Forward (remaining work)

Expand Folder Commands

Update add_folder to return WatchedFolder { id, path, name, is_accessible }, derive name from the last segment, and normalize paths consistently with the DB schema.
Extend list_folders to emit that same struct list (including id and accessibility check) so the UI can render status without extra IPC.
Add friendly error variants (system root rejection, permission issues) and surface them as strings.

Directory Browsing IPC

Implement list_dir(root_path) in Rust using std::fs::read_dir, skipping watchlist-only roots, returning { name, path, kind, size, modified } with size/mtime fetched via metadata.
Ensure errors (permission denied, missing folders) are mapped to clear messages and do not panic.
Add basic utility tests for the normalization and duplicate-handling helpers backing list_dir.

System Reveal Command

Introduce open_in_system(path, reveal) that dispatches to platform-specific shell commands (Explorer /select,, open, xdg-open), guarding against disallowed paths and returning descriptive errors.
Wire OS name detection so the frontend can label context-menu entries appropriately.

Scan Status Plumbing

Expose a lightweight scan_status shape tailored for UI polling (state, counts, current path, timestamps) and document event names (scan://progress, scan://done) for the front end.
Emit a dedicated scan://error event or enrich the done payload with an error list so toast notifications can be shown.

Frontend Store & Hooks

Build a Zustand (or context) store with methods loadFolders, addFolder, loadDir, openInSystem, and startScan, mapping to the new IPCs.
Subscribe to scan events in a React effect, updating progress indicators and refreshing candidates when scans finish.
Implement UI states: folders list with accessibility badges, directory pane with sorting/grouping, contextual menus with OS-specific labels, and an empty/error state panel.

UI Interactions & Feedback

Add “Scan all” and optional “Scan folder” buttons, disabling destructive actions while scans run.
Surface backend errors (e.g., permission denials) via toast/banner feedback and offer a quick “Open in system” action when access fails.
Verification & Docs

Write integration smoke tests (manual or Vitest with mocks) covering add/list folder, directory browsing, and open-in-system flows.
Update README/goal docs with the new command contracts and event semantics, plus a short QA checklist (add folder, browse entries, open in system, scan progress).
