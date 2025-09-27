use crate::db::{Database, DbPool};
use crate::gauge::{GaugeManager, GaugeState};
use crate::models::{ActionType, File, NewStagedFile, StagedFileRecord, WatchedRoot};
use crate::ops::{ArchiveManager, DeleteManager, UndoManager, UndoResult};
use crate::scanner::{self, ScanResult, Scanner};
use crate::scanner::watcher::{register_root, unregister_root};
use crate::selector::{scoring::Candidate, FileSelector};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tauri::State;
use walkdir::WalkDir;

// Command result types
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveOutcome {
    pub success: bool,
    pub files_processed: usize,
    pub total_bytes: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeleteOutcome {
    pub success: bool,
    pub files_processed: usize,
    pub total_bytes_freed: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub to_trash: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StageOutcome {
    pub success: bool,
    pub batch_id: Option<String>,
    pub staged_files: usize,
    pub total_bytes: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub expires_at: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct StageOptions {
    pub cooloff_days: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroupFile {
    pub id: i64,
    pub path: String,
    pub parent_dir: String,
    pub size_bytes: u64,
    pub last_seen_at: String,
    pub is_staged: bool,
    pub cooloff_until: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub total_size: u64,
    pub count: usize,
    pub files: Vec<DuplicateGroupFile>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StagedFile {
    pub record_id: i64,
    pub file_id: i64,
    pub path: String,
    pub parent_dir: String,
    pub size_bytes: u64,
    pub status: String,
    pub staged_at: String,
    pub expires_at: Option<String>,
    pub batch_id: Option<String>,
    pub note: Option<String>,
    pub cooloff_until: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchedFolder {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub is_accessible: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub modified: i64,
}

// Bucketed candidates API types
#[derive(Debug, Clone, serde::Serialize)]
pub struct BucketSummary {
    pub key: String,
    pub count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UiCandidate {
    pub id: i64,
    pub path: String,
    pub parent: String,
    pub size: u64,
    pub mime: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub accessed_at: Option<String>,
    pub partial_sha1: Option<String>,
    pub sha1: Option<String>,
    pub reason: String,
    pub group_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CandidatesResponse {
    pub by_bucket: std::collections::HashMap<String, Vec<UiCandidate>>,
    pub summaries: Vec<BucketSummary>,
    pub total_count: usize,
    pub paging: Paging,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Paging {
    pub limit: usize,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UndoBatchSummary {
    pub batch_id: String,
    pub action_type: String,
    pub file_count: usize,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub open_label: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UserPrefs {
    pub dry_run_default: bool,
    pub tidy_day: String,
    pub tidy_hour: u32,
    pub rolling_window_days: i64,
    pub max_candidates_per_day: usize,
    pub thumbnail_max_size: u32,
    pub auto_scan_enabled: bool,
    pub scan_interval_hours: u32,
    pub archive_age_threshold_days: u32,
    pub delete_age_threshold_days: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PartialUserPrefs {
    pub dry_run_default: Option<bool>,
    pub tidy_day: Option<String>,
    pub tidy_hour: Option<u32>,
    pub rolling_window_days: Option<i64>,
    pub max_candidates_per_day: Option<usize>,
    pub thumbnail_max_size: Option<u32>,
    pub auto_scan_enabled: Option<bool>,
    pub scan_interval_hours: Option<u32>,
    pub archive_age_threshold_days: Option<u32>,
    pub delete_age_threshold_days: Option<u32>,
}

/// Parameters for querying bucketed candidates
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetCandidatesBucketedParams {
    /// Maximum number of results to return
    pub limit: Option<usize>,

    /// Number of results to skip (for pagination)
    pub offset: Option<usize>,

    /// Minimum confidence score for including candidates
    pub min_confidence: Option<f64>,

    /// Maximum number of results to return per bucket
    pub max_results_per_bucket: Option<usize>,

    /// Whether to include archived files in results
    pub include_archived: Option<bool>,

    /// Whether to include deleted files in results
    pub include_deleted: Option<bool>,

    /// Optional path to scope the results to a specific directory
    #[serde(default)]
    pub root_path: Option<String>,

    /// Optional list of bucket types to include
    pub buckets: Option<Vec<String>>,

    /// Sorting criteria (e.g., "size_desc", "age_desc", "name_asc")
    pub sort: Option<String>,
}

// Error handling
#[derive(Debug)]
pub enum CommandError {
    Database(String),
    FileSystem(String),
    Validation(String),
    Permission(String),
    NotFound(String),
    Internal(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::Database(msg) => write!(f, "Database error: {}", msg),
            CommandError::FileSystem(msg) => write!(f, "File system error: {}", msg),
            CommandError::Validation(msg) => write!(f, "Validation error: {}", msg),
            CommandError::Permission(msg) => write!(f, "Permission error: {}", msg),
            CommandError::NotFound(msg) => write!(f, "Not found: {}", msg),
            CommandError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for CommandError {}

fn command_error_to_string(err: CommandError) -> String {
    err.to_string()
}

fn map_io_error(action: &str, path: &Path, err: std::io::Error) -> CommandError {
    match err.kind() {
        ErrorKind::NotFound => CommandError::NotFound(format!("{}: {}", action, path.display())),
        ErrorKind::PermissionDenied => {
            CommandError::Permission(format!("{}: {}", action, path.display()))
        }
        _ => CommandError::FileSystem(format!("Failed to {} {}: {}", action, path.display(), err)),
    }
}

fn sanitize_string(input: &str) -> String {
    let mut sanitized = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_control() {
            continue;
        }
        sanitized.push(ch);
        if sanitized.len() >= 1024 {
            break;
        }
    }
    sanitized
}

fn sanitize_note(note: Option<String>) -> Option<String> {
    note.map(|raw| {
        let mut sanitized = sanitize_string(&raw);
        if sanitized.len() > 256 {
            sanitized.truncate(256);
        }
        sanitized
    })
    .filter(|s| !s.is_empty())
}

fn normalize_directory_path(path: &Path) -> Result<PathBuf, CommandError> {
    let normalized = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    if normalized.is_dir() {
        Ok(normalized)
    } else {
        Err(CommandError::Validation(format!(
            "Path is not a directory: {}",
            path.display()
        )))
    }
}

fn normalize_existing_path(path: &Path) -> Result<PathBuf, CommandError> {
    if !path.exists() {
        return Err(CommandError::NotFound(format!(
            "Path not found: {}",
            path.display()
        )));
    }
    path.canonicalize()
        .or_else(|_| Ok(path.to_path_buf()))
        .map_err(|err| map_io_error("access path", path, err))
}

fn is_system_root(path: &Path) -> bool {
    path.parent().is_none()
}

fn watched_root_to_folder(root: WatchedRoot) -> WatchedFolder {
    WatchedFolder {
        id: root.id,
        path: root.path.clone(),
        name: Path::new(&root.path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| root.path.clone()),
        is_accessible: Path::new(&root.path).exists(),
    }
}

fn staged_payload(record: &StagedFileRecord, file: &File) -> StagedFile {
    let size = if file.size_bytes < 0 {
        0
    } else {
        file.size_bytes as u64
    };
    StagedFile {
        record_id: record.id,
        file_id: record.file_id,
        path: file.path.clone(),
        parent_dir: file.parent_dir.clone(),
        size_bytes: size,
        status: record.status.clone(),
        staged_at: record.staged_at.to_rfc3339(),
        expires_at: record.expires_at.map(|dt| dt.to_rfc3339()),
        batch_id: record.batch_id.clone(),
        note: record.note.clone(),
        cooloff_until: file.cooloff_until.map(|dt| dt.to_rfc3339()),
    }
}

fn ensure_within_watched(path: &Path, roots: &[WatchedRoot]) -> Result<(), CommandError> {
    if is_within_watched_roots(path, roots) {
        Ok(())
    } else {
        Err(CommandError::Permission(
            "Path must be within a watched folder".to_string(),
        ))
    }
}

fn validate_file_ids(file_ids: &[i64]) -> Result<(), CommandError> {
    if file_ids.is_empty() {
        return Err(CommandError::Validation("No file IDs provided".to_string()));
    }
    if file_ids.len() > 1000 {
        return Err(CommandError::Validation(
            "Too many files selected (max 1000)".to_string(),
        ));
    }
    if file_ids.iter().any(|&id| id <= 0) {
        return Err(CommandError::Validation("Invalid file ID".to_string()));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<PathBuf, CommandError> {
    let path_buf = PathBuf::from(path);
    if path_buf
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(CommandError::Validation(
            "Path traversal not allowed".to_string(),
        ));
    }

    // Be permissive like scan validation: allow any existing directory, but block system roots
    if path_buf.is_absolute() {
        if !path_buf.exists() {
            return Err(CommandError::NotFound(format!(
                "Path does not exist: {}",
                path
            )));
        }
        if is_system_root(&path_buf) {
            return Err(CommandError::Permission(
                "Watching the system root is not supported".to_string(),
            ));
        }
    }

    Ok(path_buf)
}

fn validate_scan_path(path: &str) -> Result<PathBuf, CommandError> {
    let path_buf = PathBuf::from(path);
    if path_buf
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(CommandError::Validation(
            "Path traversal not allowed".to_string(),
        ));
    }

    // For scan operations, be more permissive - allow any path that exists and is accessible
    if path_buf.is_absolute() {
        // Check if path exists and is accessible
        if !path_buf.exists() {
            return Err(CommandError::NotFound(format!(
                "Path does not exist: {}",
                path
            )));
        }

        // Additional security: ensure it's not a system directory
        if is_system_root(&path_buf) {
            return Err(CommandError::Permission(
                "Cannot scan system root".to_string(),
            ));
        }
    }

    Ok(path_buf)
}

fn open_path_with_system(path: &Path, reveal: bool) -> Result<(), CommandError> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let path_str = path
            .to_str()
            .ok_or_else(|| CommandError::Validation("Path contains invalid UTF-8".to_string()))?
            .replace('/', "\\");

        let status = if reveal {
            let arg = format!("/select,{}", path_str);
            Command::new("explorer").arg(arg).status()
        } else {
            let target = if path.is_dir() {
                path.to_path_buf()
            } else {
                path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| path.to_path_buf())
            };
            let target_str = target
                .to_str()
                .ok_or_else(|| CommandError::Validation("Path contains invalid UTF-8".to_string()))?
                .replace('/', "\\");
            Command::new("explorer").arg(target_str).status()
        };

        let status = status
            .map_err(|e| CommandError::FileSystem(format!("Failed to launch Explorer: {}", e)))?;

        if !status.success() {
            if status.code() == Some(1) {
                return Ok(());
            }
            return Err(CommandError::FileSystem(
                "Explorer returned an error".to_string(),
            ));
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let path_str = path
            .to_str()
            .ok_or_else(|| CommandError::Validation("Path contains invalid UTF-8".to_string()))?;

        let status = if reveal {
            Command::new("open").arg("-R").arg(path_str).status()
        } else {
            let target = if path.is_dir() {
                path.to_path_buf()
            } else {
                path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| path.to_path_buf())
            };
            let target_str = target.to_str().ok_or_else(|| {
                CommandError::Validation("Path contains invalid UTF-8".to_string())
            })?;
            Command::new("open").arg(target_str).status()
        };

        let status = status
            .map_err(|e| CommandError::FileSystem(format!("Failed to launch open: {}", e)))?;

        if !status.success() {
            return Err(CommandError::FileSystem(
                "open returned an error".to_string(),
            ));
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::process::Command;

        let target = if reveal && path.is_file() {
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path.to_path_buf())
        } else if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| path.to_path_buf())
        };

        let target_str = target
            .to_str()
            .ok_or_else(|| CommandError::Validation("Path contains invalid UTF-8".to_string()))?;

        let status = Command::new("xdg-open")
            .arg(target_str)
            .status()
            .map_err(|e| CommandError::FileSystem(format!("Failed to launch xdg-open: {}", e)))?;

        if !status.success() {
            return Err(CommandError::FileSystem(
                "xdg-open returned an error".to_string(),
            ));
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(CommandError::Internal(
        "Unsupported platform for open_in_system".to_string(),
    ))
}

fn canonicalize_or_clone(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}

fn path_within_root(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn is_within_watched_roots(path: &Path, roots: &[WatchedRoot]) -> bool {
    roots.iter().any(|root| {
        let root_path = canonicalize_or_clone(Path::new(&root.path));
        path_within_root(path, &root_path)
    })
}

fn list_directory_entries(dir: &Path) -> Result<Vec<DirectoryEntry>, CommandError> {
    let read_dir = fs::read_dir(dir).map_err(|err| map_io_error("open directory", dir, err))?;
    let mut entries = Vec::new();

    for entry_result in read_dir {
        let entry = entry_result.map_err(|err| map_io_error("read directory entry", dir, err))?;
        let entry_path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|err| map_io_error("inspect entry", &entry_path, err))?;

        let name = entry
            .file_name()
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string());
        let kind = if metadata.is_dir() { "dir" } else { "file" }.to_string();
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        entries.push(DirectoryEntry {
            name,
            path: entry_path.to_string_lossy().to_string(),
            kind,
            size,
            modified,
        });
    }

    entries.sort_by(|a, b| match (a.kind.as_str(), b.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

// Database state management
// AppState removed - using DbPool directly now

// Tauri Commands

#[tauri::command]
pub async fn add_folder(path: String, db: State<'_, DbPool>) -> Result<WatchedFolder, String> {
    let validated = validate_path(&path).map_err(|e| format!("ERR_VALIDATION: {}", e))?;
    let normalized = normalize_directory_path(&validated).map_err(command_error_to_string)?;

    if is_system_root(&normalized) {
        return Err("ERR_VALIDATION: Watching the system root is not supported".to_string());
    }

    let normalized_path = normalized.to_string_lossy().to_string();

    let db_clone = db.inner().clone();
    let path_for_db = normalized_path.clone();
    let root = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let id = db_instance
            .upsert_watched_root(&path_for_db)
            .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        db_instance
            .get_watched_root_by_id(id)
            .map_err(|e| format!("ERR_DATABASE: {}", e))?
            .ok_or_else(|| "ERR_DATABASE: Watched folder not found after insert".to_string())
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    let folder = watched_root_to_folder(root);
    if let Err(err) = register_root(folder.path.as_str()) {
        eprintln!("Failed to register watcher for {}: {}", folder.path, err);
    }
    Ok(folder)
}

#[tauri::command]
pub async fn pick_directory(window: tauri::Window) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let (sender, receiver) = tokio::sync::oneshot::channel();

    window.dialog().file().pick_folder(move |folder| {
        let selection = folder.map(|path| match path {
            FilePath::Path(p) => p.to_string_lossy().into_owned(),
            FilePath::Url(url) => url.to_string(),
        });
        let _ = sender.send(selection);
    });

    receiver
        .await
        .map_err(|e| format!("ERR_INTERNAL: failed to open dialog: {e}"))
}

#[tauri::command]
pub async fn list_folders(db: State<'_, DbPool>) -> Result<Vec<WatchedFolder>, String> {
    let db_clone = db.inner().clone();
    let folders = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        db_instance
            .list_watched_roots()
            .map_err(|e| format!("ERR_DATABASE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(folders.into_iter().map(watched_root_to_folder).collect())
}

#[tauri::command]
pub async fn remove_folder(id: i64, db: State<'_, DbPool>) -> Result<(), String> {
    if id <= 0 {
        return Err("ERR_VALIDATION: Invalid folder id".to_string());
    }

    let db_clone = db.inner().clone();
    let removed_path = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let root = db_instance
            .get_watched_root_by_id(id)
            .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        match root {
            Some(r) => {
                let path = r.path.clone();
                db_instance
                    .delete_watched_root(&r.path)
                    .map_err(|e| format!("ERR_DATABASE: {}", e))?;
                Ok(path)
            }
            None => Err("ERR_NOT_FOUND: Watched folder not found".to_string()),
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    if let Err(err) = unregister_root(removed_path.as_str()) {
        eprintln!("Failed to unregister watcher for {}: {}", removed_path, err);
    }

    Ok(())
}

#[tauri::command]
pub fn get_platform_info() -> PlatformInfo {
    #[cfg(target_os = "windows")]
    {
        return PlatformInfo {
            os: "windows".to_string(),
            open_label: "Open in File Explorer".to_string(),
        };
    }

    #[cfg(target_os = "macos")]
    {
        return PlatformInfo {
            os: "macos".to_string(),
            open_label: "Open in Finder".to_string(),
        };
    }

    #[cfg(target_os = "linux")]
    {
        return PlatformInfo {
            os: "linux".to_string(),
            open_label: "Open in File Manager".to_string(),
        };
    }

    #[allow(unreachable_code)]
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        open_label: "Open in File Manager".to_string(),
    }
}

#[tauri::command]
pub async fn list_dir(
    root_path: String,
    db: State<'_, DbPool>,
) -> Result<Vec<DirectoryEntry>, String> {
    if root_path.trim().is_empty() {
        return Err("ERR_VALIDATION: Path cannot be empty".to_string());
    }

    let normalized =
        normalize_directory_path(Path::new(&root_path)).map_err(command_error_to_string)?;
    let path_for_listing = normalized.clone();

    let db_clone = db.inner().clone();
    let watched_roots = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        db_instance
            .list_watched_roots()
            .map_err(|e| format!("ERR_DATABASE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    ensure_within_watched(&normalized, &watched_roots).map_err(command_error_to_string)?;

    let entries = tokio::task::spawn_blocking(move || {
        list_directory_entries(&path_for_listing).map_err(command_error_to_string)
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(entries)
}

#[tauri::command]
pub async fn open_in_system(
    path: String,
    reveal: Option<bool>,
    db: State<'_, DbPool>,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("ERR_VALIDATION: Path cannot be empty".to_string());
    }

    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let normalized =
            normalize_existing_path(Path::new(&path)).map_err(command_error_to_string)?;
        let metadata = fs::metadata(&normalized)
            .map_err(|err| map_io_error("access path", &normalized, err))
            .map_err(command_error_to_string)?;
        let check_path = if metadata.is_dir() {
            normalized.clone()
        } else {
            normalized
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| normalized.clone())
        };
        let is_file = metadata.is_file();

        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let roots = db_instance
            .list_watched_roots()
            .map_err(|e| format!("ERR_DATABASE: {}", e))?;

        ensure_within_watched(&check_path, &roots).map_err(command_error_to_string)?;

        let reveal_flag = reveal.unwrap_or(is_file);
        open_path_with_system(&normalized, reveal_flag).map_err(command_error_to_string)
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(())
}

#[tauri::command]
pub async fn start_scan(
    paths: Option<Vec<String>>,
    app: tauri::AppHandle,
    db: State<'_, DbPool>,
) -> Result<(), String> {
    let provided = paths.unwrap_or_default();
    let db_clone = db.inner().clone();
    let roots = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        if provided.is_empty() {
            db_instance
                .list_watched_paths()
                .map_err(|e| format!("ERR_DATABASE: {}", e))
        } else {
            Ok(provided)
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    if roots.is_empty() {
        return Err("ERR_VALIDATION: No scan roots configured".to_string());
    }

    let mut unique = HashSet::new();
    let mut sanitized = Vec::new();
    for root in roots {
        validate_scan_path(&root).map_err(|e| format!("ERR_VALIDATION: {}", e))?;
        if !Path::new(&root).is_dir() {
            return Err(format!("ERR_VALIDATION: Path is not a directory: {}", root));
        }
        let clean = sanitize_string(&root);
        if unique.insert(clean.clone()) {
            sanitized.push(clean);
        }
    }

    scanner::start_scan(app, db.inner().clone(), sanitized)
        .map_err(|e| format!("ERR_SCAN: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn rescan_all(app: tauri::AppHandle, db: State<'_, DbPool>) -> Result<(), String> {
    let db_clone = db.inner().clone();
    let roots = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        db_instance
            .list_watched_paths()
            .map_err(|e| format!("ERR_DATABASE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    if roots.is_empty() {
        return Err("ERR_VALIDATION: No scan roots configured".to_string());
    }

    scanner::start_scan(app, db.inner().clone(), roots).map_err(|e| format!("ERR_SCAN: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn rescan_folder(
    path: String,
    app: tauri::AppHandle,
    db: State<'_, DbPool>,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("ERR_VALIDATION: Path cannot be empty".to_string());
    }

    let normalized = normalize_directory_path(Path::new(&path)).map_err(command_error_to_string)?;
    let root = normalized.to_string_lossy().to_string();

    // Ensure it's one of the watched roots
    let db_clone = db.inner().clone();
    let watched = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        db_instance
            .list_watched_paths()
            .map_err(|e| format!("ERR_DATABASE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    if !watched
        .iter()
        .any(|p| canonicalize_or_clone(Path::new(p)) == canonicalize_or_clone(Path::new(&root)))
    {
        return Err("ERR_PERMISSION: Path is not a watched root".to_string());
    }

    scanner::start_scan(app, db.inner().clone(), vec![root])
        .map_err(|e| format!("ERR_SCAN: {e}"))?;
    Ok(())
}
#[tauri::command]
pub fn scan_status() -> Result<scanner::ScanStatusPayload, String> {
    Ok(scanner::current_status())
}

#[tauri::command]
pub async fn get_candidates(
    max_total: usize,
    db: State<'_, DbPool>,
) -> Result<Vec<Candidate>, String> {
    daily_candidates(max_total, db).await
}

fn normalize_bucket_key(reason: &str) -> String {
    let lower = reason.to_lowercase();
    match lower.as_str() {
        "screenshots" => "screenshot".to_string(),
        "big downloads" => "big_download".to_string(),
        "old desktop" => "old_desktop".to_string(),
        "executable" | "executables" => "executable".to_string(),
        "duplicates" => "duplicate".to_string(),
        other => other.replace(' ', "_"),
    }
}

pub(crate) fn filter_candidates_by_root_path(
    candidates: &mut Vec<Candidate>,
    root_path: &str,
    errors: &mut Vec<String>,
) {
    let root_path_buf = PathBuf::from(root_path);
    let root_path = if let Ok(canonical) = root_path_buf.canonicalize() {
        canonical
    } else {
        errors.push(format!(
            "Warning: Could not canonicalize path: {}",
            root_path_buf.display()
        ));
        root_path_buf
    };

    let root_path_str = root_path.to_string_lossy().to_string();

    candidates.retain(|candidate| {
        let candidate_path = Path::new(&candidate.path);
        if let Ok(canon_candidate) = candidate_path.canonicalize() {
            let canon_str = canon_candidate.to_string_lossy().to_string();
            canon_str.starts_with(&root_path_str)
        } else {
            candidate.path.starts_with(&root_path_str)
        }
    });
}

#[tauri::command]
pub async fn get_candidates_bucketed(
    params: Option<GetCandidatesBucketedParams>,
    db: State<'_, DbPool>,
) -> Result<CandidatesResponse, String> {
    let params = params.unwrap_or(GetCandidatesBucketedParams {
        root_path: None,
        buckets: None,
        limit: Some(100),
        offset: Some(0),
        sort: Some("size_desc".to_string()),
        min_confidence: None,
        max_results_per_bucket: None,
        include_archived: None,
        include_deleted: None,
    });

    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);
    if limit == 0 {
        return Err("ERR_VALIDATION: limit must be > 0".to_string());
    }

    let db_clone = db.inner().clone();
    // If root_path is provided, pull a larger pool to avoid filtering away all results
    let fetch_size = if params.root_path.is_some() {
        (limit + offset).saturating_mul(50).min(10_000)
    } else {
        limit + offset
    };
    let (mut candidates, mut errors) = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let selector = FileSelector::new();
        let db_instance = Database::new(conn);
        let mut items = selector
            .daily_candidates(Some(fetch_size), &db_instance)
            .map_err(|e| format!("ERR_SELECTOR: {}", e))?;
        Ok::<(Vec<Candidate>, Vec<String>), String>((items.drain(..).collect(), Vec::new()))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    // Filter by root path if provided
    if let Some(root_path) = params.root_path.as_deref() {
        filter_candidates_by_root_path(&mut candidates, root_path, &mut errors);
    }

    // Filter by requested buckets if provided
    let requested_buckets: std::collections::HashSet<String> = params
        .buckets
        .as_ref()
        .map(|buckets| buckets.iter().map(|s| normalize_bucket_key(s)).collect())
        .unwrap_or_default();

    if !requested_buckets.is_empty() {
        candidates.retain(|c| requested_buckets.contains(&normalize_bucket_key(&c.reason)));
    }

    // Sort
    match params.sort.as_deref() {
        Some("size_desc") => candidates.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        Some("age_desc") => candidates.sort_by(|a, b| {
            b.age_days
                .partial_cmp(&a.age_days)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        Some("name_asc") => {
            candidates.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
        }
        _ => {}
    }

    // Recompute total_count AFTER filtering and sorting
    let mut total_count = candidates.len();
    let slice_end = (offset + limit).min(total_count);
    let paged = if offset < total_count {
        candidates[offset..slice_end].to_vec()
    } else {
        Vec::new()
    };
    let has_more = slice_end < total_count;

    let mut by_bucket: std::collections::HashMap<String, Vec<UiCandidate>> =
        std::collections::HashMap::new();
    let mut summaries_acc: std::collections::HashMap<String, (usize, u64)> =
        std::collections::HashMap::new();

    for c in paged {
        let key = normalize_bucket_key(&c.reason);
        let entry = UiCandidate {
            id: c.file_id,
            path: c.path.clone(),
            parent: c.parent_dir.clone(),
            size: c.size_bytes,
            mime: None,
            created_at: None,
            modified_at: None,
            accessed_at: None,
            partial_sha1: None,
            sha1: None,
            reason: key.clone(),
            group_key: None,
        };
        by_bucket.entry(key.clone()).or_default().push(entry);
        let e = summaries_acc.entry(key).or_insert((0, 0));
        e.0 += 1;
        e.1 += c.size_bytes;
    }

    // Fallback: if we have no candidates yet (e.g., first run, scan not completed),
    // surface a shallow pass of obvious executables and old desktop/download items
    // Skip fallback if we have a specific root_path and buckets filter
    if total_count == 0 && params.root_path.is_none() {
        // Use watched roots for fallback
        let db_clone = db.inner().clone();
        let roots = {
            tokio::task::spawn_blocking(move || {
                let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
                let db_instance = Database::new(conn);
                db_instance
                    .list_watched_paths()
                    .map_err(|e| format!("ERR_DATABASE: {}", e))
            })
            .await
            .map_err(|e| format!("join error: {e}"))??
        };

        let now = std::time::SystemTime::now();
        let thirty_days = std::time::Duration::from_secs(30 * 24 * 3600);

        for root in roots {
            let walker = WalkDir::new(&root).max_depth(2).into_iter();
            for entry in walker.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let path_str = path.to_string_lossy().to_string();
                let name_lower = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let parent = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| root.clone());
                let meta = match std::fs::metadata(path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let size = meta.len();
                let modified = meta.modified().ok();
                let is_old = modified
                    .and_then(|m| now.duration_since(m).ok())
                    .map(|d| d >= thirty_days)
                    .unwrap_or(false);

                let parent_lower = parent.to_lowercase();
                let in_downloads = parent_lower.contains("downloads");
                let in_desktop = parent_lower.contains("desktop");

                let mut bucket: Option<&str> = None;
                if name_lower.ends_with(".exe") {
                    if in_downloads || is_old {
                        bucket = Some("executable");
                    }
                } else if in_downloads && is_old {
                    bucket = Some("big_download");
                } else if in_desktop && is_old {
                    bucket = Some("old_desktop");
                }

                if let Some(key) = bucket {
                    let entry = UiCandidate {
                        id: 0,
                        path: path_str.clone(),
                        parent: parent.clone(),
                        size,
                        mime: None,
                        created_at: None,
                        modified_at: None,
                        accessed_at: None,
                        partial_sha1: None,
                        sha1: None,
                        reason: key.to_string(),
                        group_key: None,
                    };
                    by_bucket.entry(key.to_string()).or_default().push(entry);
                    let e = summaries_acc.entry(key.to_string()).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += size;
                    total_count += 1;
                }
            }
        }
    }

    let summaries = summaries_acc
        .into_iter()
        .map(|(k, (count, bytes))| BucketSummary {
            key: k,
            count,
            total_bytes: bytes,
        })
        .collect::<Vec<_>>();

    Ok(CandidatesResponse {
        by_bucket,
        summaries,
        total_count,
        paging: Paging {
            limit,
            offset,
            has_more: has_more,
        },
        errors,
    })
}

#[tauri::command]
pub async fn scan_roots(
    roots: Vec<String>,
    app: tauri::AppHandle,
    db: State<'_, DbPool>,
) -> Result<ScanResult, String> {
    println!("scan_roots called with roots: {:?}", roots);

    if roots.is_empty() {
        return Err("ERR_VALIDATION: No scan roots provided".to_string());
    }

    if roots.len() > 10 {
        return Err("ERR_VALIDATION: Too many scan roots (max 10)".to_string());
    }

    let mut unique = HashSet::new();
    let mut sanitized_roots = Vec::new();
    for root in &roots {
        validate_scan_path(root).map_err(|e| format!("ERR_VALIDATION: {}", e))?;
        if !Path::new(root).is_dir() {
            return Err(format!("ERR_VALIDATION: Path is not a directory: {}", root));
        }
        let clean = sanitize_string(root);
        if unique.insert(clean.clone()) {
            sanitized_roots.push(clean);
        }
    }

    let db_clone = db.inner().clone();
    let app_handle = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let mut scanner = Scanner::new();
        scanner
            .run_scan(&app_handle, sanitized_roots, &db_instance)
            .map_err(|e| format!("ERR_SCAN: {e}"))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(result)
}

#[tauri::command]
pub async fn daily_candidates(
    max_total: usize,
    db: State<'_, DbPool>,
) -> Result<Vec<Candidate>, String> {
    println!("daily_candidates called with max_total: {}", max_total);

    // Validate input
    if max_total == 0 {
        return Err("ERR_VALIDATION: max_total must be greater than 0".to_string());
    }

    if max_total > 1000 {
        return Err("ERR_VALIDATION: max_total too large (max 1000)".to_string());
    }

    // Get candidates using spawn_blocking for database operations
    let db_clone = db.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let selector = FileSelector::new();
        let db_instance = Database::new(conn);
        selector
            .daily_candidates(Some(max_total), &db_instance)
            .map_err(|e| format!("ERR_SELECTOR: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(result)
}

#[tauri::command]
pub async fn gauge_state(db: State<'_, DbPool>) -> Result<GaugeState, String> {
    println!("gauge_state called");
    let db_clone = db.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let gauge_manager = GaugeManager::new();
        let db_instance = Database::new(conn);
        gauge_manager
            .gauge_state(&db_instance)
            .map_err(|e| format!("ERR_GAUGE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(result)
}

#[tauri::command]
pub async fn list_staged(
    statuses: Option<Vec<String>>,
    db: State<'_, DbPool>,
) -> Result<Vec<StagedFile>, String> {
    let status_filter = statuses.map(|items| {
        items
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
    });
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let pairs = match status_filter {
            Some(filters) => db_instance
                .list_staged_with_files(Some(filters.as_slice()))
                .map_err(|e| format!("ERR_DATABASE: {e}"))?,
            None => db_instance
                .list_staged_with_files(None)
                .map_err(|e| format!("ERR_DATABASE: {e}"))?,
        };
        let mut results = Vec::with_capacity(pairs.len());
        for (record, file) in pairs {
            results.push(staged_payload(&record, &file));
        }
        Ok(results)
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn stage_files(
    file_ids: Vec<i64>,
    options: Option<StageOptions>,
    db: State<'_, DbPool>,
) -> Result<StageOutcome, String> {
    validate_file_ids(&file_ids).map_err(|e| format!("ERR_VALIDATION: {e}"))?;
    if file_ids.is_empty() {
        return Err("ERR_VALIDATION: No file IDs provided".to_string());
    }

    let mut opts = options.unwrap_or_default();
    let mut cooloff_days = opts.cooloff_days.take().unwrap_or(7);
    if cooloff_days < 0 {
        cooloff_days = 0;
    }
    if cooloff_days > 30 {
        cooloff_days = 30;
    }
    let note = sanitize_note(opts.note.take());
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let mut db_instance = Database::new(conn);
        let mut archive_manager = ArchiveManager::new();

        let mut unique_ids = HashSet::new();
        let mut file_paths = Vec::new();
        for file_id in &file_ids {
            if !unique_ids.insert(*file_id) {
                continue;
            }
            let file = db_instance
                .get_file_by_id(*file_id)
                .map_err(|e| format!("ERR_DATABASE: {e}"))?
                .ok_or_else(|| format!("ERR_NOT_FOUND: File with ID {} not found", file_id))?;
            if file.is_deleted {
                return Err(format!(
                    "ERR_VALIDATION: File with ID {} has been deleted",
                    file_id
                ));
            }
            let file_path = Path::new(&file.path);
            if !file_path.exists() {
                return Err(format!(
                    "ERR_NOT_FOUND: File with ID {} not found on disk",
                    file_id
                ));
            }
            file_paths.push(file.path.clone());
        }

        if file_paths.is_empty() {
            return Err("ERR_VALIDATION: No unique file paths to stage".to_string());
        }

        let archive_result = archive_manager
            .archive_files(file_paths, &db_instance)
            .map_err(|e| format!("ERR_ARCHIVE: {e}"))?;

        let actions = db_instance
            .get_actions_by_batch_id(&archive_result.batch_id)
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;

        let archived_actions: Vec<_> = actions
            .into_iter()
            .filter(|action| action.action == ActionType::Archive)
            .collect();

        let expires_at_dt = if cooloff_days > 0 {
            Some(Utc::now() + Duration::days(cooloff_days))
        } else {
            None
        };

        let mut staged_entries = Vec::new();
        for action in &archived_actions {
            let batch_id = action
                .batch_id
                .clone()
                .or_else(|| Some(archive_result.batch_id.clone()));
            staged_entries.push(NewStagedFile {
                file_id: action.file_id,
                staged_at: action.created_at,
                expires_at: expires_at_dt.clone(),
                batch_id,
                status: "staged".to_string(),
                note: note.clone(),
            });
        }

        if !staged_entries.is_empty() {
            db_instance
                .stage_files(&staged_entries)
                .map_err(|e| format!("ERR_DATABASE: {e}"))?;
        }

        let outcome = StageOutcome {
            success: archive_result.errors.is_empty(),
            batch_id: if staged_entries.is_empty() {
                None
            } else {
                Some(archive_result.batch_id.clone())
            },
            staged_files: staged_entries.len(),
            total_bytes: archive_result.total_bytes,
            duration_ms: archive_result.duration_ms,
            errors: archive_result.errors,
            expires_at: expires_at_dt.map(|dt| dt.to_rfc3339()),
            note,
        };

        Ok(outcome)
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn restore_staged(batch_id: String, db: State<'_, DbPool>) -> Result<UndoResult, String> {
    if batch_id.trim().is_empty() {
        return Err("ERR_VALIDATION: batch_id cannot be empty".to_string());
    }

    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);

        let actions = db_instance
            .get_actions_by_batch_id(&batch_id)
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;
        let archived_ids: Vec<i64> = actions
            .iter()
            .filter(|action| action.action == ActionType::Archive)
            .map(|action| action.file_id)
            .collect();

        if archived_ids.is_empty() {
            return Err(format!(
                "ERR_NOT_FOUND: No archived files associated with batch {batch_id}"
            ));
        }

        let mut undo_manager = UndoManager::new();
        let result = undo_manager
            .undo_batch(&batch_id, &db_instance)
            .map_err(|e| format!("ERR_UNDO: {e}"))?;

        db_instance
            .update_staged_status(&archived_ids, "restored")
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;
        db_instance
            .mark_files_unstaged(&archived_ids)
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;

        Ok(result)
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn empty_staged(
    file_ids: Vec<i64>,
    to_trash: bool,
    db: State<'_, DbPool>,
) -> Result<DeleteOutcome, String> {
    validate_file_ids(&file_ids).map_err(|e| format!("ERR_VALIDATION: {e}"))?;
    if file_ids.is_empty() {
        return Err("ERR_VALIDATION: No file IDs provided".to_string());
    }

    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);

        let mut file_paths = Vec::new();
        for file_id in &file_ids {
            let file = db_instance
                .get_file_by_id(*file_id)
                .map_err(|e| format!("ERR_DATABASE: {e}"))?
                .ok_or_else(|| format!("ERR_NOT_FOUND: File with ID {} not found", file_id))?;
            validate_path(&file.path).map_err(|e| format!("ERR_VALIDATION: {e}"))?;
            file_paths.push(file.path);
        }

        let mut delete_manager = DeleteManager::new();
        delete_manager.set_use_trash(to_trash);
        let delete_result = delete_manager
            .delete_files(file_paths, &db_instance)
            .map_err(|e| format!("ERR_DELETE: {e}"))?;

        db_instance
            .update_staged_status(&file_ids, "emptied")
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;
        db_instance
            .mark_files_unstaged(&file_ids)
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;

        Ok(DeleteOutcome {
            success: delete_result.errors.is_empty(),
            files_processed: delete_result.files_deleted,
            total_bytes_freed: delete_result.total_bytes_freed,
            duration_ms: delete_result.duration_ms,
            errors: delete_result.errors,
            to_trash,
        })
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn get_duplicate_groups(
    limit: Option<usize>,
    db: State<'_, DbPool>,
) -> Result<Vec<DuplicateGroup>, String> {
    let fetch_limit = limit.unwrap_or(20).min(200);
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let groups = db_instance
            .duplicate_groups(Some(fetch_limit))
            .map_err(|e| format!("ERR_DATABASE: {e}"))?;
        let mut response = Vec::with_capacity(groups.len());
        for (hash, files) in groups {
            let mut total_size = 0u64;
            let mut group_files = Vec::with_capacity(files.len());
            for file in files {
                let file_id = file.id.unwrap_or(0);
                let size = if file.size_bytes < 0 {
                    0
                } else {
                    file.size_bytes as u64
                };
                total_size = total_size.saturating_add(size);
                group_files.push(DuplicateGroupFile {
                    id: file_id,
                    path: file.path.clone(),
                    parent_dir: file.parent_dir.clone(),
                    size_bytes: size,
                    last_seen_at: file.last_seen_at.to_rfc3339(),
                    is_staged: file.is_staged,
                    cooloff_until: file.cooloff_until.map(|dt| dt.to_rfc3339()),
                });
            }
            response.push(DuplicateGroup {
                hash,
                total_size,
                count: group_files.len(),
                files: group_files,
            });
        }
        Ok(response)
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn archive_files(
    file_ids: Vec<i64>,
    db: State<'_, DbPool>,
) -> Result<ArchiveOutcome, String> {
    // Validate input
    validate_file_ids(&file_ids).map_err(|e| format!("ERR_VALIDATION: {}", e))?;

    // Perform archive operation using spawn_blocking for database operations
    let db_clone = db.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);

        // Get file paths from database
        let mut file_paths = Vec::new();
        for file_id in &file_ids {
            match db_instance.get_file_by_id(*file_id) {
                Ok(Some(file)) => {
                    validate_path(&file.path).map_err(|e| format!("ERR_VALIDATION: {}", e))?;
                    file_paths.push(file.path);
                }
                Ok(None) => {
                    return Err(format!("ERR_NOT_FOUND: File with ID {} not found", file_id));
                }
                Err(e) => {
                    return Err(format!("ERR_DATABASE: {}", e));
                }
            }
        }

        // Perform archive operation
        let mut archive_manager = ArchiveManager::new();
        archive_manager
            .archive_files(file_paths, &db_instance)
            .map_err(|e| format!("ERR_ARCHIVE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(ArchiveOutcome {
        success: result.errors.is_empty(),
        files_processed: result.files_archived,
        total_bytes: result.total_bytes,
        duration_ms: result.duration_ms,
        errors: result.errors,
        dry_run: false, // TODO: Get from user preferences
    })
}

#[tauri::command]
pub async fn delete_files(
    file_ids: Vec<i64>,
    to_trash: bool,
    db: State<'_, DbPool>,
) -> Result<DeleteOutcome, String> {
    // Validate input
    validate_file_ids(&file_ids).map_err(|e| format!("ERR_VALIDATION: {}", e))?;

    // Perform delete operation using spawn_blocking for database operations
    let db_clone = db.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);

        // Get file paths from database
        let mut file_paths = Vec::new();
        for file_id in &file_ids {
            match db_instance.get_file_by_id(*file_id) {
                Ok(Some(file)) => {
                    validate_path(&file.path).map_err(|e| format!("ERR_VALIDATION: {}", e))?;
                    file_paths.push(file.path);
                }
                Ok(None) => {
                    return Err(format!("ERR_NOT_FOUND: File with ID {} not found", file_id));
                }
                Err(e) => {
                    return Err(format!("ERR_DATABASE: {}", e));
                }
            }
        }

        // Perform delete operation
        let mut delete_manager = DeleteManager::new();
        delete_manager.set_use_trash(to_trash);

        delete_manager
            .delete_files(file_paths, &db_instance)
            .map_err(|e| format!("ERR_DELETE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(DeleteOutcome {
        success: result.errors.is_empty(),
        files_processed: result.files_deleted,
        total_bytes_freed: result.total_bytes_freed,
        duration_ms: result.duration_ms,
        errors: result.errors,
        to_trash,
    })
}

#[tauri::command]
pub async fn undo_last(db: State<'_, DbPool>) -> Result<UndoResult, String> {
    let db_clone = db.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let mut undo_manager = UndoManager::new();
        undo_manager
            .undo_last(&db_instance)
            .map_err(|e| format!("ERR_UNDO: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(result)
}

#[tauri::command]
pub async fn list_undoable_batches(db: State<'_, DbPool>) -> Result<Vec<UndoBatchSummary>, String> {
    let db_clone = db.inner().clone();
    let batches = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let undo = UndoManager::new();
        undo.get_undoable_batches(&db_instance)
            .map_err(|e| format!("ERR_UNDO: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    let summaries = batches
        .into_iter()
        .map(|b| UndoBatchSummary {
            batch_id: b.batch_id,
            action_type: b.action_type.to_string(),
            file_count: b.file_count,
            created_at: b.created_at.timestamp(),
        })
        .collect();

    Ok(summaries)
}

#[tauri::command]
pub async fn undo_batch(batch_id: String, db: State<'_, DbPool>) -> Result<UndoResult, String> {
    if batch_id.trim().is_empty() {
        return Err("ERR_VALIDATION: batch_id cannot be empty".to_string());
    }

    let db_clone = db.inner().clone();
    let target = batch_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        let mut undo_manager = UndoManager::new();
        undo_manager
            .undo_batch(&target, &db_instance)
            .map_err(|e| format!("ERR_UNDO: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(result)
}

#[tauri::command]
pub async fn get_review_items(
    min_age_days: u32,
    db: State<'_, DbPool>,
) -> Result<Vec<StagedFile>, String> {
    // Validate input
    if min_age_days > 365 {
        return Err("ERR_VALIDATION: min_age_days too large (max 365)".to_string());
    }

    // Get staged files (archived but not deleted)
    let cutoff_date = Utc::now() - Duration::days(min_age_days as i64);

    // This would query the database for staged files
    // For now, return empty vector as placeholder
    Ok(Vec::new())
}

#[tauri::command]
pub async fn get_thumbnail(
    file_id: i64,
    max_px: u32,
    db: State<'_, DbPool>,
) -> Result<String, String> {
    // Validate input
    if file_id <= 0 {
        return Err("ERR_VALIDATION: Invalid file ID".to_string());
    }

    if max_px == 0 || max_px > 2048 {
        return Err("ERR_VALIDATION: Invalid thumbnail size (1-2048px)".to_string());
    }

    // Get file from database using spawn_blocking
    let db_clone = db.inner().clone();
    let file = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        match db_instance.get_file_by_id(file_id) {
            Ok(Some(file)) => Ok(file),
            Ok(None) => Err(format!("ERR_NOT_FOUND: File with ID {} not found", file_id)),
            Err(e) => Err(format!("ERR_DATABASE: {}", e)),
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    // Validate file path
    validate_path(&file.path).map_err(|e| format!("ERR_VALIDATION: {}", e))?;

    // Check if file exists
    if !Path::new(&file.path).exists() {
        return Err("ERR_NOT_FOUND: File does not exist on disk".to_string());
    }

    // Generate thumbnail (placeholder implementation)
    // In a real implementation, this would:
    // 1. Check if thumbnail already exists in cache
    // 2. Generate thumbnail if needed
    // 3. Return base64 encoded thumbnail or file path

    Ok("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string())
}

#[tauri::command]
pub async fn get_prefs(db: State<'_, DbPool>) -> Result<UserPrefs, String> {
    // Get preferences from database using spawn_blocking
    let db_clone = db.inner().clone();
    let prefs = tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);
        db_instance
            .get_all_preferences()
            .map_err(|e| format!("ERR_DATABASE: {}", e))
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    // Convert to UserPrefs struct
    Ok(UserPrefs {
        dry_run_default: prefs
            .get("dry_run_default")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true),
        tidy_day: prefs
            .get("tidy_day")
            .cloned()
            .unwrap_or_else(|| "Fri".to_string()),
        tidy_hour: prefs
            .get("tidy_hour")
            .and_then(|v| v.parse().ok())
            .unwrap_or(17),
        rolling_window_days: prefs
            .get("rolling_window_days")
            .and_then(|v| v.parse().ok())
            .unwrap_or(7),
        max_candidates_per_day: prefs
            .get("max_candidates_per_day")
            .and_then(|v| v.parse().ok())
            .unwrap_or(12),
        thumbnail_max_size: prefs
            .get("thumbnail_max_size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(256),
        auto_scan_enabled: prefs
            .get("auto_scan_enabled")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false),
        scan_interval_hours: prefs
            .get("scan_interval_hours")
            .and_then(|v| v.parse().ok())
            .unwrap_or(24),
        archive_age_threshold_days: prefs
            .get("archive_age_threshold_days")
            .and_then(|v| v.parse().ok())
            .unwrap_or(7),
        delete_age_threshold_days: prefs
            .get("delete_age_threshold_days")
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    })
}

#[tauri::command]
pub async fn set_prefs(prefs: PartialUserPrefs, db: State<'_, DbPool>) -> Result<(), String> {
    // Validate input
    if let Some(tidy_hour) = prefs.tidy_hour {
        if tidy_hour > 23 {
            return Err("ERR_VALIDATION: tidy_hour must be 0-23".to_string());
        }
    }

    if let Some(rolling_window_days) = prefs.rolling_window_days {
        if rolling_window_days <= 0 || rolling_window_days > 365 {
            return Err("ERR_VALIDATION: rolling_window_days must be 1-365".to_string());
        }
    }

    if let Some(max_candidates_per_day) = prefs.max_candidates_per_day {
        if max_candidates_per_day == 0 || max_candidates_per_day > 1000 {
            return Err("ERR_VALIDATION: max_candidates_per_day must be 1-1000".to_string());
        }
    }

    if let Some(thumbnail_max_size) = prefs.thumbnail_max_size {
        if thumbnail_max_size == 0 || thumbnail_max_size > 2048 {
            return Err("ERR_VALIDATION: thumbnail_max_size must be 1-2048".to_string());
        }
    }

    if let Some(scan_interval_hours) = prefs.scan_interval_hours {
        if scan_interval_hours == 0 || scan_interval_hours > 168 {
            return Err("ERR_VALIDATION: scan_interval_hours must be 1-168".to_string());
        }
    }

    if let Some(archive_age_threshold_days) = prefs.archive_age_threshold_days {
        if archive_age_threshold_days > 365 {
            return Err("ERR_VALIDATION: archive_age_threshold_days must be 0-365".to_string());
        }
    }

    if let Some(delete_age_threshold_days) = prefs.delete_age_threshold_days {
        if delete_age_threshold_days > 365 {
            return Err("ERR_VALIDATION: delete_age_threshold_days must be 0-365".to_string());
        }
    }

    // Set preferences in database using spawn_blocking
    let db_clone = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db_clone.get().map_err(|e| format!("db pool: {e}"))?;
        let db_instance = Database::new(conn);

        if let Some(dry_run_default) = prefs.dry_run_default {
            db_instance
                .set_preference("dry_run_default", &dry_run_default.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(tidy_day) = prefs.tidy_day {
            let sanitized_day = sanitize_string(&tidy_day);
            db_instance
                .set_preference("tidy_day", &sanitized_day)
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(tidy_hour) = prefs.tidy_hour {
            db_instance
                .set_preference("tidy_hour", &tidy_hour.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(rolling_window_days) = prefs.rolling_window_days {
            db_instance
                .set_preference("rolling_window_days", &rolling_window_days.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(max_candidates_per_day) = prefs.max_candidates_per_day {
            db_instance
                .set_preference(
                    "max_candidates_per_day",
                    &max_candidates_per_day.to_string(),
                )
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(thumbnail_max_size) = prefs.thumbnail_max_size {
            db_instance
                .set_preference("thumbnail_max_size", &thumbnail_max_size.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(auto_scan_enabled) = prefs.auto_scan_enabled {
            db_instance
                .set_preference("auto_scan_enabled", &auto_scan_enabled.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(scan_interval_hours) = prefs.scan_interval_hours {
            db_instance
                .set_preference("scan_interval_hours", &scan_interval_hours.to_string())
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(archive_age_threshold_days) = prefs.archive_age_threshold_days {
            db_instance
                .set_preference(
                    "archive_age_threshold_days",
                    &archive_age_threshold_days.to_string(),
                )
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        if let Some(delete_age_threshold_days) = prefs.delete_age_threshold_days {
            db_instance
                .set_preference(
                    "delete_age_threshold_days",
                    &delete_age_threshold_days.to_string(),
                )
                .map_err(|e| format!("ERR_DATABASE: {}", e))?;
        }

        Ok::<_, String>(())
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(())
}

// Helper function to get database path
pub fn get_db_path() -> Result<PathBuf, CommandError> {
    let app_data_dir = dirs::data_dir()
        .ok_or_else(|| CommandError::Internal("Failed to get app data dir".to_string()))?;
    let db_dir = app_data_dir.join("white-space");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&db_dir)
        .map_err(|e| CommandError::FileSystem(format!("Failed to create db directory: {}", e)))?;

    Ok(db_dir.join("database.db"))
}

#[cfg(test)]
mod tests;
