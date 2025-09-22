#[cfg(test)]
mod tests {
    use super::*;
    use super::scoring::*;
    use crate::db::Database;
    use chrono::{Utc, Duration};
    use std::collections::HashSet;

    fn create_test_file(id: i64, path: String, size_bytes: i64, age_days: i64) -> File {
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

    fn create_test_context() -> ScoringContext {
        let mut context = ScoringContext::new();
        context.add_duplicate_files(vec![1, 2, 3]);
        context.add_git_repos(vec!["/test/git-repo".to_string()]);
        context.add_burst_directories(vec!["/test/burst-dir".to_string()]);
        context
    }

    #[test]
    fn test_score_calculation_basic() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/file.txt".to_string(), 1024, 30);
        
        let factors = ScoreFactors {
            size_bytes: 1024,
            age_days: 30.0,
            is_duplicate: false,
            is_unopened: true,
            has_keyword_flag: false,
            in_git_repo: false,
            recent_sibling_burst: false,
        };
        
        let score = scorer.calculate_score(&file, &factors);
        
        // Should have positive score from size, age, and unopened bonus
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_score_calculation_with_penalties() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/current-project/file.txt".to_string(), 1024, 30);
        
        let factors = ScoreFactors {
            size_bytes: 1024,
            age_days: 30.0,
            is_duplicate: false,
            is_unopened: true,
            has_keyword_flag: true,  // Penalty
            in_git_repo: true,       // Penalty
            recent_sibling_burst: true, // Penalty
        };
        
        let score = scorer.calculate_score(&file, &factors);
        
        // Should have lower score due to penalties
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_score_calculation_duplicate_bonus() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/file.txt".to_string(), 1024, 30);
        
        let factors = ScoreFactors {
            size_bytes: 1024,
            age_days: 30.0,
            is_duplicate: true,  // Bonus
            is_unopened: true,  // Bonus
            has_keyword_flag: false,
            in_git_repo: false,
            recent_sibling_burst: false,
        };
        
        let score = scorer.calculate_score(&file, &factors);
        
        // Should have higher score due to duplicate and unopened bonuses
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_score_normalization_edge_cases() {
        let scorer = FileScorer::new();
        
        // Test zero size
        let norm_size_zero = scorer.normalize_size(0);
        assert_eq!(norm_size_zero, 0.0);
        
        // Test very large size
        let norm_size_large = scorer.normalize_size(10 * 1024 * 1024 * 1024); // 10GB
        assert!(norm_size_large <= 1.0);
        
        // Test zero age
        let norm_age_zero = scorer.normalize_age(0.0);
        assert_eq!(norm_age_zero, 0.0);
        
        // Test very old age
        let norm_age_old = scorer.normalize_age(1000.0); // 1000 days
        assert!(norm_age_old <= 1.0);
    }

    #[test]
    fn test_confidence_calculation() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/file.txt".to_string(), 1024, 30);
        
        let factors = ScoreFactors {
            size_bytes: 200 * 1024 * 1024, // 200MB
            age_days: 60.0,
            is_duplicate: true,
            is_unopened: true,
            has_keyword_flag: false,
            in_git_repo: false,
            recent_sibling_burst: false,
        };
        
        let confidence = scorer.calculate_confidence(&file, &factors);
        
        // Should have high confidence due to multiple positive indicators
        assert!(confidence > 0.5);
        assert!(confidence <= 1.0);
    }

    #[test]
    fn test_confidence_calculation_low() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/current-project/file.txt".to_string(), 1024, 5);
        
        let factors = ScoreFactors {
            size_bytes: 1024,
            age_days: 5.0,
            is_duplicate: false,
            is_unopened: false,
            has_keyword_flag: true,
            in_git_repo: true,
            recent_sibling_burst: true,
        };
        
        let confidence = scorer.calculate_confidence(&file, &factors);
        
