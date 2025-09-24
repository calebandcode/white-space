pub mod scoring;

use crate::db::Database;
use crate::models::{ActionType, File};
use chrono::{DateTime, Duration, Utc};
use scoring::{Candidate, FileScorer, ScoringContext};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BucketConfig {
    pub screenshots_max: usize,
    pub big_downloads_max: usize,
    pub old_desktop_max: usize,
    pub duplicates_max: usize,
    pub daily_total_max: usize,
}

impl Default for BucketConfig {
    fn default() -> Self {
        Self {
            screenshots_max: 5,
            big_downloads_max: 3,
            old_desktop_max: 2,
            duplicates_max: 2,
            daily_total_max: 12, // Mix cap per day
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileBucket {
    pub screenshots: Vec<File>,
    pub big_downloads: Vec<File>,
    pub old_desktop: Vec<File>,
    pub duplicates: Vec<File>,
}

pub struct FileSelector {
    scorer: FileScorer,
    config: BucketConfig,
}

impl FileSelector {
    pub fn new() -> Self {
        Self {
            scorer: FileScorer::new(),
            config: BucketConfig::default(),
        }
    }

    pub fn daily_candidates(
        &self,
        max_total: usize,
        db: &Database,
    ) -> Result<Vec<Candidate>, Box<dyn std::error::Error>> {
        // Get all files from database
        let all_files = self.get_all_files(db)?;

        // Create scoring context
        let context = self.create_scoring_context(&all_files, db)?;

        // Bucket files
        let buckets = self.bucket_files(&all_files, &context);

        // Score and select candidates
        let candidates = self.select_candidates(&buckets, &context, max_total);

        Ok(candidates)
    }

    fn get_all_files(&self, db: &Database) -> Result<Vec<File>, Box<dyn std::error::Error>> {
        db.get_all_active_files()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn create_scoring_context(
        &self,
        files: &[File],
        db: &Database,
    ) -> Result<ScoringContext, Box<dyn std::error::Error>> {
        let mut context = ScoringContext::new();

        // Find duplicate files (same SHA1)
        let duplicates = self.find_duplicates(files);
        context.add_duplicate_files(duplicates);

        // Find Git repositories
        let git_repos = self.find_git_repos(files);
        context.add_git_repos(git_repos);

        // Find directories with recent burst activity
        let burst_dirs = self.find_burst_directories(files);
        context.add_burst_directories(burst_dirs);

        Ok(context)
    }

    fn bucket_files(&self, files: &[File], context: &ScoringContext) -> FileBucket {
        let mut screenshots = Vec::new();
        let mut big_downloads = Vec::new();
        let mut old_desktop = Vec::new();
        let mut duplicates = Vec::new();

        for file in files {
            // Screenshots bucket
            if self.is_screenshot(&file) {
                screenshots.push(file.clone());
            }

            // Big Downloads bucket
            if self.is_big_download(&file) {
                big_downloads.push(file.clone());
            }

            // Old Desktop bucket
            if self.is_old_desktop(&file) {
                old_desktop.push(file.clone());
            }

            // Duplicates bucket
            if self.is_duplicate(&file, context) {
                duplicates.push(file.clone());
            }
        }

        FileBucket {
            screenshots,
            big_downloads,
            old_desktop,
            duplicates,
        }
    }

    fn path_has_segment(path_str: &str, target_lower: &str) -> bool {
        let path = Path::new(path_str);
        for comp in path.components() {
            let s = comp.as_os_str().to_string_lossy().to_lowercase();
            if s == target_lower {
                return true;
            }
        }
        false
    }

    fn filename_contains(path_str: &str, needle_lower: &str) -> bool {
        Path::new(path_str)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_lowercase().contains(needle_lower))
            .unwrap_or(false)
    }

    fn is_screenshot(&self, file: &File) -> bool {
        // Name contains "screenshot" OR parent has a segment named "screenshots"
        Self::filename_contains(&file.path, "screenshot")
            || Self::path_has_segment(&file.parent_dir, "screenshots")
    }

    fn is_big_download(&self, file: &File) -> bool {
        let in_downloads = Self::path_has_segment(&file.parent_dir, "downloads");
        let size_mb = file.size_bytes as f64 / (1024.0 * 1024.0);
        let age_days = self.scorer.calculate_age_days(file);

        // Under Downloads, size > 100MB, unopened OR age > 30d
        in_downloads && size_mb > 100.0
            && (file.last_opened_at.is_none() || age_days > 30.0)
    }

    fn is_old_desktop(&self, file: &File) -> bool {
        let in_desktop = Self::path_has_segment(&file.parent_dir, "desktop");
        let age_days = self.scorer.calculate_age_days(file);

        // Under Desktop, age > 14d
        in_desktop && age_days > 14.0
    }

    fn is_duplicate(&self, file: &File, context: &ScoringContext) -> bool {
        // Skip files > 2GB for duplicate detection (lazy)
        if file.size_bytes as u64 > 2 * 1024 * 1024 * 1024 {
            return false;
        }

        context.duplicate_files.contains(&file.id.unwrap_or(0))
    }

    fn find_duplicates(&self, files: &[File]) -> Vec<i64> {
        let mut sha1_groups: HashMap<String, Vec<i64>> = HashMap::new();

        for file in files {
            if let Some(sha1) = &file.sha1 {
                if !sha1.is_empty() {
                    sha1_groups
                        .entry(sha1.clone())
                        .or_insert_with(Vec::new)
                        .push(file.id.unwrap_or(0));
                }
            }
        }

        // Return file IDs that have duplicates (more than 1 file with same SHA1)
        sha1_groups
            .values()
            .filter(|group| group.len() > 1)
            .flatten()
            .copied()
            .collect()
    }

    fn find_git_repos(&self, files: &[File]) -> Vec<String> {
        let mut git_repos = HashSet::new();

        for file in files {
            let path = &file.path;
            if let Some(git_dir_pos) = path.find("/.git/") {
                let repo_path = &path[..git_dir_pos];
                git_repos.insert(repo_path.to_string());
            }
        }

        git_repos.into_iter().collect()
    }

    fn find_burst_directories(&self, files: &[File]) -> Vec<String> {
        let mut dir_activity: HashMap<String, Vec<DateTime<Utc>>> = HashMap::new();
        let cutoff_time = Utc::now() - Duration::hours(72);

        for file in files {
            if file.last_seen_at > cutoff_time {
                dir_activity
                    .entry(file.parent_dir.clone())
                    .or_insert_with(Vec::new)
                    .push(file.last_seen_at);
            }
        }

        // Find directories with 3+ recent modifications
        dir_activity
            .into_iter()
            .filter(|(_, timestamps)| timestamps.len() >= 3)
            .map(|(dir, _)| dir)
            .collect()
    }

    fn select_candidates(
        &self,
        buckets: &FileBucket,
        context: &ScoringContext,
        max_total: usize,
    ) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        // Select from each bucket up to their limits
        candidates.extend(self.select_from_bucket(
            &buckets.screenshots,
            context,
            self.config.screenshots_max,
            "Screenshots",
        ));
        candidates.extend(self.select_from_bucket(
            &buckets.big_downloads,
            context,
            self.config.big_downloads_max,
            "Big Downloads",
        ));
        candidates.extend(self.select_from_bucket(
            &buckets.old_desktop,
            context,
            self.config.old_desktop_max,
            "Old Desktop",
        ));
        candidates.extend(self.select_from_bucket(
            &buckets.duplicates,
            context,
            self.config.duplicates_max,
            "Duplicates",
        ));

        // Sort by score (highest first) and limit to max_total
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(max_total.min(self.config.daily_total_max));

        candidates
    }

    fn select_from_bucket(
        &self,
        files: &[File],
        context: &ScoringContext,
        max_count: usize,
        reason: &str,
    ) -> Vec<Candidate> {
        let mut scored_candidates: Vec<(Candidate, DateTime<Utc>)> = files
            .iter()
            .map(|file| {
                let factors = self.scorer.extract_score_factors(file, context);
                let score = self.scorer.calculate_score(file, &factors);
                let confidence = self.scorer.calculate_confidence(file, &factors);
                let preview_hint = self.scorer.generate_preview_hint(file, &factors);

                (
                    Candidate {
                        file_id: file.id.unwrap_or(0),
                        path: file.path.clone(),
                        parent_dir: file.parent_dir.clone(),
                        size_bytes: file.size_bytes as u64,
                        reason: reason.to_string(),
                        score,
                        confidence,
                        preview_hint,
                        age_days: factors.age_days,
                    },
                    file.last_seen_at,
                )
            })
            .collect();

        scored_candidates.sort_by(|a, b| {
            b.0.score
                .partial_cmp(&a.0.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.cmp(&a.1))
        });
        scored_candidates.truncate(max_count);

        scored_candidates
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect()
    }

    pub fn get_bucket_stats(
        &self,
        db: &Database,
    ) -> Result<HashMap<String, usize>, Box<dyn std::error::Error>> {
        let all_files = self.get_all_files(db)?;
        let context = self.create_scoring_context(&all_files, db)?;
        let buckets = self.bucket_files(&all_files, &context);

        let mut stats = HashMap::new();
        stats.insert("screenshots".to_string(), buckets.screenshots.len());
        stats.insert("big_downloads".to_string(), buckets.big_downloads.len());
        stats.insert("old_desktop".to_string(), buckets.old_desktop.len());
        stats.insert("duplicates".to_string(), buckets.duplicates.len());

        Ok(stats)
    }

    pub fn update_config(&mut self, config: BucketConfig) {
        self.config = config;
    }
}

impl Default for FileSelector {
    fn default() -> Self {
        Self::new()
    }
}
