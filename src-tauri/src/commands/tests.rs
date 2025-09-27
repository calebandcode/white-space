#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{ActionType, NewAction, NewFile, WatchedRoot};
    use chrono::Utc;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_db(&db_path).unwrap();
        db.run_migrations().unwrap();
        (temp_dir, db)
    }

    fn create_test_file(db: &Database, path: &str) -> i64 {
        let new_file = NewFile {
            path: path.to_string(),
            parent_dir: PathBuf::from(path)
                .parent()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            mime: "text/plain".to_string(),
            size_bytes: 1024,
            sha1: "test_hash".to_string(),
        };
        db.insert_file(new_file).unwrap()
    }

    fn create_test_action(db: &Database, file_id: i64, action: ActionType) -> i64 {
        let new_action = NewAction {
            file_id,
            action,
            batch_id: Some("test_batch".to_string()),
            src_path: Some("/test/src".to_string()),
            dst_path: Some("/test/dst".to_string()),
            origin: None,
            note: None,
        };
        db.insert_action(new_action).unwrap()
    }

    #[test]
    fn test_validate_path_valid() {
        let home = dirs::home_dir().unwrap();
        let test_path = home.join("Desktop").join("test.txt");

        let result = validate_path(test_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_traversal() {
        let result = validate_path("/home/../etc/passwd");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::Validation(_)));
    }

    #[test]
    fn test_validate_path_not_allowed() {
        let result = validate_path("/etc/passwd");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::Permission(_)));
    }

    #[test]
    fn test_validate_file_ids_empty() {
        let result = validate_file_ids(&[]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::Validation(_)));
    }

    #[test]
    fn test_validate_file_ids_too_many() {
        let ids: Vec<i64> = (1..=1001).collect();
        let result = validate_file_ids(&ids);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::Validation(_)));
    }

    #[test]
    fn test_validate_file_ids_invalid() {
        let result = validate_file_ids(&[0, -1, 5]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CommandError::Validation(_)));
    }

    #[test]
    fn test_validate_file_ids_valid() {
        let result = validate_file_ids(&[1, 2, 3]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sanitize_string() {
        let input = "Hello\x00World\x01Test";
        let result = sanitize_string(input);
        assert_eq!(result, "HelloWorldTest");
    }

    #[test]
    fn test_sanitize_string_length_limit() {
        let input = "a".repeat(2000);
        let result = sanitize_string(&input);
        assert_eq!(result.len(), 1000);
    }

    #[test]
    fn test_app_state_new() {
        let (temp_dir, db) = setup_test_db();
        let db_path = temp_dir.path().join("test.db");

        let app_state = AppState::new(&db_path);
        assert!(app_state.is_ok());
    }

    #[test]
    fn test_scan_roots_empty() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = scan_roots(vec![], tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_scan_roots_too_many() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let roots: Vec<String> = (1..=11).map(|i| format!("/test/root{}", i)).collect();
        let result = scan_roots(roots, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_scan_roots_invalid_path() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = scan_roots(
            vec!["/etc/passwd".to_string()],
            tauri::State::from(&app_state),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_daily_candidates_zero_max() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = daily_candidates(0, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_daily_candidates_too_large() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = daily_candidates(1001, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_filter_candidates_by_root_path_keeps_matching_files() {
        let temp_dir = TempDir::new().unwrap();
        let root_dir = temp_dir.path().join("root");
        let nested = root_dir.join("nested");
        fs::create_dir_all(&nested).unwrap();

        let matching_path = nested.join("Screenshot 2024-01-01.png");
        fs::write(&matching_path, b"content").unwrap();

        let outside_dir = temp_dir.path().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_path = outside_dir.join("Screenshot 2023-01-01.png");
        fs::write(&outside_path, b"content").unwrap();

        let mut candidates = vec![
            Candidate {
                file_id: 1,
                path: matching_path.to_string_lossy().to_string(),
                parent_dir: nested.to_string_lossy().to_string(),
                size_bytes: 1024,
                reason: "Screenshots".to_string(),
                score: 0.9,
                confidence: 0.9,
                preview_hint: "".to_string(),
                age_days: 10.0,
            },
            Candidate {
                file_id: 2,
                path: outside_path.to_string_lossy().to_string(),
                parent_dir: outside_dir.to_string_lossy().to_string(),
                size_bytes: 1024,
                reason: "Screenshots".to_string(),
                score: 0.8,
                confidence: 0.8,
                preview_hint: "".to_string(),
                age_days: 20.0,
            },
        ];

        let mut errors = Vec::new();
        let root_str = root_dir.to_string_lossy().to_string();
        filter_candidates_by_root_path(&mut candidates, &root_str, &mut errors);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, matching_path.to_string_lossy());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_archive_files_empty() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = archive_files(vec![], tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_archive_files_nonexistent() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = archive_files(vec![999], tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_NOT_FOUND"));
    }

    #[test]
    fn test_delete_files_empty() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = delete_files(vec![], true, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_delete_files_nonexistent() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = delete_files(vec![999], true, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_NOT_FOUND"));
    }

    #[test]
    fn test_get_review_items_too_large() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_review_items(366, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_get_thumbnail_invalid_id() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_thumbnail(0, 256, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_get_thumbnail_nonexistent() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_thumbnail(999, 256, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_NOT_FOUND"));
    }

    #[test]
    fn test_get_thumbnail_invalid_size() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_thumbnail(1, 0, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_get_thumbnail_too_large() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_thumbnail(1, 3000, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_tidy_hour() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            tidy_hour: Some(24),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_rolling_window() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            rolling_window_days: Some(0),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_max_candidates() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            max_candidates_per_day: Some(0),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_thumbnail_size() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            thumbnail_max_size: Some(0),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_scan_interval() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            scan_interval_hours: Some(0),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_invalid_age_thresholds() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            archive_age_threshold_days: Some(366),
            delete_age_threshold_days: Some(366),
            ..Default::default()
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ERR_VALIDATION"));
    }

    #[test]
    fn test_set_prefs_valid() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let prefs = PartialUserPrefs {
            dry_run_default: Some(true),
            tidy_day: Some("Mon".to_string()),
            tidy_hour: Some(9),
            rolling_window_days: Some(7),
            max_candidates_per_day: Some(10),
            thumbnail_max_size: Some(512),
            auto_scan_enabled: Some(true),
            scan_interval_hours: Some(12),
            archive_age_threshold_days: Some(7),
            delete_age_threshold_days: Some(30),
        };

        let result = set_prefs(prefs, tauri::State::from(&app_state));
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_prefs_defaults() {
        let (temp_dir, db) = setup_test_db();
        let app_state = AppState { db };

        let result = get_prefs(tauri::State::from(&app_state));
        assert!(result.is_ok());

        let prefs = result.unwrap();
        assert_eq!(prefs.dry_run_default, true);
        assert_eq!(prefs.tidy_day, "Fri");
        assert_eq!(prefs.tidy_hour, 17);
        assert_eq!(prefs.rolling_window_days, 7);
        assert_eq!(prefs.max_candidates_per_day, 12);
        assert_eq!(prefs.thumbnail_max_size, 256);
        assert_eq!(prefs.auto_scan_enabled, false);
        assert_eq!(prefs.scan_interval_hours, 24);
        assert_eq!(prefs.archive_age_threshold_days, 7);
        assert_eq!(prefs.delete_age_threshold_days, 30);
    }

    #[test]
    fn test_get_db_path() {
        let result = get_db_path();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("white-space"));
        assert!(path.to_string_lossy().contains("database.db"));
    }

    #[test]
    fn normalize_directory_path_handles_existing_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).unwrap();

        let normalized = normalize_directory_path(&nested).expect("should normalize directory");

        assert!(normalized.ends_with("nested"));
    }

    #[test]
    fn watched_root_membership_checks_descendants() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();

        let normalized_root = normalize_directory_path(&root).expect("normalize root");
        let normalized_child = normalize_directory_path(&child).expect("normalize child");

        let roots = vec![WatchedRoot {
            id: 1,
            path: normalized_root.to_string_lossy().to_string(),
            created_at: Utc::now(),
        }];

        assert!(is_within_watched_roots(&normalized_child, &roots));

        let outside = temp_dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let normalized_outside = normalize_directory_path(&outside).expect("normalize outside");

        assert!(!is_within_watched_roots(&normalized_outside, &roots));
    }

    #[test]
    fn normalize_existing_path_handles_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "hello").unwrap();

        let normalized = normalize_existing_path(&file_path).expect("normalize file");

        assert!(normalized.ends_with("file.txt"));
    }

    #[test]
    fn ensure_within_watched_rejects_outside() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let normalized_root = normalize_directory_path(&root).expect("normalize root");

        let roots = vec![WatchedRoot {
            id: 1,
            path: normalized_root.to_string_lossy().to_string(),
            created_at: Utc::now(),
        }];

        ensure_within_watched(&normalized_root, &roots).expect("root allowed");

        let outside = temp_dir.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let normalized_outside = normalize_directory_path(&outside).expect("normalize outside");

        assert!(ensure_within_watched(&normalized_outside, &roots).is_err());
    }

    #[test]
    fn test_command_error_display() {
        let errors = vec![
            CommandError::Database("test".to_string()),
            CommandError::FileSystem("test".to_string()),
            CommandError::Validation("test".to_string()),
            CommandError::Permission("test".to_string()),
            CommandError::NotFound("test".to_string()),
            CommandError::Internal("test".to_string()),
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(display.contains("test"));
        }
    }

    #[test]
    fn test_archive_outcome_serialization() {
        let outcome = ArchiveOutcome {
            success: true,
            files_processed: 5,
            total_bytes: 1024 * 1024,
            duration_ms: 1000,
            errors: vec!["test error".to_string()],
            dry_run: false,
        };

        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("files_processed"));
    }

    #[test]
    fn test_delete_outcome_serialization() {
        let outcome = DeleteOutcome {
            success: true,
            files_processed: 3,
            total_bytes_freed: 512 * 1024,
            duration_ms: 500,
            errors: vec![],
            to_trash: true,
        };

        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("to_trash"));
    }

    #[test]
    fn test_staged_file_serialization() {
        let staged_file = StagedFile {
            record_id: 1,
            file_id: 1,
            path: "/test/path".to_string(),
            parent_dir: "/test".to_string(),
            size_bytes: 1024,
            status: "staged".to_string(),
            staged_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            batch_id: Some("batch".to_string()),
            note: None,
            cooloff_until: None,
        };

        let json = serde_json::to_string(&staged_file).unwrap();
        assert!(json.contains("file_id"));
        assert!(json.contains("path"));
    }

    #[test]
    fn test_user_prefs_serialization() {
        let prefs = UserPrefs {
            dry_run_default: true,
            tidy_day: "Mon".to_string(),
            tidy_hour: 9,
            rolling_window_days: 7,
            max_candidates_per_day: 10,
            thumbnail_max_size: 512,
            auto_scan_enabled: true,
            scan_interval_hours: 12,
            archive_age_threshold_days: 7,
            delete_age_threshold_days: 30,
        };

        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("dry_run_default"));
        assert!(json.contains("tidy_day"));
    }

    #[test]
    fn test_partial_user_prefs_deserialization() {
        let json = r#"{
            "dry_run_default": true,
            "tidy_day": "Mon",
            "tidy_hour": 9,
            "rolling_window_days": 7,
            "max_candidates_per_day": 10,
            "thumbnail_max_size": 512,
            "auto_scan_enabled": true,
            "scan_interval_hours": 12,
            "archive_age_threshold_days": 7,
            "delete_age_threshold_days": 30
        }"#;

        let prefs: PartialUserPrefs = serde_json::from_str(json).unwrap();
        assert_eq!(prefs.dry_run_default, Some(true));
        assert_eq!(prefs.tidy_day, Some("Mon".to_string()));
    }
}

// Add Default implementation for PartialUserPrefs
impl Default for PartialUserPrefs {
    fn default() -> Self {
        Self {
            dry_run_default: None,
            tidy_day: None,
            tidy_hour: None,
            rolling_window_days: None,
            max_candidates_per_day: None,
            thumbnail_max_size: None,
            auto_scan_enabled: None,
            scan_interval_hours: None,
            archive_age_threshold_days: None,
            delete_age_threshold_days: None,
        }
    }
}
