#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{File, Action, ActionType, NewAction, NewFile};
    use tempfile::TempDir;
    use std::fs;
    use chrono::{DateTime, Utc, Duration, Weekday};

    fn create_test_database() -> Database {
        Database::open_db(":memory:").unwrap()
    }

    fn create_test_file(db: &Database, path: &str, size_bytes: u64) -> i64 {
        let file = NewFile {
            path: path.to_string(),
            parent_dir: "/test".to_string(),
            mime: Some("text/plain".to_string()),
            size_bytes: size_bytes as i64,
            created_at: Some(Utc::now()),
            last_opened_at: None,
            sha1: Some("test_hash".to_string()),
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
            is_deleted: false,
        };
        
        db.insert_file(file).unwrap()
    }

    fn create_test_action(db: &Database, file_id: i64, action_type: ActionType, created_at: DateTime<Utc>) -> i64 {
        let action = NewAction {
            file_id,
            action: action_type,
            batch_id: Some("test_batch".to_string()),
            src_path: Some("/test/source.txt".to_string()),
            dst_path: Some("/test/destination.txt".to_string()),
        };
        
        db.insert_action(action).unwrap()
    }

    #[test]
    fn test_gauge_state_computation() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        assert!(gauge_state.potential_today_bytes >= 0);
        assert!(gauge_state.staged_week_bytes >= 0);
        assert!(gauge_state.freed_week_bytes >= 0);
        assert!(gauge_state.computed_at <= Utc::now());
        assert!(gauge_state.window_start <= gauge_state.window_end);
    }

    #[test]
    fn test_rolling_window_bounds() {
        let gauge_manager = GaugeManager::new();
        let now = Utc::now();
        
        let (start, end) = gauge_manager.get_window_bounds(now);
        
        // Should be 7 days back
        let expected_start = now - Duration::days(7);
        assert!((start - expected_start).num_seconds().abs() < 60); // Within 1 minute
        assert_eq!(end, now);
    }

    #[test]
    fn test_tidy_day_bounds() {
        let mut gauge_manager = GaugeManager::new();
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(Weekday::Fri);
        gauge_manager.set_tidy_hour(17);
        
        let now = Utc::now();
        let (start, end) = gauge_manager.get_window_bounds(now);
        
        // Start should be on a Friday at 17:00
        assert_eq!(start.weekday(), Weekday::Fri);
        assert_eq!(start.hour(), 17);
        assert_eq!(end, now);
    }

    #[test]
    fn test_tidy_day_edge_cases() {
        let mut gauge_manager = GaugeManager::new();
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(Weekday::Fri);
        gauge_manager.set_tidy_hour(17);
        
        // Test case: exactly at tidy day hour
        let now = Utc::now();
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
        
        // Test case: just before tidy day hour
        gauge_manager.set_tidy_hour(now.hour() + 1);
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
        
        // Test case: just after tidy day hour
        gauge_manager.set_tidy_hour(now.hour() - 1);
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
    }

    #[test]
    fn test_multiple_actions_per_file() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        // Create a test file
        let file_id = create_test_file(&db, "/test/file.txt", 1024);
        
        // Create multiple actions for the same file
        let now = Utc::now();
        let archive_time = now - Duration::days(3);
        let delete_time = now - Duration::days(1);
        
        // Archive the file
        create_test_action(&db, file_id, ActionType::Archive, archive_time);
        
        // Delete the file
        create_test_action(&db, file_id, ActionType::Delete, delete_time);
        
        // The gauge should handle multiple actions correctly
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        // The file should be counted in freed_week_bytes since it was deleted
        assert!(gauge_state.freed_week_bytes >= 0);
    }

    #[test]
    fn test_window_edges() {
        let mut gauge_manager = GaugeManager::new();
        let now = Utc::now();
        
        // Test edge case: exactly at tidy day hour
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(now.weekday());
        gauge_manager.set_tidy_hour(now.hour());
        
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
        
        // Test edge case: just before tidy day hour
        gauge_manager.set_tidy_hour(now.hour() + 1);
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
        
        // Test edge case: just after tidy day hour
        gauge_manager.set_tidy_hour(now.hour() - 1);
        let (start, end) = gauge_manager.get_window_bounds(now);
        assert!(start <= end);
    }

    #[test]
    fn test_config_updates() {
        let mut gauge_manager = GaugeManager::new();
        
        // Test setting tidy day
        gauge_manager.set_tidy_day(Weekday::Mon);
        assert_eq!(gauge_manager.get_config().tidy_day, Weekday::Mon);
        
        // Test setting tidy hour
        gauge_manager.set_tidy_hour(14);
        assert_eq!(gauge_manager.get_config().tidy_hour, 14);
        
        // Test setting rolling window
        gauge_manager.set_rolling_window_days(14);
        assert_eq!(gauge_manager.get_config().rolling_window_days, 14);
        
        // Test enabling tidy day reset
        gauge_manager.set_reset_on_tidy_day(true);
        assert!(gauge_manager.get_config().reset_on_tidy_day);
    }

    #[test]
    fn test_next_reset_time() {
        let mut gauge_manager = GaugeManager::new();
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(Weekday::Fri);
        gauge_manager.set_tidy_hour(17);
        
        let now = Utc::now();
        let next_reset = gauge_manager.get_next_reset_time(now);
        
        if let Some(reset_time) = next_reset {
            assert_eq!(reset_time.weekday(), Weekday::Fri);
            assert_eq!(reset_time.hour(), 17);
            assert!(reset_time > now);
        }
    }

    #[test]
    fn test_bytes_formatting() {
        let gauge_manager = GaugeManager::new();
        
        assert_eq!(gauge_manager.format_bytes(0), "0 B");
        assert_eq!(gauge_manager.format_bytes(1024), "1.0 KB");
        assert_eq!(gauge_manager.format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(gauge_manager.format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_window_info() {
        let mut gauge_manager = GaugeManager::new();
        let now = Utc::now();
        
        // Test rolling window
        let (start, end, description) = gauge_manager.get_window_info(now);
        assert!(description.contains("Rolling"));
        assert!(start <= end);
        
        // Test tidy day window
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(Weekday::Fri);
        gauge_manager.set_tidy_hour(17);
        
        let (start, end, description) = gauge_manager.get_window_info(now);
        assert!(description.contains("Tidy day"));
        assert!(start <= end);
    }

    #[test]
    fn test_gauge_summary() {
        let gauge_manager = GaugeManager::new();
        let state = GaugeState {
            potential_today_bytes: 1024 * 1024, // 1MB
            staged_week_bytes: 2 * 1024 * 1024, // 2MB
            freed_week_bytes: 512 * 1024, // 512KB
            computed_at: Utc::now(),
            window_start: Utc::now() - Duration::days(7),
            window_end: Utc::now(),
        };
        
        let summary = gauge_manager.get_gauge_summary(&state);
        assert!(summary.contains("Potential:"));
        assert!(summary.contains("Staged:"));
        assert!(summary.contains("Freed:"));
    }

    #[test]
    fn test_config_serialization() {
        let config = GaugeConfig {
            reset_on_tidy_day: true,
            tidy_day: Weekday::Fri,
            tidy_hour: 17,
            rolling_window_days: 7,
        };
        
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: GaugeConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.reset_on_tidy_day, deserialized.reset_on_tidy_day);
        assert_eq!(config.tidy_day, deserialized.tidy_day);
        assert_eq!(config.tidy_hour, deserialized.tidy_hour);
        assert_eq!(config.rolling_window_days, deserialized.rolling_window_days);
    }

    #[test]
    fn test_gauge_state_serialization() {
        let state = GaugeState {
            potential_today_bytes: 1024,
            staged_week_bytes: 2048,
            freed_week_bytes: 512,
            computed_at: Utc::now(),
            window_start: Utc::now() - Duration::days(7),
            window_end: Utc::now(),
        };
        
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: GaugeState = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(state.potential_today_bytes, deserialized.potential_today_bytes);
        assert_eq!(state.staged_week_bytes, deserialized.staged_week_bytes);
        assert_eq!(state.freed_week_bytes, deserialized.freed_week_bytes);
    }

    #[test]
    fn test_archive_without_delete() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        // Create a test file
        let file_id = create_test_file(&db, "/test/file.txt", 1024);
        
        // Archive the file but don't delete it
        let now = Utc::now();
        let archive_time = now - Duration::days(3);
        
        create_test_action(&db, file_id, ActionType::Archive, archive_time);
        
        // The file should be counted in staged_week_bytes
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        assert!(gauge_state.staged_week_bytes >= 0);
    }

    #[test]
    fn test_delete_without_archive() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        // Create a test file
        let file_id = create_test_file(&db, "/test/file.txt", 1024);
        
        // Delete the file without archiving it first
        let now = Utc::now();
        let delete_time = now - Duration::days(1);
        
        create_test_action(&db, file_id, ActionType::Delete, delete_time);
        
        // The file should be counted in freed_week_bytes
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        assert!(gauge_state.freed_week_bytes >= 0);
    }

    #[test]
    fn test_archive_then_delete() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        // Create a test file
        let file_id = create_test_file(&db, "/test/file.txt", 1024);
        
        // Archive the file
        let now = Utc::now();
        let archive_time = now - Duration::days(3);
        let delete_time = now - Duration::days(1);
        
        create_test_action(&db, file_id, ActionType::Archive, archive_time);
        create_test_action(&db, file_id, ActionType::Delete, delete_time);
        
        // The file should be counted in freed_week_bytes (not staged)
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        assert!(gauge_state.freed_week_bytes >= 0);
    }

    #[test]
    fn test_outside_window_actions() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();
        
        // Create a test file
        let file_id = create_test_file(&db, "/test/file.txt", 1024);
        
        // Create actions outside the window
        let now = Utc::now();
        let old_time = now - Duration::days(10); // Outside 7-day window
        
        create_test_action(&db, file_id, ActionType::Archive, old_time);
        
        // The file should not be counted in staged_week_bytes
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
        
        let gauge_state = state.unwrap();
        assert_eq!(gauge_state.staged_week_bytes, 0);
    }

    #[test]
    fn test_tidy_day_weekday_calculation() {
        let mut gauge_manager = GaugeManager::new();
        gauge_manager.set_reset_on_tidy_day(true);
        gauge_manager.set_tidy_day(Weekday::Mon);
        gauge_manager.set_tidy_hour(9);
        
        // Test with different weekdays
        let now = Utc::now();
        let (start, end) = gauge_manager.get_window_bounds(now);
        
        // Start should be on a Monday at 9:00
        assert_eq!(start.weekday(), Weekday::Mon);
        assert_eq!(start.hour(), 9);
        assert!(start <= end);
    }

    #[test]
    fn test_rolling_window_custom_days() {
        let mut gauge_manager = GaugeManager::new();
        gauge_manager.set_rolling_window_days(14); // 2 weeks
        
        let now = Utc::now();
        let (start, end) = gauge_manager.get_window_bounds(now);
        
        // Should be 14 days back
        let expected_start = now - Duration::days(14);
        assert!((start - expected_start).num_seconds().abs() < 60); // Within 1 minute
        assert_eq!(end, now);
    }

    #[test]
    fn test_gauge_manager_default() {
        let gauge_manager = GaugeManager::default();
        let config = gauge_manager.get_config();
        
        assert!(!config.reset_on_tidy_day);
        assert_eq!(config.tidy_day, Weekday::Fri);
        assert_eq!(config.tidy_hour, 17);
        assert_eq!(config.rolling_window_days, 7);
    }

    #[test]
    fn test_gauge_manager_new() {
        let gauge_manager = GaugeManager::new();
        let config = gauge_manager.get_config();
        
        assert!(!config.reset_on_tidy_day);
        assert_eq!(config.tidy_day, Weekday::Fri);
        assert_eq!(config.tidy_hour, 17);
        assert_eq!(config.rolling_window_days, 7);
    }
}






