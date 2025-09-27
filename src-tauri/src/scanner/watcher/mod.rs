use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::{Lazy, OnceCell};
use tauri::AppHandle;

use crate::db::{Database, DbPool};
use super::queue_scan_from_watcher;

struct WatcherRuntime {
    watcher: RecommendedWatcher,
    roots: Arc<Mutex<Vec<PathBuf>>>,
}

static WATCHER_STATE: Lazy<Mutex<Option<WatcherRuntime>>> = Lazy::new(|| Mutex::new(None));
static WATCHER_STARTED: OnceCell<()> = OnceCell::new();

pub fn start_watchers<R: tauri::Runtime>(app: AppHandle<R>, pool: DbPool) -> anyhow::Result<()> {
    if WATCHER_STARTED.set(()).is_err() {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let roots_arc: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let callback_tx = tx.clone();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = callback_tx.send(res);
    })?;
    watcher.configure(Config::default().with_poll_interval(Duration::from_secs(2)))?;

    {
        let conn = pool
            .get()
            .context("watcher db pool")?;
        let db = Database::new(conn);
        let existing_roots = db
            .list_watched_paths()
            .context("list watched roots for watcher")?;
        for path in existing_roots {
            let path_buf = PathBuf::from(&path);
            if path_buf.exists() {
                watcher
                    .watch(&path_buf, RecursiveMode::Recursive)
                    .with_context(|| format!("watcher failed to watch path {path}"))?;
                roots_arc.lock().expect("watcher roots lock").push(path_buf);
            }
        }
    }

    {
        let mut guard = WATCHER_STATE.lock().expect("watcher state lock");
        *guard = Some(WatcherRuntime {
            watcher,
            roots: roots_arc.clone(),
        });
    }

    let thread_app = app.clone();
    let thread_pool = pool.clone();
    std::thread::spawn(move || {
        let mut backoff: HashMap<PathBuf, Instant> = HashMap::new();
        while let Ok(event_res) = rx.recv() {
            match event_res {
                Ok(event) => handle_event(&thread_app, &thread_pool, &roots_arc, &mut backoff, event),
                Err(err) => eprintln!("watcher error: {err}"),
            }
        }
    });

    Ok(())
}

pub fn register_root(path: &str) -> anyhow::Result<()> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Ok(());
    }
    let mut state = WATCHER_STATE.lock().expect("watcher state lock");
    if let Some(runtime) = state.as_mut() {
        let mut roots = runtime.roots.lock().expect("watcher roots lock");
        if roots.iter().any(|existing| existing == &path_buf) {
            return Ok(());
        }
        runtime
            .watcher
            .watch(&path_buf, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch new root {path}"))?;
        roots.push(path_buf);
    }
    Ok(())
}

pub fn unregister_root(path: &str) -> anyhow::Result<()> {
    let path_buf = PathBuf::from(path);
    let mut state = WATCHER_STATE.lock().expect("watcher state lock");
    if let Some(runtime) = state.as_mut() {
        let mut roots = runtime.roots.lock().expect("watcher roots lock");
        if let Some(index) = roots.iter().position(|existing| existing == &path_buf) {
            runtime
                .watcher
                .unwatch(&path_buf)
                .with_context(|| format!("failed to unwatch root {path}"))?;
            roots.remove(index);
        }
    }
    Ok(())
}

fn handle_event<R: tauri::Runtime>(
    app: &AppHandle<R>,
    pool: &DbPool,
    roots: &Arc<Mutex<Vec<PathBuf>>>,
    backoff: &mut HashMap<PathBuf, Instant>,
    event: Event,
) {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    ) {
        return;
    }

    let known_roots = roots.lock().expect("watcher roots lock").clone();
    if known_roots.is_empty() {
        return;
    }

    let mut affected = HashSet::new();
    for raw_path in event.paths {
        let canonical = canonicalize_best_effort(&raw_path);
        for root in &known_roots {
            if canonical.starts_with(root) {
                affected.insert(root.clone());
            }
        }
    }

    if affected.is_empty() {
        return;
    }

    let now = Instant::now();
    for root in affected {
        if let Some(last) = backoff.get(&root) {
            if now.duration_since(*last) < Duration::from_secs(5) {
                continue;
            }
        }
        backoff.insert(root.clone(), now);
        let root_str = root.to_string_lossy().to_string();
        if let Err(err) = queue_scan_from_watcher(app, pool, vec![root_str]) {
            eprintln!("failed to queue watcher scan: {err}");
        }
    }
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
