#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::open_db(&db_path).unwrap();
        db.run_migrations().unwrap();
        (temp_dir, db)
    }

    #[test]
    fn test_license_manager_new() {
        let manager = LicenseManager::new();
        assert_eq!(manager.api_base_url, "https://api.whitespace.app/v1");
    }

    #[test]
    fn test_license_storage_new() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);
        assert!(storage.db.get_preference("license_key").is_ok());
    }

    #[test]
    fn test_license_storage_store_and_get() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();

        // Retrieve license data
        let (license_key, instance_id, instance_name) = storage.get_license_data().unwrap();
        assert_eq!(license_key, Some("test-key".to_string()));
        assert_eq!(instance_id, Some("test-instance".to_string()));
        assert_eq!(instance_name, Some("Test Device".to_string()));
    }

    #[test]
    fn test_license_storage_clear() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();

        // Clear license data
        storage.clear_license_data().unwrap();

        // Verify data is cleared
        let (license_key, instance_id, instance_name) = storage.get_license_data().unwrap();
        assert_eq!(license_key, Some("".to_string()));
        assert_eq!(instance_id, Some("".to_string()));
        assert_eq!(instance_name, Some("".to_string()));
    }

    #[test]
    fn test_license_storage_details() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        let expires_at = Utc::now() + Duration::days(30);

        // Store license details
        storage
            .store_license_details(Some(expires_at), Some(5), Some(2))
            .unwrap();

        // Retrieve license details
        let (stored_expires_at, max_seats, used_seats) = storage.get_license_details().unwrap();
        assert!(stored_expires_at.is_some());
        assert_eq!(max_seats, Some(5));
        assert_eq!(used_seats, Some(2));
    }

    #[test]
    fn test_license_checker_new() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);
        let checker = LicenseChecker::new(storage);
        assert!(checker.is_license_valid().is_ok());
    }

    #[test]
    fn test_license_checker_no_license() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);
        let checker = LicenseChecker::new(storage);

        let is_valid = checker.is_license_valid().unwrap();
        assert!(!is_valid);
    }

    #[test]
    fn test_license_checker_with_license() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let is_valid = checker.is_license_valid().unwrap();
        assert!(is_valid); // Should be valid due to offline grace
    }

    #[test]
    fn test_offline_grace_period() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data with recent validation
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let now = Utc::now().to_rfc3339();
        storage.db.set_preference("last_validated", &now).unwrap();

        let checker = LicenseChecker::new(storage);
        let is_in_grace = checker.is_in_offline_grace().unwrap();
        assert!(is_in_grace);
    }

    #[test]
    fn test_offline_grace_expired() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data with old validation (beyond grace period)
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let old_time = (Utc::now() - Duration::days(15)).to_rfc3339();
        storage
            .db
            .set_preference("last_validated", &old_time)
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let is_in_grace = checker.is_in_offline_grace().unwrap();
        assert!(!is_in_grace);
    }

    #[test]
    fn test_license_status_unlicensed() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);
        let checker = LicenseChecker::new(storage);

        let status = checker.get_license_status().unwrap();
        assert!(!status.is_licensed);
        assert!(!status.is_offline_grace);
        assert_eq!(status.status_message, "No license found");
    }

    #[test]
    fn test_license_status_licensed() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let expires_at = Utc::now() + Duration::days(30);
        storage
            .store_license_details(Some(expires_at), Some(5), Some(2))
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert_eq!(status.license_key, Some("test-key".to_string()));
        assert_eq!(status.instance_id, Some("test-instance".to_string()));
        assert_eq!(status.instance_name, Some("Test Device".to_string()));
        assert_eq!(status.max_seats, Some(5));
        assert_eq!(status.used_seats, Some(2));
        assert!(status.days_remaining.is_some());
    }

    #[test]
    fn test_license_status_offline_grace() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license data with recent validation
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let now = Utc::now().to_rfc3339();
        storage.db.set_preference("last_validated", &now).unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert!(status.is_offline_grace);
        assert_eq!(status.status_message, "Offline grace period active");
        assert!(status.grace_expires_at.is_some());
    }

    #[test]
    fn test_needs_validation_never_validated() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);
        let checker = LicenseChecker::new(storage);

        let needs_validation = checker.needs_validation().unwrap();
        assert!(needs_validation);
    }

    #[test]
    fn test_needs_validation_recent() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store recent validation
        let now = Utc::now().to_rfc3339();
        storage.db.set_preference("last_validated", &now).unwrap();

        let checker = LicenseChecker::new(storage);
        let needs_validation = checker.needs_validation().unwrap();
        assert!(!needs_validation);
    }

    #[test]
    fn test_needs_validation_old() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store old validation (beyond 7 days)
        let old_time = (Utc::now() - Duration::days(8)).to_rfc3339();
        storage
            .db
            .set_preference("last_validated", &old_time)
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let needs_validation = checker.needs_validation().unwrap();
        assert!(needs_validation);
    }

    #[test]
    fn test_activate_resp_serialization() {
        let response = ActivateResp {
            success: true,
            instance_id: Some("test-instance".to_string()),
            message: "Activation successful".to_string(),
            expires_at: Some(Utc::now() + Duration::days(30)),
            max_seats: Some(5),
            used_seats: Some(2),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("instance_id"));
        assert!(json.contains("message"));
    }

    #[test]
    fn test_validate_resp_serialization() {
        let response = ValidateResp {
            success: true,
            valid: true,
            message: "Validation successful".to_string(),
            expires_at: Some(Utc::now() + Duration::days(30)),
            max_seats: Some(5),
            used_seats: Some(2),
            instance_name: Some("Test Device".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("valid"));
        assert!(json.contains("message"));
    }

    #[test]
    fn test_deactivate_resp_serialization() {
        let response = DeactivateResp {
            success: true,
            message: "Deactivation successful".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("message"));
    }

    #[test]
    fn test_license_status_serialization() {
        let status = LicenseStatus {
            is_licensed: true,
            license_key: Some("test-key".to_string()),
            instance_id: Some("test-instance".to_string()),
            instance_name: Some("Test Device".to_string()),
            expires_at: Some(Utc::now() + Duration::days(30)),
            max_seats: Some(5),
            used_seats: Some(2),
            last_validated: Some(Utc::now()),
            is_offline_grace: false,
            grace_expires_at: None,
            days_remaining: Some(30),
            status_message: "License valid".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("is_licensed"));
        assert!(json.contains("license_key"));
        assert!(json.contains("status_message"));
    }

    #[test]
    fn test_create_license_storage() {
        let (temp_dir, db) = setup_test_db();
        let storage = create_license_storage(db);

        // Test that storage works
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let (license_key, instance_id, instance_name) = storage.get_license_data().unwrap();
        assert_eq!(license_key, Some("test-key".to_string()));
        assert_eq!(instance_id, Some("test-instance".to_string()));
        assert_eq!(instance_name, Some("Test Device".to_string()));
    }

    #[test]
    fn test_license_status_expired() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store expired license
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let expired_time = Utc::now() - Duration::days(1);
        storage
            .store_license_details(Some(expired_time), Some(5), Some(2))
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert_eq!(status.status_message, "License expired");
        assert!(status.days_remaining.is_some());
        assert!(status.days_remaining.unwrap() < 0);
    }

    #[test]
    fn test_license_status_expiring_soon() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license expiring in 3 days
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let expiring_time = Utc::now() + Duration::days(3);
        storage
            .store_license_details(Some(expiring_time), Some(5), Some(2))
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert_eq!(status.status_message, "License valid");
        assert_eq!(status.days_remaining, Some(3));
    }

    #[test]
    fn test_license_status_unknown() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license without expiration
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert_eq!(status.status_message, "License status unknown");
        assert!(status.days_remaining.is_none());
    }

    #[test]
    fn test_grace_period_calculation() {
        let (temp_dir, db) = setup_test_db();
        let storage = LicenseStorage::new(db);

        // Store license with validation 5 days ago
        storage
            .store_license_data("test-key", "test-instance", "Test Device")
            .unwrap();
        let validation_time = (Utc::now() - Duration::days(5)).to_rfc3339();
        storage
            .db
            .set_preference("last_validated", &validation_time)
            .unwrap();

        let checker = LicenseChecker::new(storage);
        let status = checker.get_license_status().unwrap();

        assert!(status.is_licensed);
        assert!(status.is_offline_grace);
        assert!(status.grace_expires_at.is_some());

        // Grace should expire in 9 days (14 - 5)
        let grace_remaining = status.grace_expires_at.unwrap() - Utc::now();
        assert!(grace_remaining.num_days() <= 9);
        assert!(grace_remaining.num_days() >= 8);
    }
}
