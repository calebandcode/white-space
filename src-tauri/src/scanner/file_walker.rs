use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub parent_dir: PathBuf,
    pub size_bytes: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
    pub accessed_at: Option<DateTime<Utc>>,
    pub mime_type: Option<String>,
}

pub struct FileWalker {
    skip_dirs: HashSet<String>,
    skip_files: HashSet<String>,
}

impl FileWalker {
    pub fn new() -> Self {
        let mut skip_dirs = HashSet::new();
        skip_dirs.insert(".git".to_string());
        skip_dirs.insert("node_modules".to_string());
        skip_dirs.insert(".DS_Store".to_string());
        skip_dirs.insert("Thumbs.db".to_string());

        let mut skip_files = HashSet::new();
        skip_files.insert(".DS_Store".to_string());
        skip_files.insert("Thumbs.db".to_string());

        Self {
            skip_dirs,
            skip_files,
        }
    }

    pub fn should_skip_dir(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| self.skip_dirs.contains(name))
            .unwrap_or(false)
    }

    pub fn should_skip_file(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| self.skip_files.contains(name))
            .unwrap_or(false)
    }

    pub fn extract_metadata(&self, file_path: &Path) -> Result<FileMetadata> {
        let metadata = fs::metadata(file_path)?;
        let parent_dir = file_path.parent().unwrap_or(Path::new("/")).to_path_buf();

        let created_at = metadata.created().ok().and_then(|t| self.to_datetime(t));
        let modified_at = metadata.modified().ok().and_then(|t| self.to_datetime(t));
        let accessed_at = metadata.accessed().ok().and_then(|t| self.to_datetime(t));

        Ok(FileMetadata {
            path: file_path.to_path_buf(),
            parent_dir,
            size_bytes: metadata.len(),
            created_at,
            modified_at,
            accessed_at,
            mime_type: self.detect_mime_type(file_path),
        })
    }

    fn to_datetime(&self, time: std::time::SystemTime) -> Option<DateTime<Utc>> {
        time.duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|dur| DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos()))
    }

    fn detect_mime_type(&self, file_path: &Path) -> Option<String> {
        let extension = file_path.extension()?.to_string_lossy().to_lowercase();
        match extension.as_str() {
            "txt" => Some("text/plain".to_string()),
            "md" => Some("text/markdown".to_string()),
            "html" => Some("text/html".to_string()),
            "css" => Some("text/css".to_string()),
            "js" => Some("application/javascript".to_string()),
            "json" => Some("application/json".to_string()),
            "pdf" => Some("application/pdf".to_string()),
            "jpg" | "jpeg" => Some("image/jpeg".to_string()),
            "png" => Some("image/png".to_string()),
            "gif" => Some("image/gif".to_string()),
            "mp4" => Some("video/mp4".to_string()),
            "mp3" => Some("audio/mpeg".to_string()),
            "zip" => Some("application/zip".to_string()),
            "tar" => Some("application/x-tar".to_string()),
            "gz" => Some("application/gzip".to_string()),
            _ => None,
        }
    }
}

impl Default for FileWalker {
    fn default() -> Self {
        Self::new()
    }
}