        // Should have low confidence due to multiple negative indicators
        assert!(confidence < 0.5);
        assert!(confidence >= 0.0);
    }

    #[test]
    fn test_preview_hint_generation() {
        let scorer = FileScorer::new();
        let file = create_test_file(1, "/test/file.txt".to_string(), 1024, 30);
        
        let factors = ScoreFactors {
            size_bytes: 200 * 1024 * 1024, // 200MB
            age_days: 60.0,
            is_duplicate: true,
            is_unopened: true,
            has_keyword_flag: false,
            in_git_repo: false,
            recent_sibling_burst: false,
        };
        
        let hint = scorer.generate_preview_hint(&file, &factors);
        
        // Should contain multiple hints
        assert!(hint.contains("duplicate"));
        assert!(hint.contains("unopened"));
        assert!(hint.contains("large"));
        assert!(hint.contains("old"));
    }

    #[test]
    fn test_keyword_flag_detection() {
        let scorer = FileScorer::new();
        
        // Test positive cases
        assert!(scorer.has_keyword_flag("/test/current-project/file.txt"));
        assert!(scorer.has_keyword_flag("/test/wip-project/file.txt"));
        assert!(scorer.has_keyword_flag("/test/active-project/file.txt"));
        assert!(scorer.has_keyword_flag("/test/final-project/file.txt"));
        
        // Test negative cases
        assert!(!scorer.has_keyword_flag("/test/old-project/file.txt"));
        assert!(!scorer.has_keyword_flag("/test/document.txt"));
    }

    #[test]
    fn test_age_calculation() {
        let scorer = FileScorer::new();
        let now = Utc::now();
        let file_time = now - Duration::days(30);
        
        let file = File {
            id: Some(1),
            path: "/test/file.txt".to_string(),
            parent_dir: "/test".to_string(),
            mime: Some("text/plain".to_string()),
            size_bytes: 1024,
            created_at: file_time,
            last_opened_at: None,
            sha1: Some("test_hash".to_string()),
            first_seen_at: file_time,
            last_seen_at: file_time,
            is_deleted: false,
        };
        
        let age_days = scorer.calculate_age_days(&file);
        assert!((age_days - 30.0).abs() < 1.0); // Allow for small time differences
    }

    #[test]
    fn test_bucket_classification() {
        let selector = FileSelector::new();
        
        // Test screenshot detection
        let screenshot_file = create_test_file(1, "/Users/test/Screenshots/screenshot.png".to_string(), 1024, 30);
        assert!(selector.is_screenshot(&screenshot_file));
        
        let screenshot_file2 = create_test_file(2, "/Users/test/screenshot_2024.png".to_string(), 1024, 30);
        assert!(selector.is_screenshot(&screenshot_file2));
        
        // Test big download detection
        let big_download = create_test_file(3, "/Users/test/Downloads/large_file.zip".to_string(), 150 * 1024 * 1024, 45);
        assert!(selector.is_big_download(&big_download));
        
        // Test old desktop detection
        let old_desktop = create_test_file(4, "/Users/test/Desktop/old_file.txt".to_string(), 1024, 20);
        assert!(selector.is_old_desktop(&old_desktop));
    }

    #[test]
    fn test_duplicate_detection() {
        let selector = FileSelector::new();
        let context = create_test_context();
        
        let duplicate_file = create_test_file(1, "/test/file.txt".to_string(), 1024, 30);
        assert!(selector.is_duplicate(&duplicate_file, &context));
        
        let non_duplicate_file = create_test_file(4, "/test/file2.txt".to_string(), 1024, 30);
        assert!(!selector.is_duplicate(&non_duplicate_file, &context));
        
        // Test large file exclusion
        let large_file = create_test_file(5, "/test/large_file.txt".to_string(), 3 * 1024 * 1024 * 1024, 30);
        assert!(!selector.is_duplicate(&large_file, &context));
    }

    #[test]
    fn test_duplicate_finding() {
        let selector = FileSelector::new();
        
        let files = vec![
            create_test_file_with_sha1(1, "/test/file1.txt".to_string(), 1024, 30, "hash1"),
            create_test_file_with_sha1(2, "/test/file2.txt".to_string(), 2048, 30, "hash1"), // Duplicate
            create_test_file_with_sha1(3, "/test/file3.txt".to_string(), 1024, 30, "hash2"),
            create_test_file_with_sha1(4, "/test/file4.txt".to_string(), 1024, 30, "hash1"), // Duplicate
        ];
        
        let duplicates = selector.find_duplicates(&files);
        assert_eq!(duplicates.len(), 3); // file1, file2, file4
        assert!(duplicates.contains(&1));
        assert!(duplicates.contains(&2));
        assert!(duplicates.contains(&4));
        assert!(!duplicates.contains(&3));
    }

    #[test]
    fn test_git_repo_detection() {
        let selector = FileSelector::new();
        
        let files = vec![
            create_test_file(1, "/test/project/.git/config".to_string(), 1024, 30),
            create_test_file(2, "/test/project/src/main.rs".to_string(), 1024, 30),
            create_test_file(3, "/test/other/file.txt".to_string(), 1024, 30),
        ];
        
        let git_repos = selector.find_git_repos(&files);
        assert_eq!(git_repos.len(), 1);
        assert!(git_repos.contains(&"/test/project".to_string()));
    }

    #[test]
    fn test_burst_detection() {
        let selector = FileSelector::new();
        let now = Utc::now();
        
        let files = vec![
            create_test_file_with_time(1, "/test/burst-dir/file1.txt".to_string(), 1024, now - Duration::hours(10)),
            create_test_file_with_time(2, "/test/burst-dir/file2.txt".to_string(), 1024, now - Duration::hours(20)),
            create_test_file_with_time(3, "/test/burst-dir/file3.txt".to_string(), 1024, now - Duration::hours(30)),
            create_test_file_with_time(4, "/test/other-dir/file4.txt".to_string(), 1024, now - Duration::hours(10)),
        ];
        
        let burst_dirs = selector.find_burst_directories(&files);
        assert_eq!(burst_dirs.len(), 1);
        assert!(burst_dirs.contains(&"/test/burst-dir".to_string()));
    }

    #[test]
    fn test_candidate_selection() {
        let selector = FileSelector::new();
        let context = create_test_context();
        
        let buckets = FileBucket {
            screenshots: vec![create_test_file(1, "/test/screenshot.png".to_string(), 1024, 30)],
            big_downloads: vec![create_test_file(2, "/test/large.zip".to_string(), 150 * 1024 * 1024, 45)],
            old_desktop: vec![create_test_file(3, "/test/old.txt".to_string(), 1024, 20)],
            duplicates: vec![create_test_file(4, "/test/duplicate.txt".to_string(), 1024, 30)],
        };
        
        let candidates = selector.select_candidates(&buckets, &context, 10);
        
        // Should have candidates from all buckets
        assert!(!candidates.is_empty());
        assert!(candidates.len() <= 10);
        
        // Check that candidates have required fields
        for candidate in &candidates {
            assert!(candidate.file_id > 0);
            assert!(!candidate.path.is_empty());
            assert!(candidate.score >= 0.0);
            assert!(candidate.score <= 1.0);
            assert!(candidate.confidence >= 0.0);
            assert!(candidate.confidence <= 1.0);
            assert!(!candidate.reason.is_empty());
        }
    }

    #[test]
    fn test_config_limits() {
        let mut selector = FileSelector::new();
        let config = BucketConfig {
            screenshots_max: 2,
            big_downloads_max: 1,
            old_desktop_max: 1,
            duplicates_max: 1,
            daily_total_max: 3,
        };
        
        selector.update_config(config);
        
        // Test that limits are respected
        let context = create_test_context();
        let buckets = FileBucket {
            screenshots: vec![
                create_test_file(1, "/test/screenshot1.png".to_string(), 1024, 30),
                create_test_file(2, "/test/screenshot2.png".to_string(), 1024, 30),
                create_test_file(3, "/test/screenshot3.png".to_string(), 1024, 30),
            ],
            big_downloads: vec![
                create_test_file(4, "/test/large1.zip".to_string(), 150 * 1024 * 1024, 45),
                create_test_file(5, "/test/large2.zip".to_string(), 150 * 1024 * 1024, 45),
            ],
            old_desktop: vec![create_test_file(6, "/test/old.txt".to_string(), 1024, 20)],
            duplicates: vec![create_test_file(7, "/test/duplicate.txt".to_string(), 1024, 30)],
        };
        
        let candidates = selector.select_candidates(&buckets, &context, 10);
        
        // Should respect daily_total_max limit
        assert!(candidates.len() <= 3);
    }

    // Helper functions for tests
    fn create_test_file_with_sha1(id: i64, path: String, size_bytes: i64, age_days: i64, sha1: &str) -> File {
        let mut file = create_test_file(id, path, size_bytes, age_days);
        file.sha1 = Some(sha1.to_string());
        file
    }

    fn create_test_file_with_time(id: i64, path: String, size_bytes: i64, last_seen: DateTime<Utc>) -> File {
        File {
            id: Some(id),
            path,
            parent_dir: "/test".to_string(),
            mime: Some("text/plain".to_string()),
            size_bytes,
            created_at: last_seen,
            last_opened_at: None,
            sha1: Some("test_hash".to_string()),
            first_seen_at: last_seen,
            last_seen_at,
            is_deleted: false,
        }
    }
}






