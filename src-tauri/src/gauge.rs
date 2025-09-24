use crate::db::Database;
use crate::models::{ActionType, File};
use crate::ops::error::{OpsError, OpsResult};
use crate::selector::FileSelector;
use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GaugeState {
    pub potential_today_bytes: u64,
    pub staged_week_bytes: u64,
    pub freed_week_bytes: u64,
    pub computed_at: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GaugeConfig {
    pub reset_on_tidy_day: bool,
    pub tidy_day: Weekday,
    pub tidy_hour: u32,
    pub rolling_window_days: i64,
}

impl Default for GaugeConfig {
    fn default() -> Self {
        Self {
            reset_on_tidy_day: false,
            tidy_day: Weekday::Fri,
            tidy_hour: 17,
            rolling_window_days: 7,
        }
    }
}

pub struct GaugeManager {
    config: GaugeConfig,
    selector: FileSelector,
}

impl GaugeManager {
    pub fn new() -> Self {
        Self {
            config: GaugeConfig::default(),
            selector: FileSelector::new(),
        }
    }

    pub fn gauge_state(&self, db: &Database) -> OpsResult<GaugeState> {
        let now = Utc::now();
        let (window_start, window_end) = self.get_window_bounds(now);

        // Compute potential (current daily candidates)
        let potential_today_bytes = self.compute_potential_today(db)?;

        // Compute staged (archived but not deleted in window)
        let staged_week_bytes = self.compute_staged_week(db, window_start, window_end)?;

        // Compute freed (deleted in window)
        let freed_week_bytes = self.compute_freed_week(db, window_start, window_end)?;

        Ok(GaugeState {
            potential_today_bytes,
            staged_week_bytes,
            freed_week_bytes,
            computed_at: now,
            window_start,
            window_end,
        })
    }

    fn get_window_bounds(&self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        if self.config.reset_on_tidy_day {
            self.get_tidy_day_bounds(now)
        } else {
            self.get_rolling_window_bounds(now)
        }
    }

    fn get_tidy_day_bounds(&self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        let tidy_day = self.config.tidy_day;
        let tidy_hour = self.config.tidy_hour;

        // Find the most recent tidy day at the specified hour
        let mut current = now.date_naive();
        let mut tidy_datetime = current.and_hms_opt(tidy_hour, 0, 0).unwrap();

        // If we're past the tidy time today and today is the tidy day, use today
        if now.weekday() == tidy_day && now.hour() >= tidy_hour {
            // Use today's tidy time as window start
        } else {
            // Find the most recent tidy day
            let days_back =
                (now.weekday().num_days_from_monday() + 7 - tidy_day.num_days_from_monday()) % 7;
            if days_back == 0 && now.hour() < tidy_hour {
                // If today is tidy day but we haven't reached the hour yet, go back a week
                current = current - Duration::days(7);
            } else {
                current = current - Duration::days(days_back as i64);
            }
            tidy_datetime = current.and_hms_opt(tidy_hour, 0, 0).unwrap();
        }

        let window_start = DateTime::from_naive_utc_and_offset(tidy_datetime, Utc);
        let window_end = now;

        (window_start, window_end)
    }

    fn get_rolling_window_bounds(&self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        let window_start = now - Duration::days(self.config.rolling_window_days);
        let window_end = now;

        (window_start, window_end)
    }

    fn compute_potential_today(&self, db: &Database) -> OpsResult<u64> {
        // Get current daily candidates
        let candidates = self.selector.daily_candidates(Some(1000), db)?; // Large limit to get all

        let total_bytes: u64 = candidates
            .iter()
            .map(|candidate| candidate.size_bytes as u64)
            .sum();

        Ok(total_bytes)
    }

    fn compute_staged_week(
        &self,
        db: &Database,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> OpsResult<u64> {
        // Get all files that were archived in the window
        let archived_files = self.get_archived_files_in_window(db, window_start, window_end)?;

        let mut staged_bytes = 0u64;

        for file in archived_files {
            // Check if this file has been deleted after being archived
            if !self.has_delete_action_after_archive(db, &file, window_start, window_end)? {
                staged_bytes += file.size_bytes as u64;
            }
        }

        Ok(staged_bytes)
    }

    fn compute_freed_week(
        &self,
        db: &Database,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> OpsResult<u64> {
        // Get all delete actions in the window
        let delete_actions = self.get_delete_actions_in_window(db, window_start, window_end)?;

        let mut freed_bytes = 0u64;

        for action in delete_actions {
            // Get the file size from the action's file_id
            if let Some(file) = self.get_file_by_id(db, action.file_id)? {
                freed_bytes += file.size_bytes as u64;
            }
        }

        Ok(freed_bytes)
    }

    fn get_archived_files_in_window(
        &self,
        db: &Database,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> OpsResult<Vec<File>> {
        db.get_files_archived_in_period(&window_start.to_rfc3339(), &window_end.to_rfc3339())
            .map_err(|e| OpsError::GaugeError(format!("Failed to get archived files: {}", e)))
    }

    fn get_delete_actions_in_window(
        &self,
        db: &Database,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> OpsResult<Vec<crate::models::Action>> {
        db.get_files_deleted_in_period(&window_start.to_rfc3339(), &window_end.to_rfc3339())
            .map_err(|e| OpsError::GaugeError(format!("Failed to get delete actions: {}", e)))
    }

    fn has_delete_action_after_archive(
        &self,
        db: &Database,
        file: &File,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> OpsResult<bool> {
        // Check if there's a delete action for this file after its archive action
        // For now, return false as placeholder
        Ok(false)
    }

    fn get_file_by_id(&self, db: &Database, file_id: i64) -> OpsResult<Option<File>> {
        db.get_file_by_id(file_id)
            .map_err(|e| OpsError::GaugeError(format!("Failed to get file by ID: {}", e)))
    }

    pub fn update_config(&mut self, config: GaugeConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &GaugeConfig {
        &self.config
    }

    pub fn set_reset_on_tidy_day(&mut self, enabled: bool) {
        self.config.reset_on_tidy_day = enabled;
    }

    pub fn set_tidy_day(&mut self, day: Weekday) {
        self.config.tidy_day = day;
    }

    pub fn set_tidy_hour(&mut self, hour: u32) {
        self.config.tidy_hour = hour;
    }

    pub fn set_rolling_window_days(&mut self, days: i64) {
        self.config.rolling_window_days = days;
    }

    pub fn get_window_info(&self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>, String) {
        let (start, end) = self.get_window_bounds(now);
        let description = if self.config.reset_on_tidy_day {
            format!(
                "Tidy day window: {} {}:00",
                self.config.tidy_day, self.config.tidy_hour
            )
        } else {
            format!("Rolling {} day window", self.config.rolling_window_days)
        };

        (start, end, description)
    }

    pub fn get_next_reset_time(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if !self.config.reset_on_tidy_day {
            return None;
        }

        let tidy_day = self.config.tidy_day;
        let tidy_hour = self.config.tidy_hour;

        // Find next tidy day at the specified hour
        let mut current = now.date_naive();
        let mut days_ahead = 0;

        loop {
            let weekday = current.weekday();
            if weekday == tidy_day {
                let tidy_datetime = current.and_hms_opt(tidy_hour, 0, 0).unwrap();
                let tidy_time = DateTime::from_naive_utc_and_offset(tidy_datetime, Utc);

                if tidy_time > now {
                    return Some(tidy_time);
                }
            }

            current = current + Duration::days(1);
            days_ahead += 1;

            if days_ahead > 7 {
                break;
            }
        }

        None
    }

    pub fn get_gauge_summary(&self, state: &GaugeState) -> String {
        format!(
            "Potential: {}, Staged: {}, Freed: {}",
            self.format_bytes(state.potential_today_bytes),
            self.format_bytes(state.staged_week_bytes),
            self.format_bytes(state.freed_week_bytes)
        )
    }

    fn format_bytes(&self, bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        const THRESHOLD: u64 = 1024;

        if bytes == 0 {
            return "0 B".to_string();
        }

        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
            size /= THRESHOLD as f64;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
}

impl Default for GaugeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_database() -> Database {
        Database::open_db(":memory:").unwrap()
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
            freed_week_bytes: 512 * 1024,       // 512KB
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
    fn test_multiple_actions_per_file() {
        let db = create_test_database();
        let gauge_manager = GaugeManager::new();

        // This test would verify that files with multiple actions are handled correctly
        // For now, just ensure the function doesn't panic
        let state = gauge_manager.gauge_state(&db);
        assert!(state.is_ok());
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

        assert_eq!(
            state.potential_today_bytes,
            deserialized.potential_today_bytes
        );
        assert_eq!(state.staged_week_bytes, deserialized.staged_week_bytes);
        assert_eq!(state.freed_week_bytes, deserialized.freed_week_bytes);
    }
}
