use crate::db::Database;
use crate::models::{ActionType, NewAction};
use crate::ops::error::{OpsError, OpsResult};
use chrono::{DateTime, Duration, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DeleteConfig {
    pub use_trash: bool,
    pub permanent_delete: bool,
    pub archive_age_threshold_days: i64,
    pub confirm_permanent: bool,
}

impl Default for DeleteConfig {
    fn default() -> Self {
        Self {
            use_trash: true,
            permanent_delete: false,
            archive_age_threshold_days: 7,
            confirm_permanent: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeleteResult {
    pub batch_id: String,
    pub files_deleted: usize,
    pub total_bytes_freed: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub trash_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteCandidate {
    pub file_id: i64,
    pub path: String,
    pub size_bytes: u64,
    pub age_days: i64,
    pub is_archive: bool,
    pub archive_age_days: Option<i64>,
}

pub struct DeleteManager {
    config: DeleteConfig,
}

impl DeleteManager {
    pub fn new() -> Self {
        Self {
            config: DeleteConfig::default(),
        }
    }

    pub fn delete_files(
        &mut self,
        file_paths: Vec<String>,
        db: &Database,
    ) -> OpsResult<DeleteResult> {
        let start_time = SystemTime::now();
        let batch_id = self.generate_batch_id();

        let mut files_deleted = 0;
        let mut total_bytes_freed = 0u64;
        let mut errors = Vec::new();
        let mut trash_path = None;

        for file_path in file_paths {
            match self.delete_single_file(&file_path, &batch_id, db) {
                Ok((bytes_freed, trash)) => {
                    files_deleted += 1;
                    total_bytes_freed += bytes_freed;
                    if trash.is_some() && trash_path.is_none() {
                        trash_path = trash;
                    }
                }
                Err(e) => {
                    errors.push(format!("Failed to delete {}: {}", file_path, e));
                }
            }
        }

        let duration = start_time
            .elapsed()
            .unwrap_or(std::time::Duration::from_secs(0));
        let duration_ms = duration.as_millis() as u64;

        Ok(DeleteResult {
            batch_id,
            files_deleted,
            total_bytes_freed,
            duration_ms,
            errors,
            trash_path,
        })
    }

    fn delete_single_file(
        &self,
        file_path: &str,
        batch_id: &str,
        db: &Database,
    ) -> OpsResult<(u64, Option<String>)> {
        let path = Path::new(file_path);

        if !path.exists() {
            return Err(OpsError::DeleteError(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        let file_size = fs::metadata(path)?.len();

        // Determine deletion method
        let (deleted_path, trash_path) = if self.config.use_trash && !self.config.permanent_delete {
            self.move_to_trash(path)?
        } else {
            self.permanent_delete(path)?
        };

        // Log the action
        self.log_delete_action(file_path, &deleted_path, batch_id, db)?;

        Ok((file_size, trash_path))
    }

    fn move_to_trash(&self, path: &Path) -> OpsResult<(String, Option<String>)> {
        let trash_dir = self.get_trash_directory()?;
        let filename = path
            .file_name()
            .ok_or_else(|| OpsError::DeleteError("Invalid file path".to_string()))?
            .to_string_lossy();

        let mut trash_path = trash_dir.join(&*filename);

        // Handle conflicts by appending " (n)" suffix
        let mut counter = 1;
        while trash_path.exists() {
            let stem = path
                .file_stem()
                .ok_or_else(|| OpsError::DeleteError("Invalid file name".to_string()))?
                .to_string_lossy();
            let extension = path
                .extension()
                .map(|ext| format!(".{}", ext.to_string_lossy()))
                .unwrap_or_default();

            trash_path = trash_dir.join(format!("{} ({}){}", stem, counter, extension));
            counter += 1;
        }

        // Move to trash
        fs::rename(path, &trash_path)
            .map_err(|e| OpsError::DeleteError(format!("Failed to move to trash: {}", e)))?;

        Ok((
            trash_path.to_string_lossy().to_string(),
            Some(trash_dir.to_string_lossy().to_string()),
        ))
    }

    fn permanent_delete(&self, path: &Path) -> OpsResult<(String, Option<String>)> {
        fs::remove_file(path)
            .map_err(|e| OpsError::DeleteError(format!("Failed to delete file: {}", e)))?;

        Ok((path.to_string_lossy().to_string(), None))
    }

    fn get_trash_directory(&self) -> OpsResult<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            // Windows Recycle Bin
            if let Some(user_profile) = std::env::var_os("USERPROFILE") {
                let recycle_bin = PathBuf::from(user_profile)
                    .join("AppData")
                    .join("Local")
                    .join("Microsoft")
                    .join("Windows")
                    .join("Explorer");
                if !recycle_bin.exists() {
                    fs::create_dir_all(&recycle_bin).map_err(|e| {
                        OpsError::DeleteError(format!(
                            "Failed to create recycle bin directory: {}",
                            e
                        ))
                    })?;
                }
                Ok(recycle_bin)
            } else {
                Err(OpsError::DeleteError(
                    "Cannot determine user profile directory".to_string(),
                ))
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS Trash
            if let Some(home) = dirs::home_dir() {
                let trash = home.join(".Trash");
                if !trash.exists() {
                    fs::create_dir_all(&trash).map_err(|e| {
                        OpsError::DeleteError(format!("Failed to create trash directory: {}", e))
                    })?;
                }
                Ok(trash)
            } else {
                Err(OpsError::DeleteError(
                    "Cannot determine home directory".to_string(),
                ))
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux Trash
            if let Some(home) = dirs::home_dir() {
                let trash = home
                    .join(".local")
                    .join("share")
                    .join("Trash")
                    .join("files");
                if !trash.exists() {
                    fs::create_dir_all(&trash).map_err(|e| {
                        OpsError::DeleteError(format!("Failed to create trash directory: {}", e))
                    })?;
                }
                Ok(trash)
            } else {
                Err(OpsError::DeleteError(
                    "Cannot determine home directory".to_string(),
                ))
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            // Fallback for other systems
            if let Some(home) = dirs::home_dir() {
                let trash = home.join(".trash");
                if !trash.exists() {
                    fs::create_dir_all(&trash).map_err(|e| {
                        OpsError::DeleteError(format!("Failed to create trash directory: {}", e))
                    })?;
                }
                Ok(trash)
            } else {
                Err(OpsError::DeleteError(
                    "Cannot determine home directory".to_string(),
                ))
            }
        }
    }

    fn log_delete_action(
        &self,
        src_path: &str,
        dst_path: &str,
        batch_id: &str,
        db: &Database,
    ) -> OpsResult<()> {
        // Find file_id in database
        let file_id = self.get_file_id_from_path(src_path, db)?;

        let action = NewAction {
            file_id,
            action: ActionType::Delete,
            batch_id: Some(batch_id.to_string()),
            src_path: Some(src_path.to_string()),
            dst_path: Some(dst_path.to_string()),
            origin: Some("delete_manager".to_string()),
            note: None,
        };

        db.insert_action(&action)
            .map_err(|e| OpsError::DeleteError(format!("Failed to log action: {}", e)))?;

        Ok(())
    }

    fn get_file_id_from_path(&self, path: &str, db: &Database) -> OpsResult<i64> {
        db.get_file_id_by_path(path)
            .map_err(|e| OpsError::DeleteError(format!("Failed to lookup file ID: {}", e)))?
            .ok_or_else(|| OpsError::DeleteError(format!("File not found in database: {}", path)))
    }

    fn generate_batch_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_millis();

        format!("delete_{}", timestamp)
    }

    pub fn get_delete_candidates(&self, db: &Database) -> OpsResult<Vec<DeleteCandidate>> {
        // This would query the database for files eligible for deletion
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    pub fn filter_archive_candidates(
        &self,
        candidates: Vec<DeleteCandidate>,
    ) -> Vec<DeleteCandidate> {
        let cutoff_date = Utc::now() - Duration::days(self.config.archive_age_threshold_days);

        candidates
            .into_iter()
            .filter(|candidate| {
                if candidate.is_archive {
                    if let Some(archive_age) = candidate.archive_age_days {
                        archive_age >= self.config.archive_age_threshold_days
                    } else {
                        candidate.age_days >= self.config.archive_age_threshold_days
                    }
                } else {
                    false // Only preselect archives
                }
            })
            .collect()
    }

    pub fn update_config(&mut self, config: DeleteConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &DeleteConfig {
        &self.config
    }

    pub fn set_permanent_delete(&mut self, permanent: bool) {
        self.config.permanent_delete = permanent;
        if permanent {
            self.config.use_trash = false;
        }
    }

    pub fn set_use_trash(&mut self, use_trash: bool) {
        self.config.use_trash = use_trash;
        if use_trash {
            self.config.permanent_delete = false;
        }
    }
}

impl Default for DeleteManager {
    fn default() -> Self {
        Self::new()
    }
}
