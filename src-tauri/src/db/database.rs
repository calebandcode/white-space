use crate::models::{Action, File, NewAction, NewFile, NewMetric, NewStagedFile, StagedFileRecord, WatchedRoot, WeeklyTotals};
use chrono::{DateTime, Utc};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Result as SqliteResult, Row};
use std::collections::HashSet;
use std::path::Path;

pub struct Database {
    conn: PooledConnection<SqliteConnectionManager>,
}

impl Database {
    pub fn new(conn: PooledConnection<SqliteConnectionManager>) -> Self {
        Database { conn }
    }

    fn map_row_to_file(row: &Row<'_>) -> SqliteResult<File> {
        let mime: Option<String> = row.get("mime").unwrap_or(None);
        let mime = mime.filter(|s| !s.is_empty());

        let partial_sha1: Option<String> = row.get("partial_sha1").unwrap_or(None);
        let partial_sha1 = partial_sha1.filter(|s| !s.is_empty());

        let sha1: Option<String> = row.get("sha1").unwrap_or(None);
        let sha1 = sha1.filter(|s| !s.is_empty());

        let is_deleted = row.get::<_, i64>("is_deleted").unwrap_or(0) != 0;
        let is_staged = row.get::<_, i64>("is_staged").unwrap_or(0) != 0;
        let cooloff_until = row
            .get::<_, Option<DateTime<Utc>>>("cooloff_until")
            .unwrap_or(None);

        Ok(File {
            id: row.get("id")?,
            path: row.get("path")?,
            parent_dir: row.get("parent_dir")?,
            mime,
            size_bytes: row.get("size_bytes")?,
            created_at: row.get("created_at")?,
            modified_at: row.get("modified_at").unwrap_or(None),
            accessed_at: row.get("accessed_at").unwrap_or(None),
            last_opened_at: row.get("last_opened_at").unwrap_or(None),
            partial_sha1,
            sha1,
            first_seen_at: row.get("first_seen_at")?,
            last_seen_at: row.get("last_seen_at")?,
            is_deleted,
            is_staged,
            cooloff_until,
        })
    }

    fn map_row_to_staged(row: &Row<'_>) -> SqliteResult<StagedFileRecord> {
        Ok(StagedFileRecord {
            id: row.get("id")?,
            file_id: row.get("file_id")?,
            staged_at: row.get("staged_at")?,
            expires_at: row.get("expires_at").unwrap_or(None),
            batch_id: row.get("batch_id").unwrap_or(None),
            status: row.get("status")?,
            note: row.get("note").unwrap_or(None),
        })
    }

