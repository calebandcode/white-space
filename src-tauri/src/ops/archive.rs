use crate::db::Database;
use crate::models::{ActionType, NewAction};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::space::SpaceManager;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ArchiveConfig {
    pub base_path: PathBuf,
    pub date_format: String,
    pub free_space_buffer: f64,  // Percentage (5.0 = 5%)
    pub progress_threshold: u64, // Bytes (500MB)
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            base_path: Self::get_default_archive_path(),
            date_format: "%Y-%m-%d".to_string(),
            free_space_buffer: 5.0,
            progress_threshold: 500 * 1024 * 1024, // 500MB
        }
    }
}

impl ArchiveConfig {
    fn get_default_archive_path() -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            #[cfg(target_os = "windows")]
            {
                home.join("Archive").join("WhiteSpace")
            }
            #[cfg(not(target_os = "windows"))]
            {
                home.join("Archive").join("White Space")
            }
        } else {
            PathBuf::from("./Archive")
        }
    }

    pub fn get_daily_path(&self) -> PathBuf {
        let today = Utc::now().format(&self.date_format).to_string();
        self.base_path.join(today)
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveProgress {
    pub file_path: String,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone)]
pub struct ArchiveResult {
    pub batch_id: String,
    pub files_archived: usize,
    pub total_bytes: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

pub struct ArchiveManager {
    config: ArchiveConfig,
    space_manager: SpaceManager,
}

impl ArchiveManager {
    pub fn new() -> Self {
        Self {
            config: ArchiveConfig::default(),
            space_manager: SpaceManager::new(),
        }
    }

    pub fn archive_files(
        &mut self,
        file_paths: Vec<String>,
        db: &Database,
    ) -> OpsResult<ArchiveResult> {
        let start_time = SystemTime::now();
        let batch_id = self.generate_batch_id();
        let archive_path = self.config.get_daily_path();

        // Preflight checks
        self.preflight_checks(&file_paths, &archive_path)?;

        let mut files_archived = 0;
        let mut total_bytes = 0u64;
        let mut errors = Vec::new();

        // Create archive directory
        fs::create_dir_all(&archive_path).map_err(|e| {
            OpsError::ArchiveError(format!("Failed to create archive directory: {}", e))
        })?;

        for file_path in file_paths {
            match self.archive_single_file(&file_path, &archive_path, &batch_id, db) {
                Ok(bytes) => {
                    files_archived += 1;
                    total_bytes += bytes;
                }
                Err(e) => {
                    errors.push(format!("Failed to archive {}: {}", file_path, e));
                }
            }
        }

        let duration = start_time
            .elapsed()
            .unwrap_or(std::time::Duration::from_secs(0));
        let duration_ms = duration.as_millis() as u64;

        Ok(ArchiveResult {
            batch_id,
            files_archived,
            total_bytes,
            duration_ms,
            errors,
        })
    }

