pub mod active_project;
pub mod file_walker;
pub mod watcher;
mod hash;

use self::active_project::{ActiveProjectDetector, DevRepo};
use self::file_walker::FileWalker;
use self::hash::{hash_first_n, hash_full};
use crate::db::{Database, DbPool};
use crate::models::{NewFile, NewMetric};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

const PROGRESS_EMIT_INTERVAL: u64 = 250;
const PARTIAL_SAMPLE_SIZE: usize = 256 * 1024; // 256KB
const SMALL_FILE_THRESHOLD: u64 = 4 * 1024 * 1024; // 4MB

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

fn validate_scan_path(path: &str) -> anyhow::Result<()> {
    let path_buf = PathBuf::from(path);
    if path_buf
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("Path traversal not allowed");
    }
    if path_buf.is_absolute() {
        if !path_buf.exists() {
            anyhow::bail!("Path does not exist");
        }
        if path_buf.parent().is_none() {
            anyhow::bail!("Cannot scan system root");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanResult {
    pub counted: u64,
    pub skipped: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgressPayload {
    pub scanned: u64,
    pub skipped: u64,
    pub errors: u64,
    pub path_sample: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanFinishedPayload {
    pub scanned: u64,
    pub skipped: u64,
    pub errors: u64,
    pub error_messages: Vec<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanErrorPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanStatusPayload {
    pub state: String,
    pub scanned: u64,
    pub skipped: u64,
    pub errors: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub roots: usize,
    pub current_path: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct ScanStatusInternal {
    state: ScanState,
    scanned: u64,
    skipped: u64,
    errors: u64,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    roots: usize,
    current_path: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScanState {
    Idle,
    Running,
}

impl Default for ScanStatusInternal {
    fn default() -> Self {
        Self {
            state: ScanState::Idle,
            scanned: 0,
            skipped: 0,
            errors: 0,
            started_at: None,
            finished_at: None,
            roots: 0,
            current_path: None,
            last_error: None,
        }
    }
}

static SCAN_STATUS: Lazy<Mutex<ScanStatusInternal>> =
    Lazy::new(|| Mutex::new(ScanStatusInternal::default()));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanTrigger {
    Manual,
    Watcher,
}

impl ScanTrigger {
    fn emit_queued(self) -> bool {
        true
    }
}

#[derive(Clone)]
struct ScanJob {
    roots: Vec<String>,
    trigger: ScanTrigger,
}

static SCAN_QUEUE: Lazy<Mutex<VecDeque<ScanJob>>> = Lazy::new(|| Mutex::new(VecDeque::new()));

fn enqueue_scan_job<R: tauri::Runtime>(
    app: &AppHandle<R>,
    pool: &DbPool,
    roots: Vec<String>,
    trigger: ScanTrigger,
) -> anyhow::Result<()> {
    if roots.is_empty() {
        anyhow::bail!("no scan roots provided");
    }

    {
        let mut queue = SCAN_QUEUE.lock().expect("scan queue lock");
        if queue.iter().any(|job| job.roots == roots) {
            return Ok(());
        }
        queue.push_back(ScanJob { roots, trigger });
    }

    process_queue(app, pool);
    Ok(())
}

fn process_queue<R: tauri::Runtime>(app: &AppHandle<R>, pool: &DbPool) {
    let job_opt = {
        let mut queue = SCAN_QUEUE.lock().expect("scan queue lock");
        let mut status = SCAN_STATUS.lock().expect("scan status lock");
        if status.state == ScanState::Running {
            None
        } else {
            queue.pop_front().map(|job| {
                status.state = ScanState::Running;
                status.scanned = 0;
                status.skipped = 0;
                status.errors = 0;
                status.started_at = Some(Utc::now());
                status.finished_at = None;
                status.roots = job.roots.len();
                status.current_path = None;
                status.last_error = None;
                job
            })
        }
    };

    if let Some(job) = job_opt {
        if job.trigger.emit_queued() {
            emit_queued(app, job.roots.len());
        }

        let app_handle = app.clone();
        let pool_clone = pool.clone();
        let roots = job.roots.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let result = (|| {
                let conn = pool_clone
                    .get()
                    .map_err(|e| anyhow::anyhow!("db pool: {e}"))?;
                let db = Database::new(conn);
                let mut scanner = Scanner::new();
                scanner.run_scan(&app_handle, roots.clone(), &db)
            })();

            match result {
                Ok(summary) => finalize_status(
                    summary.counted,
                    summary.skipped,
                    summary.errors.len() as u64,
                ),
                Err(err) => {
                    let message = err.to_string();
                    finalize_status_error(message.clone());
                    emit_error(&app_handle, message);
                }
            }

            process_queue(&app_handle, &pool_clone);
        });
    }
}

pub(crate) fn queue_scan_from_watcher<R: tauri::Runtime>(
    app: &AppHandle<R>,
    pool: &DbPool,
    roots: Vec<String>,
) -> anyhow::Result<()> {
    enqueue_scan_job(app, pool, roots, ScanTrigger::Watcher)
}

pub const SCAN_PROGRESS_EVENT: &str = "scan://progress";
pub const SCAN_DONE_EVENT: &str = "scan://done";
pub const SCAN_ERROR_EVENT: &str = "scan://error";
pub const SCAN_QUEUED_EVENT: &str = "scan://queued";

pub fn start_scan<R: tauri::Runtime>(
    app: AppHandle<R>,
    pool: DbPool,
    roots: Vec<String>,
) -> anyhow::Result<()> {
    if roots.is_empty() {
        anyhow::bail!("no scan roots provided");
    }

    let mut unique = HashSet::new();
    let mut sanitized = Vec::new();
    for root in roots {
        validate_scan_path(&root).map_err(|e| anyhow::anyhow!("ERR_VALIDATION: {e}"))?;
        if !Path::new(&root).is_dir() {
            anyhow::bail!("ERR_VALIDATION: Path is not a directory: {root}");
        }
        let clean = sanitize_string(&root);
        if unique.insert(clean.clone()) {
            sanitized.push(clean);
        }
    }

    enqueue_scan_job(&app, &pool, sanitized, ScanTrigger::Manual)
}

pub fn current_status() -> ScanStatusPayload {
    let status = SCAN_STATUS.lock().expect("scan status lock");
    ScanStatusPayload {
        state: match status.state {
            ScanState::Idle => "idle".to_string(),
            ScanState::Running => "running".to_string(),
        },
        scanned: status.scanned,
        skipped: status.skipped,
        errors: status.errors,
        started_at: status.started_at,
        finished_at: status.finished_at,
        roots: status.roots,
        current_path: status.current_path.clone(),
        last_error: status.last_error.clone(),
    }
}

fn update_progress(scanned: u64, skipped: u64, errors: u64, current: Option<PathBuf>) {
    if let Ok(mut status) = SCAN_STATUS.lock() {
        status.scanned = scanned;
        status.skipped = skipped;
        status.errors = errors;
        status.current_path = current.map(|p| p.to_string_lossy().to_string());
        status.last_error = None;
    }
}

fn finalize_status(scanned: u64, skipped: u64, errors: u64) {
    if let Ok(mut status) = SCAN_STATUS.lock() {
        status.scanned = scanned;
        status.skipped = skipped;
        status.errors = errors;
        status.finished_at = Some(Utc::now());
        status.state = ScanState::Idle;
        status.current_path = None;
        status.last_error = None;
    }
}

fn finalize_status_error(message: String) {
    if let Ok(mut status) = SCAN_STATUS.lock() {
        status.errors += 1;
        status.finished_at = Some(Utc::now());
        status.state = ScanState::Idle;
        status.current_path = None;
        status.last_error = Some(message);
    }
}

pub struct Scanner {
    file_walker: FileWalker,
    project_detector: ActiveProjectDetector,
    performance_target_ms: u64,
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            file_walker: FileWalker::new(),
            project_detector: ActiveProjectDetector::new(),
            performance_target_ms: 90_000,
        }
    }

    pub fn run_scan<R: tauri::Runtime>(
        &mut self,
        app: &AppHandle<R>,
        roots: Vec<String>,
        db: &Database,
    ) -> anyhow::Result<ScanResult> {
        let start_time = SystemTime::now();

        let repos = self.project_detector.detect_dev_repos(&roots);
        self.record_project_metrics(&repos, db);

        let mut summary = ScanResult {
            counted: 0,
            skipped: 0,
            duration_ms: 0,
            errors: Vec::new(),
        };

        let mut hash_candidates: HashMap<(u64, String), Vec<(i64, String)>> = HashMap::new();
        for root in roots.iter() {
            let root_path = Path::new(root);
            if !root_path.exists() {
                summary
                    .errors
                    .push(format!("Root path does not exist: {}", root));
                continue;
            }

            let mut root_seen: HashSet<String> = HashSet::new();
            let mut entries = WalkDir::new(root_path).follow_links(false).into_iter();
            while let Some(entry) = entries.next() {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();

                        if entry.file_type().is_dir() {
                            if self.file_walker.should_skip_dir(path) {
                                summary.skipped += 1;
                                entries.skip_current_dir();
                            }
                            continue;
                        }

                        if entry.file_type().is_symlink() {
                            summary.skipped += 1;
                            continue;
                        }

                        if self.file_walker.should_skip_file(path) {
                            summary.skipped += 1;
                            continue;
                        }

                        match self.process_file(path, db, &mut hash_candidates) {
                            Ok(stored_path) => {
                                root_seen.insert(stored_path);
                                summary.counted += 1;
                                if summary.counted % PROGRESS_EMIT_INTERVAL == 0 {
                                    emit_progress(
                                        app,
                                        summary.counted,
                                        summary.skipped,
                                        summary.errors.len() as u64,
                                        Some(path),
                                    );
                                    update_progress(
                                        summary.counted,
                                        summary.skipped,
                                        summary.errors.len() as u64,
                                        Some(path.to_path_buf()),
                                    );
                                }
                            }
                            Err(err) => {
                                summary.errors.push(err.to_string());
                            }
                        }
                    }
                    Err(err) => {
                        summary.errors.push(err.to_string());
                        summary.skipped += 1;
                    }
                }
            }

            if let Err(err) = db.mark_missing_for_root(root, &root_seen) {
                summary.errors.push(format!("Failed to reconcile missing entries for {}: {}", root, err));
            }
        }

        self.populate_full_hashes(db, &mut hash_candidates, &mut summary);

        let duration = start_time.elapsed().unwrap_or(Duration::from_secs(0));
        summary.duration_ms = duration.as_millis() as u64;

        emit_progress(
            app,
            summary.counted,
            summary.skipped,
            summary.errors.len() as u64,
            None,
        );
        let finished_at = Utc::now();
        let started_at = DateTime::<Utc>::from(start_time);
        emit_done(
            app,
            ScanFinishedPayload {
                scanned: summary.counted,
                skipped: summary.skipped,
                errors: summary.errors.len() as u64,
                error_messages: summary.errors.clone(),
                started_at: Some(started_at),
                finished_at: Some(finished_at),
            },
        );
        if !summary.errors.is_empty() {
            for message in &summary.errors {
                emit_error(app, message.clone());
            }
        }
        update_progress(
            summary.counted,
            summary.skipped,
            summary.errors.len() as u64,
            None,
        );

        self.record_performance_metrics(&summary, duration, db);

        Ok(summary)
    }

    fn process_file(
        &self,
        path: &Path,
        db: &Database,
        hash_candidates: &mut HashMap<(u64, String), Vec<(i64, String)>>,
    ) -> anyhow::Result<String> {
        let metadata = self.file_walker.extract_metadata(path)?;
        let path_str = metadata.path.to_string_lossy().to_string();
        let parent_dir = metadata.parent_dir.to_string_lossy().to_string();

        let partial_hash = hash_first_n(&metadata.path, PARTIAL_SAMPLE_SIZE).ok();
        let mut full_hash = None;
        if metadata.size_bytes <= SMALL_FILE_THRESHOLD {
            full_hash = hash_full(&metadata.path).ok();
        }

        let new_file = NewFile {
            path: path_str.clone(),
            parent_dir,
            mime: metadata.mime_type,
            size_bytes: metadata.size_bytes as i64,
            created_at: metadata.created_at,
            modified_at: metadata.modified_at,
            accessed_at: metadata.accessed_at,
            partial_sha1: partial_hash.clone(),
            sha1: full_hash.clone(),
        };

        let file_id = db.upsert_file(&new_file)?;

        if full_hash.is_none() {
            if let Some(partial) = partial_hash {
                hash_candidates
                    .entry((metadata.size_bytes, partial))
                    .or_default()
                    .push((file_id, path_str.clone()));
            }
        }

        Ok(path_str)
    }

    fn populate_full_hashes(
        &self,
        db: &Database,
        hash_candidates: &mut HashMap<(u64, String), Vec<(i64, String)>>,
        summary: &mut ScanResult,
    ) {
        for ((_, partial), entries) in hash_candidates.drain() {
            if entries.len() < 2 {
                continue;
            }

            for (file_id, path) in entries {
                let path_buf = PathBuf::from(&path);
                match hash_full(&path_buf) {
                    Ok(full) => {
                        if let Err(err) =
                            db.update_file_hashes(file_id, Some(&partial), Some(&full))
                        {
                            summary
                                .errors
                                .push(format!("Failed to update hash for {}: {}", path, err));
                        }
                    }
                    Err(err) => {
                        summary
                            .errors
                            .push(format!("Failed to hash {}: {}", path, err));
                    }
                }
            }
        }
    }

    fn record_project_metrics(&self, repos: &[DevRepo], db: &Database) {
        let activity_stats = self.project_detector.analyze_project_activity(repos);
        for (metric, count) in activity_stats {
            let new_metric = NewMetric {
                metric: format!("project_{}", metric),
                value: count as f64,
                context: Some("scan".to_string()),
            };
            if let Err(e) = db.insert_metric(&new_metric) {
                eprintln!("Failed to record project metric: {}", e);
            }
        }

        let total_repos = NewMetric {
            metric: "total_dev_repos".to_string(),
            value: repos.len() as f64,
            context: Some("scan".to_string()),
        };
        if let Err(e) = db.insert_metric(&total_repos) {
            eprintln!("Failed to record total repos metric: {}", e);
        }
    }

    fn record_performance_metrics(&self, result: &ScanResult, duration: Duration, db: &Database) {
        let duration_ms = duration.as_millis() as u64;

        let performance_metric = NewMetric {
            metric: "scan_duration_ms".to_string(),
            value: duration_ms as f64,
            context: Some("performance".to_string()),
        };
        if let Err(e) = db.insert_metric(&performance_metric) {
            eprintln!("Failed to record performance metric: {}", e);
        }

        let files_counted = NewMetric {
            metric: "files_counted".to_string(),
            value: result.counted as f64,
            context: Some("scan".to_string()),
        };
        if let Err(e) = db.insert_metric(&files_counted) {
            eprintln!("Failed to record files counted metric: {}", e);
        }

        let files_skipped = NewMetric {
            metric: "files_skipped".to_string(),
            value: result.skipped as f64,
            context: Some("scan".to_string()),
        };
        if let Err(e) = db.insert_metric(&files_skipped) {
            eprintln!("Failed to record files skipped metric: {}", e);
        }

        let files_per_second = if duration_ms > 0 {
            (result.counted as f64) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        let throughput_metric = NewMetric {
            metric: "files_per_second".to_string(),
            value: files_per_second,
            context: Some("performance".to_string()),
        };
        if let Err(e) = db.insert_metric(&throughput_metric) {
            eprintln!("Failed to record throughput metric: {}", e);
        }

        let target_met = duration_ms <= self.performance_target_ms;
        let target_metric = NewMetric {
            metric: "performance_target_met".to_string(),
            value: if target_met { 1.0 } else { 0.0 },
            context: Some("performance".to_string()),
        };
        if let Err(e) = db.insert_metric(&target_metric) {
            eprintln!("Failed to record target metric: {}", e);
        }
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_progress<R: tauri::Runtime>(
    app: &AppHandle<R>,
    scanned: u64,
    skipped: u64,
    errors: u64,
    path: Option<&Path>,
) {
    let payload = ScanProgressPayload {
        scanned,
        skipped,
        errors,
        path_sample: path.map(|p| p.to_string_lossy().to_string()),
    };
    let _ = app.emit(SCAN_PROGRESS_EVENT, payload);
}

fn emit_done<R: tauri::Runtime>(app: &AppHandle<R>, payload: ScanFinishedPayload) {
    let _ = app.emit(SCAN_DONE_EVENT, payload);
}

fn emit_error<R: tauri::Runtime>(app: &AppHandle<R>, message: String) {
    let payload = ScanErrorPayload { message };
    let _ = app.emit(SCAN_ERROR_EVENT, payload);
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanQueuedPayload {
    pub roots: usize,
}

fn emit_queued<R: tauri::Runtime>(app: &AppHandle<R>, roots: usize) {
    let payload = ScanQueuedPayload { roots };
    let _ = app.emit(SCAN_QUEUED_EVENT, payload);
}