    pub fn run_migrations(&self) -> SqliteResult<()> {
        // Enable WAL mode - use query instead of execute for PRAGMA
        let _: String = self
            .conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                parent_dir TEXT NOT NULL,
                mime TEXT,
                size_bytes INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT,
                accessed_at TEXT,
                last_opened_at TEXT,
                partial_sha1 TEXT,
                sha1 TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                is_deleted INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER NOT NULL,
                action TEXT NOT NULL CHECK (action IN ('archive', 'delete', 'restore')),
                batch_id TEXT NOT NULL,
                src_path TEXT NOT NULL,
                dst_path TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (file_id) REFERENCES files (id)
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS prefs (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                metric TEXT NOT NULL,
                value REAL NOT NULL,
                context TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS watched_roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        self.ensure_column("files", "modified_at", "TEXT")?;
        self.ensure_column("files", "accessed_at", "TEXT")?;
        self.ensure_column("files", "last_opened_at", "TEXT")?;
        self.ensure_column("files", "partial_sha1", "TEXT")?;
        self.ensure_column("files", "sha1", "TEXT")?;
        self.ensure_column("files", "is_staged", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("files", "cooloff_until", "TEXT")?;
        self.ensure_column("actions", "origin", "TEXT")?;
        self.ensure_column("actions", "note", "TEXT")?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS staged_files (\n                id INTEGER PRIMARY KEY AUTOINCREMENT,\n                file_id INTEGER NOT NULL,\n                staged_at TEXT NOT NULL,\n                expires_at TEXT,\n                batch_id TEXT,\n                status TEXT NOT NULL DEFAULT 'pending',\n                note TEXT,\n                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE\n            )",
            [],
        )?;
        self.conn.execute("CREATE INDEX IF NOT EXISTS idx_staged_files_status ON staged_files(status)", [])?;
        self.conn.execute("CREATE INDEX IF NOT EXISTS idx_staged_files_expires_at ON staged_files(expires_at)", [])?;
        self.conn.execute("CREATE INDEX IF NOT EXISTS idx_staged_files_file_id ON staged_files(file_id)", [])?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_parent_dir ON files(parent_dir)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_last_seen_at ON files(last_seen_at)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_actions_batch_id ON actions(batch_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_actions_action_created_at ON actions(action, created_at)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_sha1 ON files(sha1)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_partial_sha1 ON files(partial_sha1)",
            [],
        )?;

        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, column_type: &str) -> SqliteResult<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                return Ok(());
            }
        }
        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}");
        let _ = self.conn.execute(&sql, []);
        Ok(())
    }

    pub fn upsert_file(&self, file: &NewFile) -> SqliteResult<i64> {
        let now = Utc::now();
        let created_at = file.created_at.unwrap_or(now);
        self.conn.query_row(
            "INSERT INTO files (
                path, parent_dir, mime, size_bytes, created_at, modified_at, accessed_at,
                last_opened_at, partial_sha1, sha1, first_seen_at, last_seen_at, is_deleted
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)
            ON CONFLICT(path) DO UPDATE SET
                parent_dir = excluded.parent_dir,
                mime = excluded.mime,
                size_bytes = excluded.size_bytes,
                modified_at = excluded.modified_at,
                accessed_at = excluded.accessed_at,
                partial_sha1 = excluded.partial_sha1,
                sha1 = COALESCE(excluded.sha1, files.sha1),
                last_seen_at = excluded.last_seen_at,
                is_deleted = 0
            RETURNING id",
            params![
                &file.path,
                &file.parent_dir,
                file.mime.as_deref(),
                file.size_bytes,
                created_at,
                file.modified_at,
                file.accessed_at,
                Option::<DateTime<Utc>>::None,
                file.partial_sha1.as_deref(),
                file.sha1.as_deref(),
                now,
                now,
            ],
            |row| row.get(0),
        )
    }

    pub fn update_file_hashes(
        &self,
        file_id: i64,
        partial_sha1: Option<&str>,
        sha1: Option<&str>,
    ) -> SqliteResult<()> {
        self.conn.execute(
            "UPDATE files SET partial_sha1 = COALESCE(?1, partial_sha1), sha1 = COALESCE(?2, sha1) WHERE id = ?3",
            params![partial_sha1, sha1, file_id],
        )?;
        Ok(())
    }

    pub fn mark_missing_as_deleted(&self, existing_paths: &[String]) -> SqliteResult<u64> {
        let placeholders = existing_paths
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "UPDATE files SET is_deleted = 1 WHERE path NOT IN ({})",
            if placeholders.is_empty() {
                "''".to_string()
            } else {
                placeholders
            }
        );
        let rows = if existing_paths.is_empty() {
            self.conn.execute("UPDATE files SET is_deleted = 1", [])?
        } else {
            let params = existing_paths
                .iter()
                .map(|p| p as &dyn rusqlite::ToSql)
                .collect::<Vec<_>>();
            self.conn.execute(&sql, params.as_slice())?
        };
        Ok(rows as u64)
    }

    pub fn get_file_by_id(&self, id: i64) -> SqliteResult<Option<File>> {
        let mut stmt = self.conn.prepare("SELECT * FROM files WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], |row| Self::map_row_to_file(row))?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_active_files(&self) -> SqliteResult<Vec<File>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM files WHERE is_deleted = 0 ORDER BY last_seen_at DESC")?;
        let rows = stmt.query_map([], |row| Self::map_row_to_file(row))?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    pub fn by_dir(&self, parent_dir: &str) -> SqliteResult<Vec<File>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM files WHERE parent_dir = ?1 AND is_deleted = 0")?;
        let rows = stmt.query_map([parent_dir], |row| Self::map_row_to_file(row))?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    pub fn insert_action(&self, action: &NewAction) -> SqliteResult<i64> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO actions (file_id, action, batch_id, src_path, dst_path, origin, note, created_at)\n             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                action.file_id,
                action.action.to_string(),
                action.batch_id.as_deref().unwrap_or(""),
                action.src_path.as_deref().unwrap_or(""),
                action.dst_path.as_deref().unwrap_or(""),
                action.origin.as_deref().unwrap_or(""),
                action.note.as_deref().unwrap_or(""),
                &now.to_rfc3339(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn latest_action(&self, file_id: i64) -> SqliteResult<Option<Action>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM actions WHERE file_id = ?1 ORDER BY created_at DESC LIMIT 1")?;
        let mut rows = stmt.query_map([file_id], |row| {
            let action_str: String = row.get("action")?;
            let action = action_str.parse().map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "ActionType".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;
            Ok(Action {
                id: row.get("id")?,
                file_id: row.get("file_id")?,
                action,
                batch_id: row.get("batch_id")?,
                src_path: row.get("src_path")?,
                dst_path: row.get("dst_path")?,
                origin: row.get("origin")?,
                note: row.get("note")?,
                created_at: row.get("created_at")?,
            })
        })?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn weekly_totals(&self, weeks_back: i64) -> SqliteResult<Vec<WeeklyTotals>> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                DATE(created_at, '-' || (?1 * 7) || ' days') as week_start,
                COUNT(*) as total_files,
                SUM(CASE WHEN action = 'archive' THEN 1 ELSE 0 END) as archived_files,
                SUM(CASE WHEN action = 'delete' THEN 1 ELSE 0 END) as deleted_files,
                SUM(CASE WHEN action = 'restore' THEN 1 ELSE 0 END) as restored_files
             FROM actions a
             JOIN files f ON a.file_id = f.id
             WHERE a.created_at >= datetime('now', '-' || (?1 * 7) || ' days')
             GROUP BY week_start
             ORDER BY week_start DESC",
        )?;
        let rows = stmt.query_map([weeks_back], |row| {
            Ok(WeeklyTotals {
                week_start: row.get(0)?,
                total_files: row.get(1)?,
                archived_files: row.get(2)?,
                deleted_files: row.get(3)?,
                restored_files: row.get(4)?,
            })
        })?;
        let mut totals = Vec::new();
        for row in rows {
            totals.push(row?);
        }
        Ok(totals)
    }

    pub fn set_preference(&self, key: &str, value: &str) -> SqliteResult<()> {
        let mut stmt = self
            .conn
            .prepare("INSERT OR REPLACE INTO prefs (key, value) VALUES (?1, ?2)")?;
        stmt.execute([key, value])?;
        Ok(())
    }

    pub fn get_preference(&self, key: &str) -> SqliteResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM prefs WHERE key = ?1")?;
        stmt.query_row([key], |row| row.get(0)).optional()
    }

    pub fn get_all_preferences(&self) -> SqliteResult<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM prefs")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut prefs = std::collections::HashMap::new();
        for row in rows {
            let (key, value) = row?;
            prefs.insert(key, value);
        }
        Ok(prefs)
    }

    pub fn insert_metric(&self, metric: &NewMetric) -> SqliteResult<i64> {
        let now = Utc::now();
        let mut stmt = self.conn.prepare(
            "INSERT INTO metrics (metric, value, context, created_at)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        stmt.execute([
            &metric.metric,
            &metric.value.to_string(),
            &metric
                .context
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            &now.to_rfc3339(),
        ])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn upsert_watched_root(&self, path: &str) -> SqliteResult<i64> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT OR IGNORE INTO watched_roots (path, created_at) VALUES (?1, ?2)",
            params![path, now],
        )?;
        self.conn.query_row(
            "SELECT id FROM watched_roots WHERE path = ?1",
            [path],
            |row| row.get(0),
        )
    }

    pub fn delete_watched_root(&self, path: &str) -> SqliteResult<()> {
        self.conn
            .execute("DELETE FROM watched_roots WHERE path = ?1", [path])?;
        Ok(())
    }

    pub fn get_watched_root_by_id(&self, id: i64) -> SqliteResult<Option<WatchedRoot>> {
        self.conn
            .query_row(
                "SELECT id, path, created_at FROM watched_roots WHERE id = ?1",
                [id],
                |row| {
                    Ok(WatchedRoot {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                },
            )
            .optional()
    }

    pub fn list_watched_roots(&self) -> SqliteResult<Vec<WatchedRoot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, created_at FROM watched_roots ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(WatchedRoot {
                id: row.get(0)?,
                path: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        let mut roots = Vec::new();
        for row in rows {
            roots.push(row?);
        }
        Ok(roots)
    }

    pub fn list_watched_paths(&self) -> SqliteResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM watched_roots ORDER BY created_at ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut paths = Vec::new();
        for row in rows {
            paths.push(row?);
        }
        Ok(paths)
    }

    // File ID lookup methods
    pub fn get_file_id_by_path(&self, path: &str) -> SqliteResult<Option<i64>> {
        self.conn
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                [path],
                |row| row.get(0),
            )
            .optional()
    }


    // Action-related queries
    pub fn get_actions_by_batch_id(&self, batch_id: &str) -> SqliteResult<Vec<Action>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, action, batch_id, src_path, dst_path, origin, note, created_at FROM actions WHERE batch_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map([batch_id], |row| {
            let action = row
                .get::<_, String>(2)?
                .parse()
                .unwrap_or(crate::models::ActionType::Archive);
            Ok(Action {
                id: Some(row.get(0)?),
                file_id: row.get(1)?,
                action,
                batch_id: row.get(3)?,
                src_path: row.get(4)?,
                dst_path: row.get(5)?,
                origin: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut actions = Vec::new();
        for row in rows {
            actions.push(row?);
        }
        Ok(actions)
    }

    pub fn get_latest_batch_id(&self) -> SqliteResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT batch_id FROM actions ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn get_undoable_batches(&self) -> SqliteResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT batch_id FROM actions WHERE action IN ('archive', 'delete') ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut batches = Vec::new();
        for row in rows {
            batches.push(row?);
        }
        Ok(batches)
    }

    // Gauge-related queries
    pub fn get_files_archived_in_period(&self, start_date: &str, end_date: &str) -> SqliteResult<Vec<File>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.path, f.parent_dir, f.mime, f.size_bytes, f.created_at, f.modified_at, f.accessed_at, f.last_opened_at, f.partial_sha1, f.sha1, f.first_seen_at, f.last_seen_at, f.is_deleted 
             FROM files f 
             JOIN actions a ON f.id = a.file_id 
             WHERE a.action = 'archive' AND a.created_at BETWEEN ?1 AND ?2"
        )?;
        let rows = stmt.query_map([start_date, end_date], Self::map_row_to_file)?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    pub fn get_files_deleted_in_period(&self, start_date: &str, end_date: &str) -> SqliteResult<Vec<Action>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, action, batch_id, src_path, dst_path, origin, note, created_at FROM actions WHERE action = 'delete' AND created_at BETWEEN ?1 AND ?2"
        )?;
        let rows = stmt.query_map([start_date, end_date], |row| {
            let action = row
                .get::<_, String>(2)?
                .parse()
                .unwrap_or(crate::models::ActionType::Delete);
            Ok(Action {
                id: Some(row.get(0)?),
                file_id: row.get(1)?,
                action,
                batch_id: row.get(3)?,
                src_path: row.get(4)?,
                dst_path: row.get(5)?,
                origin: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        let mut actions = Vec::new();
        for row in rows {
            actions.push(row?);
        }
        Ok(actions)
    }

    // Staged-in-window queries (current staged state only)
    pub fn list_current_staged_files_in_period(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> SqliteResult<Vec<File>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.path, f.parent_dir, f.mime, f.size_bytes, f.created_at, f.modified_at, f.accessed_at, f.last_opened_at, f.partial_sha1, f.sha1, f.first_seen_at, f.last_seen_at, f.is_deleted \
             FROM staged_files s \
             JOIN files f ON f.id = s.file_id \
             WHERE s.status = 'staged' AND s.staged_at BETWEEN ?1 AND ?2"
        )?;
        let rows = stmt.query_map([start_date, end_date], Self::map_row_to_file)?;
        let mut files = Vec::new();
        for row in rows { files.push(row?); }
        Ok(files)
    }

    pub fn stage_files(&self, entries: &[NewStagedFile]) -> SqliteResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut insert_stmt = self.conn.prepare(
            "INSERT INTO staged_files (file_id, staged_at, expires_at, batch_id, status, note)\n             VALUES (?1, ?2, ?3, ?4, ?5, ?6)\n             ON CONFLICT(file_id) DO UPDATE SET\n                staged_at = excluded.staged_at,\n                expires_at = excluded.expires_at,\n                batch_id = excluded.batch_id,\n                status = excluded.status,\n                note = excluded.note"
        )?;
        let mut update_stmt = self.conn.prepare("UPDATE files SET is_staged = 1, cooloff_until = ?2 WHERE id = ?1")?;

        for entry in entries {
            let staged_at = entry.staged_at.to_rfc3339();
            let expires_at = entry.expires_at.map(|dt| dt.to_rfc3339());
            insert_stmt.execute(params![
                entry.file_id,
                &staged_at,
                expires_at.as_deref(),
                entry.batch_id.as_deref().unwrap_or(""),
                entry.status.as_str(),
                entry.note.as_deref().unwrap_or(""),
            ])?;
            update_stmt.execute(params![entry.file_id, expires_at.as_deref()])?;
        }

        Ok(())
    }

    pub fn update_staged_status(&self, file_ids: &[i64], status: &str) -> SqliteResult<()> {
        if file_ids.is_empty() {
            return Ok(());
        }
        let mut stmt = self.conn.prepare("UPDATE staged_files SET status = ?1 WHERE file_id = ?2")?;
        for file_id in file_ids {
            stmt.execute(params![status, file_id])?;
        }
        Ok(())
    }

    pub fn mark_files_unstaged(&self, file_ids: &[i64]) -> SqliteResult<()> {
        if file_ids.is_empty() {
            return Ok(());
        }
        let mut reset_stmt = self.conn.prepare("UPDATE files SET is_staged = 0, cooloff_until = NULL WHERE id = ?1")?;
        let mut delete_stmt = self.conn.prepare("DELETE FROM staged_files WHERE file_id = ?1")?;
        for file_id in file_ids {
            reset_stmt.execute(params![file_id])?;
            delete_stmt.execute(params![file_id])?;
        }
        Ok(())
    }

    pub fn list_staged_with_files(&self, statuses: Option<&[String]>) -> SqliteResult<Vec<(StagedFileRecord, File)>> {
        let filters = statuses.map(|items| items.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>());
        let mut stmt = self.conn.prepare("SELECT id, file_id, staged_at, expires_at, batch_id, status, note FROM staged_files")?;
        let rows = stmt.query_map([], |row| Self::map_row_to_staged(row))?;
        let mut results = Vec::new();
        for row in rows {
            let record = row?;
            if let Some(filter) = &filters {
                let status = record.status.to_lowercase();
                if !filter.iter().any(|s| s == &status) {
                    continue;
                }
            }
            if let Some(file) = self.get_file_by_id(record.file_id)? {
                results.push((record, file));
            }
        }
        Ok(results)
    }

    pub fn update_file_location(&self, file_id: i64, new_path: &str) -> SqliteResult<()> {
        let parent = Path::new(new_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| new_path.to_string());
        self.conn.execute("UPDATE files SET path = ?1, parent_dir = ?2 WHERE id = ?3", params![new_path, parent, file_id])?;
        Ok(())
    }

    pub fn duplicate_groups(&self, limit: Option<usize>) -> SqliteResult<Vec<(String, Vec<File>)>> {
        let base_sql = "SELECT sha1 FROM files WHERE sha1 IS NOT NULL AND sha1 != '' AND is_deleted = 0 GROUP BY sha1 HAVING COUNT(*) > 1 ORDER BY COUNT(*) DESC";
        let hashes = if let Some(limit) = limit {
            let mut stmt = self.conn.prepare(&format!("{base_sql} LIMIT ?"))?;
            let rows = stmt.query_map([limit as i64], |row| row.get::<_, String>(0))?;
            let mut collected = Vec::new();
            for row in rows {
                collected.push(row?);
            }
            collected
        } else {
            let mut stmt = self.conn.prepare(base_sql)?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut collected = Vec::new();
            for row in rows {
                collected.push(row?);
            }
            collected
        };

        let mut results = Vec::with_capacity(hashes.len());
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM files WHERE sha1 = ?1 AND sha1 IS NOT NULL AND sha1 != '' AND is_deleted = 0 ORDER BY size_bytes DESC")?;
        for hash in hashes {
            let rows = stmt.query_map([hash.as_str()], |row| Self::map_row_to_file(row))?;
            let mut files = Vec::new();
            for row in rows {
                files.push(row?);
            }
            results.push((hash, files));
        }

        Ok(results)
    }

    pub fn mark_missing_for_root(&self, root: &str, seen_paths: &HashSet<String>) -> SqliteResult<()> {
        let pattern = Self::root_like_pattern(root);
        let mut stmt = self.conn.prepare("SELECT id, path FROM files WHERE path LIKE ?1 AND is_deleted = 0")?;
        let rows = stmt.query_map([pattern], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        let mut missing = Vec::new();
        for row in rows {
            let (file_id, path) = row?;
            if !seen_paths.contains(&path) {
                missing.push(file_id);
            }
        }
        for id in missing {
            self.conn.execute("UPDATE files SET is_deleted = 1, is_staged = 0, cooloff_until = NULL WHERE id = ?1", params![id])?;
            self.conn.execute("DELETE FROM staged_files WHERE file_id = ?1", params![id])?;
        }
        Ok(())
    }

    fn root_like_pattern(root: &str) -> String {
        if root.ends_with('/') || root.ends_with('\\') {
            format!("{root}%")
        } else if root.contains('\\') {
            format!("{root}\\%")
        } else {
            format!("{root}/%")
        }
    }

    pub fn get_total_file_size(&self) -> SqliteResult<i64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(size_bytes), 0) FROM files WHERE is_deleted = 0",
                [],
                |row| row.get(0),
            )
    }

    pub fn get_candidate_files(&self, limit: i64) -> SqliteResult<Vec<File>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, parent_dir, mime, size_bytes, created_at, modified_at, accessed_at, last_opened_at, partial_sha1, sha1, first_seen_at, last_seen_at, is_deleted 
             FROM files 
             WHERE is_deleted = 0 
             ORDER BY size_bytes DESC, last_seen_at ASC 
             LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], Self::map_row_to_file)?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }
}