    fn preflight_checks(&self, file_paths: &[String], archive_path: &Path) -> OpsResult<()> {
        // Check if archive directory can be created
        if let Some(parent) = archive_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    OpsError::ArchiveError(format!("Failed to create parent directory: {}", e))
                })?;
            }
        }

        // Check permissions
        self.check_permissions(archive_path)?;

        // Calculate total size and check free space
        let total_size = self.calculate_total_size(file_paths)?;
        self.check_free_space(archive_path, total_size)?;

        // Verify all source files exist
        for file_path in file_paths {
            if !Path::new(file_path).exists() {
                return Err(OpsError::ArchiveError(format!(
                    "Source file does not exist: {}",
                    file_path
                )));
            }
        }

        Ok(())
    }

    fn check_permissions(&self, path: &Path) -> OpsResult<()> {
        // Check if we can write to the directory
        if path.exists() {
            let metadata = fs::metadata(path).map_err(|e| {
                OpsError::ArchiveError(format!("Failed to read directory metadata: {}", e))
            })?;

            if !metadata.permissions().readonly() {
                return Ok(());
            }
        }

        // Try to create a test file
        let test_file = path.join(".test_write_permission");
        match fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                Ok(())
            }
            Err(e) => Err(OpsError::ArchiveError(format!(
                "No write permission to archive directory: {}",
                e
            ))),
        }
    }

    fn calculate_total_size(&self, file_paths: &[String]) -> OpsResult<u64> {
        let mut total = 0u64;

        for file_path in file_paths {
            let metadata = fs::metadata(file_path).map_err(|e| {
                OpsError::ArchiveError(format!(
                    "Failed to read file metadata for {}: {}",
                    file_path, e
                ))
            })?;
            total += metadata.len();
        }

        Ok(total)
    }

    fn check_free_space(&self, archive_path: &Path, required_bytes: u64) -> OpsResult<()> {
        let available_space = self.space_manager.get_available_space(archive_path)?;
        let buffer_bytes = (required_bytes as f64 * self.config.free_space_buffer / 100.0) as u64;
        let required_with_buffer = required_bytes + buffer_bytes;

        if available_space < required_with_buffer {
            return Err(OpsError::ArchiveError(format!(
                "Insufficient disk space. Required: {} bytes, Available: {} bytes",
                required_with_buffer, available_space
            )));
        }

        Ok(())
    }

    fn archive_single_file(
        &self,
        source_path: &str,
        archive_dir: &Path,
        batch_id: &str,
        db: &Database,
    ) -> OpsResult<u64> {
        let source = Path::new(source_path);
        let filename = source
            .file_name()
            .ok_or_else(|| OpsError::ArchiveError("Invalid file path".to_string()))?
            .to_string_lossy();

        let mut dest_path = archive_dir.join(&*filename);

        // Handle conflicts by appending " (n)" suffix
        let mut counter = 1;
        while dest_path.exists() {
            let stem = source
                .file_stem()
                .ok_or_else(|| OpsError::ArchiveError("Invalid file name".to_string()))?
                .to_string_lossy();
            let extension = source
                .extension()
                .map(|ext| format!(".{}", ext.to_string_lossy()))
                .unwrap_or_default();

            dest_path = archive_dir.join(format!("{} ({}){}", stem, counter, extension));
            counter += 1;
        }

        // Get file size for progress tracking
        let file_size = fs::metadata(source)?.len();

        // Try to move first (fastest)
        match fs::rename(source, &dest_path) {
            Ok(_) => {
                // Success - log the action
                self.log_archive_action(source_path, &dest_path.to_string_lossy(), batch_id, db)?;
                Ok(file_size)
            }
            Err(_) => {
                // Cross-volume move failed, fallback to copy + delete
                self.copy_and_delete(source, &dest_path, file_size)?;
                self.log_archive_action(source_path, &dest_path.to_string_lossy(), batch_id, db)?;
                Ok(file_size)
            }
        }
    }

    fn copy_and_delete(&self, source: &Path, dest: &Path, file_size: u64) -> OpsResult<()> {
        // Copy file
        fs::copy(source, dest)
            .map_err(|e| OpsError::ArchiveError(format!("Failed to copy file: {}", e)))?;

        // Force sync to ensure data is written
        self.sync_file(dest)?;

        // Verify copy
        self.verify_copy(source, dest)?;

        // Delete original
        fs::remove_file(source).map_err(|e| {
            OpsError::ArchiveError(format!("Failed to delete original file: {}", e))
        })?;

        Ok(())
    }

    fn sync_file(&self, path: &Path) -> OpsResult<()> {
        // On Unix systems, we can use fsync
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let file = fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_SYNC)
                .open(path)
                .map_err(|e| OpsError::ArchiveError(format!("Failed to sync file: {}", e)))?;
            file.sync_all()
                .map_err(|e| OpsError::ArchiveError(format!("Failed to sync file: {}", e)))?;
        }

        // On Windows, we rely on the OS
        #[cfg(windows)]
        {
            // Windows handles this automatically
        }

        Ok(())
    }

    fn verify_copy(&self, source: &Path, dest: &Path) -> OpsResult<()> {
        let source_size = fs::metadata(source)?.len();
        let dest_size = fs::metadata(dest)?.len();

        if source_size != dest_size {
            return Err(OpsError::ArchiveError(format!(
                "Copy verification failed: source size {} != dest size {}",
                source_size, dest_size
            )));
        }

        Ok(())
    }

    fn log_archive_action(
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
            action: ActionType::Archive,
            batch_id: Some(batch_id.to_string()),
            src_path: Some(src_path.to_string()),
            dst_path: Some(dst_path.to_string()),
            origin: Some("archive_manager".to_string()),
            note: None,
        };

        db.insert_action(&action)
            .map_err(|e| OpsError::ArchiveError(format!("Failed to log action: {}", e)))?;
        db.update_file_location(file_id, dst_path).map_err(|e| {
            OpsError::ArchiveError(format!("Failed to update file location: {}", e))
        })?;

        Ok(())
    }

    fn get_file_id_from_path(&self, path: &str, db: &Database) -> OpsResult<i64> {
        db.get_file_id_by_path(path)
            .map_err(|e| OpsError::ArchiveError(format!("Failed to lookup file ID: {}", e)))?
            .ok_or_else(|| OpsError::ArchiveError(format!("File not found in database: {}", path)))
    }

    fn generate_batch_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::from_secs(0))
            .as_millis();

        format!("archive_{}", timestamp)
    }

    pub fn update_config(&mut self, config: ArchiveConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &ArchiveConfig {
        &self.config
    }
}

impl Default for ArchiveManager {
    fn default() -> Self {
        Self::new()
    }
}
