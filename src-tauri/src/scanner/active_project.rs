use crate::models::{ActionType, NewMetric};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DevRepo {
    pub path: PathBuf,
    pub git_root: PathBuf,
    pub keyword_flags: Vec<String>,
    pub last_activity: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct RecentBurst {
    pub directory: PathBuf,
    pub modified_count: u32,
    pub time_window_hours: u32,
    pub is_burst: bool,
}

pub struct ActiveProjectDetector {
    keyword_patterns: Vec<String>,
    burst_threshold: u32,
    burst_window_hours: u32,
}

impl ActiveProjectDetector {
    pub fn new() -> Self {
        let keyword_patterns = vec![
            "current".to_string(),
            "project".to_string(),
            "active".to_string(),
            "wip".to_string(),
            "final".to_string(),
        ];

        Self {
            keyword_patterns,
            burst_threshold: 3,
            burst_window_hours: 72,
        }
    }

    pub fn detect_dev_repos(&self, roots: &[String]) -> Vec<DevRepo> {
        let mut repos = Vec::new();

        for root in roots {
            if let Ok(repos_in_root) = self.scan_for_git_repos(&PathBuf::from(root)) {
                repos.extend(repos_in_root);
            }
        }

        repos
    }

    fn scan_for_git_repos(&self, path: &Path) -> Result<Vec<DevRepo>, Box<dyn std::error::Error>> {
        let mut repos = Vec::new();
        self.walk_for_git_repos(path, &mut repos)?;
        Ok(repos)
    }

    fn walk_for_git_repos(
        &self,
        path: &Path,
        repos: &mut Vec<DevRepo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !path.is_dir() {
            return Ok(());
        }

        // Check if current directory is a git repo
        if path.join(".git").exists() {
            let repo = self.analyze_git_repo(path)?;
            repos.push(repo);
            return Ok(());
        }

        // Recursively check subdirectories (with depth limit)
        let entries = fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip .git directories
            if entry_path.file_name().unwrap_or_default() == ".git" {
                continue;
            }

            // Skip node_modules and other common skip directories
            if self.should_skip_directory(&entry_path) {
                continue;
            }

            if entry_path.is_dir() {
                self.walk_for_git_repos(&entry_path, repos)?;
            }
        }

        Ok(())
    }

    fn should_skip_directory(&self, path: &Path) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        matches!(
            name.as_ref(),
            ".git" | "node_modules" | ".DS_Store" | "Thumbs.db"
        )
    }

    fn analyze_git_repo(&self, repo_path: &Path) -> Result<DevRepo, Box<dyn std::error::Error>> {
        let keyword_flags = self.detect_keyword_flags(repo_path);
        let last_activity = self.get_last_git_activity(repo_path)?;
        let is_active = self.is_repo_active(repo_path, &last_activity)?;

        Ok(DevRepo {
            path: repo_path.to_path_buf(),
            git_root: repo_path.to_path_buf(),
            keyword_flags,
            last_activity,
            is_active,
        })
    }

    fn detect_keyword_flags(&self, repo_path: &Path) -> Vec<String> {
        let mut flags = Vec::new();
        let repo_name = repo_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        for pattern in &self.keyword_patterns {
            if repo_name.contains(pattern) {
                flags.push(pattern.clone());
            }
        }

        flags
    }

    fn get_last_git_activity(
        &self,
        repo_path: &Path,
    ) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
        // Try to get the last commit date from git
        // For now, we'll use the directory's modification time as a fallback
        let metadata = fs::metadata(repo_path)?;
        let modified = metadata.modified()?;
        let duration = modified.duration_since(std::time::UNIX_EPOCH)?;

        Ok(DateTime::from_timestamp(duration.as_secs() as i64, 0).unwrap_or_else(Utc::now))
    }

    fn is_repo_active(
        &self,
        repo_path: &Path,
        last_activity: &DateTime<Utc>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Check if there's been recent activity (within last 7 days)
        let week_ago = Utc::now() - Duration::days(7);
        Ok(last_activity > &week_ago)
    }

    pub fn detect_recent_burst(
        &self,
        directory: &Path,
    ) -> Result<RecentBurst, Box<dyn std::error::Error>> {
        let mut modified_count = 0u32;
        let cutoff_time = Utc::now() - Duration::hours(self.burst_window_hours as i64);

        self.count_recent_modifications(directory, &cutoff_time, &mut modified_count)?;

        let is_burst = modified_count >= self.burst_threshold;

        Ok(RecentBurst {
            directory: directory.to_path_buf(),
            modified_count,
            time_window_hours: self.burst_window_hours,
            is_burst,
        })
    }

    fn count_recent_modifications(
        &self,
        path: &Path,
        cutoff_time: &DateTime<Utc>,
        count: &mut u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !path.is_dir() {
            return Ok(());
        }

        let entries = fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();

            // Skip hidden files and directories
            if entry_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with('.')
            {
                continue;
            }

            // Skip common skip directories
            if self.should_skip_directory(&entry_path) {
                continue;
            }

            if entry_path.is_file() {
                if let Ok(metadata) = fs::metadata(&entry_path) {
                    if let Ok(modified) = metadata.modified() {
                        let duration = modified.duration_since(std::time::UNIX_EPOCH)?;
                        if let Some(modified_time) =
                            DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        {
                            if modified_time > *cutoff_time {
                                *count += 1;
                            }
                        }
                    }
                }
            } else if entry_path.is_dir() {
                self.count_recent_modifications(&entry_path, cutoff_time, count)?;
            }
        }

        Ok(())
    }

    pub fn get_default_scan_roots() -> Vec<String> {
        let mut roots = Vec::new();

        // Try to get user directories
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Desktop").to_string_lossy().to_string());
            roots.push(home.join("Downloads").to_string_lossy().to_string());
            roots.push(home.join("Pictures").to_string_lossy().to_string());
            roots.push(home.join("Documents").to_string_lossy().to_string());
        }

        // Add common development directories
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Projects").to_string_lossy().to_string());
            roots.push(home.join("Code").to_string_lossy().to_string());
            roots.push(home.join("dev").to_string_lossy().to_string());
        }

        roots
            .into_iter()
            .filter(|path| Path::new(path).exists())
            .collect()
    }

    pub fn analyze_project_activity(&self, repos: &[DevRepo]) -> HashMap<String, u32> {
        let mut activity_stats = HashMap::new();

        for repo in repos {
            let category = if repo.is_active { "active" } else { "inactive" };

            *activity_stats.entry(category.to_string()).or_insert(0) += 1;

            // Count keyword flags
            for flag in &repo.keyword_flags {
                *activity_stats.entry(format!("flag_{}", flag)).or_insert(0) += 1;
            }
        }

        activity_stats
    }
}

impl Default for ActiveProjectDetector {
    fn default() -> Self {
        Self::new()
    }
}




