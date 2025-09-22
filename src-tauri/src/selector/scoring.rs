use crate::models::{ActionType, File};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ScoreFactors {
    pub size_bytes: u64,
    pub age_days: f64,
    pub is_duplicate: bool,
    pub is_unopened: bool,
    pub has_keyword_flag: bool,
    pub in_git_repo: bool,
    pub recent_sibling_burst: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Candidate {
    pub file_id: i64,
    pub path: String,
    pub parent_dir: String,
    pub size_bytes: u64,
    pub reason: String,
    pub score: f64,
    pub confidence: f64,
    pub preview_hint: String,
    pub age_days: f64,
}

pub struct FileScorer {
    max_size_bytes: u64,
    max_age_days: f64,
}

impl FileScorer {
    pub fn new() -> Self {
        Self {
            max_size_bytes: 2 * 1024 * 1024 * 1024, // 2GB
            max_age_days: 365.0,                    // 1 year
        }
    }

    pub fn calculate_score(&self, file: &File, factors: &ScoreFactors) -> f64 {
        // Normalize size (0-1 scale, log scale for better distribution)
        let norm_size = self.normalize_size(factors.size_bytes);

        // Normalize age (0-1 scale, older = higher score)
        let norm_age = self.normalize_age(factors.age_days);

        // Base score components
        let size_score = 0.45 * norm_size;
        let age_score = 0.25 * norm_age;
        let duplicate_score = if factors.is_duplicate { 0.20 } else { 0.0 };
        let unopened_score = if factors.is_unopened { 0.10 } else { 0.0 };

        // Penalty components (negative)
        let keyword_penalty = if factors.has_keyword_flag { -0.30 } else { 0.0 };
        let git_penalty = if factors.in_git_repo { -0.80 } else { 0.0 };
        let burst_penalty = if factors.recent_sibling_burst {
            -0.70
        } else {
            0.0
        };

        // Calculate final score
        let score = size_score
            + age_score
            + duplicate_score
            + unopened_score
            + keyword_penalty
            + git_penalty
            + burst_penalty;

        // Clamp score to [0, 1] range
        score.max(0.0).min(1.0)
    }

    fn normalize_size(&self, size_bytes: u64) -> f64 {
        if size_bytes == 0 {
            return 0.0;
        }

        // Use log scale for better distribution of file sizes
        let log_size = (size_bytes as f64).ln();
        let log_max = (self.max_size_bytes as f64).ln();

        (log_size / log_max).min(1.0)
    }

    fn normalize_age(&self, age_days: f64) -> f64 {
        if age_days <= 0.0 {
            return 0.0;
        }

        // Older files get higher scores (up to max_age_days)
        (age_days / self.max_age_days).min(1.0)
    }

    pub fn calculate_confidence(&self, file: &File, factors: &ScoreFactors) -> f64 {
        let mut confidence: f64 = 0.5; // Base confidence

        // Increase confidence for clear indicators
        if factors.is_duplicate {
            confidence += 0.2;
        }

        if factors.is_unopened && factors.age_days > 30.0 {
            confidence += 0.15;
        }

        if factors.size_bytes > 100 * 1024 * 1024 {
            // 100MB
            confidence += 0.1;
        }

        // Decrease confidence for active projects
        if factors.in_git_repo {
            confidence -= 0.2;
        }

        if factors.has_keyword_flag {
            confidence -= 0.1;
        }

        if factors.recent_sibling_burst {
            confidence -= 0.15;
        }

        // Clamp confidence to [0, 1] range
        confidence.max(0.0).min(1.0)
    }

    pub fn generate_preview_hint(&self, file: &File, factors: &ScoreFactors) -> String {
        let mut hints = Vec::new();

        if factors.is_duplicate {
            hints.push("duplicate".to_string());
        }

        if factors.is_unopened {
            hints.push("unopened".to_string());
        }

        if factors.size_bytes > 100 * 1024 * 1024 {
            hints.push("large".to_string());
        }

        if factors.age_days > 30.0 {
            hints.push("old".to_string());
        }

        if factors.in_git_repo {
            hints.push("git-repo".to_string());
        }

        if factors.has_keyword_flag {
            hints.push("flagged".to_string());
        }

        if factors.recent_sibling_burst {
            hints.push("recent-activity".to_string());
        }

        if hints.is_empty() {
            "candidate".to_string()
        } else {
            hints.join(", ")
        }
    }

    pub fn extract_score_factors(&self, file: &File, context: &ScoringContext) -> ScoreFactors {
        let age_days = self.calculate_age_days(file);
        let is_duplicate = context.duplicate_files.contains(&file.id.unwrap_or(0));
        let is_unopened = file.last_opened_at.is_none() && file.accessed_at.is_none();
        let has_keyword_flag = self.has_keyword_flag(&file.path);
        let in_git_repo = context.git_repos.contains(&file.parent_dir);
        let recent_sibling_burst = context.burst_directories.contains(&file.parent_dir);

        ScoreFactors {
            size_bytes: file.size_bytes as u64,
            age_days,
            is_duplicate,
            is_unopened,
            has_keyword_flag,
            in_git_repo,
            recent_sibling_burst,
        }
    }

    pub fn calculate_age_days(&self, file: &File) -> f64 {
        let now = Utc::now();
        let reference = file
            .accessed_at
            .or(file.modified_at)
            .unwrap_or(file.last_seen_at);
        let file_time = reference;
        let duration = now.signed_duration_since(file_time);
        duration.num_days() as f64
    }

    fn has_keyword_flag(&self, path: &str) -> bool {
        let keywords = ["current", "project", "active", "wip", "final"];
        let path_lower = path.to_lowercase();

        keywords.iter().any(|keyword| path_lower.contains(keyword))
    }
}

#[derive(Debug, Clone)]
pub struct ScoringContext {
    pub duplicate_files: HashSet<i64>,
    pub git_repos: HashSet<String>,
    pub burst_directories: HashSet<String>,
}

impl ScoringContext {
    pub fn new() -> Self {
        Self {
            duplicate_files: HashSet::new(),
            git_repos: HashSet::new(),
            burst_directories: HashSet::new(),
        }
    }

    pub fn add_duplicate_files(&mut self, file_ids: Vec<i64>) {
        for file_id in file_ids {
            self.duplicate_files.insert(file_id);
        }
    }

    pub fn add_git_repos(&mut self, repo_paths: Vec<String>) {
        for path in repo_paths {
            self.git_repos.insert(path);
        }
    }

    pub fn add_burst_directories(&mut self, dir_paths: Vec<String>) {
        for path in dir_paths {
            self.burst_directories.insert(path);
        }
    }
}

impl Default for FileScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ScoringContext {
    fn default() -> Self {
        Self::new()
    }
}

// Edge case testing utilities
#[cfg(test)]
mod test_utils {
    use super::*;
    use chrono::Utc;

    pub fn create_test_file(id: i64, path: String, size_bytes: i64, age_days: i64) -> File {
        let now = Utc::now();
        let file_time = now - Duration::days(age_days);

        File {
            id: Some(id),
            path,
            parent_dir: "/test".to_string(),
            mime: Some("text/plain".to_string()),
            size_bytes,
            created_at: file_time,
            last_opened_at: None,
            sha1: Some("test_hash".to_string()),
            first_seen_at: file_time,
            last_seen_at: file_time,
            is_deleted: false,
        }
    }

    pub fn create_test_context() -> ScoringContext {
        let mut context = ScoringContext::new();
        context.add_duplicate_files(vec![1, 2, 3]);
        context.add_git_repos(vec!["/test/git-repo".to_string()]);
        context.add_burst_directories(vec!["/test/burst-dir".to_string()]);
        context
    }
}
