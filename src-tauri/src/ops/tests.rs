#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::ops::*;
    use tempfile::TempDir;
    use std::fs;
    use std::path::Path;

    fn create_test_database() -> Database {
        Database::open_db(":memory:").unwrap()
    }

    fn create_test_files(temp_dir: &TempDir) -> Vec<String> {
        let files = vec![
            "test1.txt",
            "test2.txt", 
            "large_file.bin",
            "screenshot.png",
            "document.pdf",
        ];

        let mut file_paths = Vec::new();
        for filename in files {
            let file_path = temp_dir.path().join(filename);
            let content = match filename {
                "large_file.bin" => vec![0u8; 1024 * 1024], // 1MB
                _ => b"test content".to_vec(),
            };
            fs::write(&file_path, content).unwrap();
            file_paths.push(file_path.to_string_lossy().to_string());
        }

        file_paths
    }

    #[test]
    fn test_archive_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        
        // Create test files
        let file_paths = create_test_files(&temp_dir);
        
        // Test archive operation
        let result = archive_manager.archive_files(file_paths.clone(), &db);
        assert!(result.is_ok());
        
        let archive_result = result.unwrap();
        assert_eq!(archive_result.files_archived, file_paths.len());
        assert!(archive_result.total_bytes > 0);
        assert!(archive_result.duration_ms > 0);
        assert!(!archive_result.batch_id.is_empty());
        
        // Verify files were moved to archive
        let archive_path = archive_manager.get_config().get_daily_path();
        for file_path in &file_paths {
            let filename = Path::new(file_path).file_name().unwrap();
            let archived_file = archive_path.join(filename);
            assert!(archived_file.exists());
        }
    }

    #[test]
    fn test_archive_conflict_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        
        // Create a file
        let file_path = temp_dir.path().join("conflict.txt");
        fs::write(&file_path, "content").unwrap();
        
        // Create archive directory with same filename
        let archive_path = archive_manager.get_config().get_daily_path();
        fs::create_dir_all(&archive_path).unwrap();
        fs::write(archive_path.join("conflict.txt"), "existing").unwrap();
        
        // Archive the file - should create conflict resolution
        let result = archive_manager.archive_files(vec![file_path.to_string_lossy().to_string()], &db);
        assert!(result.is_ok());
        
        // Check that conflict was resolved with (1) suffix
        let conflict_file = archive_path.join("conflict (1).txt");
        assert!(conflict_file.exists());
    }

    #[test]
    fn test_delete_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut delete_manager = DeleteManager::new();
        
        // Create test files
        let file_paths = create_test_files(&temp_dir);
        
        // Test delete operation
        let result = delete_manager.delete_files(file_paths.clone(), &db);
        assert!(result.is_ok());
        
        let delete_result = result.unwrap();
        assert_eq!(delete_result.files_deleted, file_paths.len());
        assert!(delete_result.total_bytes_freed > 0);
        assert!(delete_result.duration_ms > 0);
        assert!(!delete_result.batch_id.is_empty());
        
        // Verify files were moved to trash
        for file_path in &file_paths {
            assert!(!Path::new(file_path).exists());
        }
    }

    #[test]
    fn test_delete_permanent_mode() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut delete_manager = DeleteManager::new();
        
        // Set permanent delete mode
        delete_manager.set_permanent_delete(true);
        
        // Create a test file
        let file_path = temp_dir.path().join("permanent.txt");
        fs::write(&file_path, "content").unwrap();
        
        // Delete the file
        let result = delete_manager.delete_files(vec![file_path.to_string_lossy().to_string()], &db);
        assert!(result.is_ok());
        
        // Verify file was permanently deleted
        assert!(!file_path.exists());
    }

    #[test]
    fn test_undo_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        let mut undo_manager = UndoManager::new();
        
        // Create and archive a file
        let file_path = temp_dir.path().join("undo_test.txt");
        fs::write(&file_path, "content").unwrap();
        
        let archive_result = archive_manager.archive_files(vec![file_path.to_string_lossy().to_string()], &db);
        assert!(archive_result.is_ok());
        
        // Undo the archive operation
        let undo_result = undo_manager.undo_last(&db);
        assert!(undo_result.is_ok());
        
        let undo = undo_result.unwrap();
        assert_eq!(undo.actions_reversed, 1);
        assert_eq!(undo.files_restored, 1);
        assert!(!undo.batch_id.is_empty());
        
        // Verify file was restored
        assert!(file_path.exists());
    }

    #[test]
    fn test_space_management() {
        let temp_dir = TempDir::new().unwrap();
        let space_manager = SpaceManager::new();
        
        // Test space info
        let space_info = space_manager.get_space_info(temp_dir.path());
        assert!(space_info.is_ok());
        
        let info = space_info.unwrap();
        assert!(info.total_bytes > 0);
        assert!(info.available_bytes > 0);
        assert!(info.free_percentage >= 0.0);
        assert!(info.free_percentage <= 100.0);
        
        // Test space requirements check
        let checks = space_manager.check_space_requirements(
            vec![temp_dir.path().to_string_lossy().to_string()],
            1024
        );
        assert!(checks.is_ok());
        
        let space_checks = checks.unwrap();
        assert_eq!(space_checks.len(), 1);
        assert!(space_checks[0].sufficient);
    }

    #[test]
    fn test_directory_size_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let space_manager = SpaceManager::new();
        
        // Create test files
        let file_paths = create_test_files(&temp_dir);
        
        // Calculate directory size
        let total_size = space_manager.calculate_directory_size(temp_dir.path());
        assert!(total_size.is_ok());
        
        let size = total_size.unwrap();
        assert!(size > 0);
        
        // Should be at least the size of our test files
        assert!(size >= 1024 * 1024); // At least 1MB from large_file.bin
    }

    #[test]
    fn test_largest_files() {
        let temp_dir = TempDir::new().unwrap();
        let space_manager = SpaceManager::new();
        
        // Create test files with different sizes
        let files = vec![
            ("small.txt", 100),
            ("medium.txt", 1000),
            ("large.txt", 10000),
        ];
        
        for (filename, size) in files {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, vec![0u8; size]).unwrap();
        }
        
        // Get largest files
        let largest = space_manager.get_largest_files(temp_dir.path(), 2);
        assert!(largest.is_ok());
        
        let files = largest.unwrap();
        assert_eq!(files.len(), 2);
        
        // Should be sorted by size (largest first)
        assert!(files[0].1 >= files[1].1);
    }

    #[test]
    fn test_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        
        // Test with non-existent file
        let result = archive_manager.archive_files(vec!["/nonexistent/file.txt".to_string()], &db);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(matches!(error, OpsError::ArchiveError(_)));
        
        // Test error message
        let user_message = error.to_user_message();
        assert_eq!(user_message.title, "Archive Failed");
        assert!(user_message.message.contains("nonexistent"));
        assert!(user_message.suggestion.is_some());
        assert!(user_message.recoverable);
    }

    #[test]
    fn test_cross_volume_simulation() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        
        // Create a file in temp_dir1
        let file_path = temp_dir1.path().join("cross_volume.txt");
        fs::write(&file_path, "cross volume test").unwrap();
        
        // Set archive path to temp_dir2 (simulating cross-volume)
        let mut config = ArchiveConfig::default();
        config.base_path = temp_dir2.path().to_path_buf();
        archive_manager.update_config(config);
        
        // Archive the file
        let result = archive_manager.archive_files(vec![file_path.to_string_lossy().to_string()], &db);
        assert!(result.is_ok());
        
        // Verify file was copied and original deleted
        assert!(!file_path.exists());
        
        let archive_path = archive_manager.get_config().get_daily_path();
        let archived_file = archive_path.join("cross_volume.txt");
        assert!(archived_file.exists());
    }

    #[test]
    fn test_batch_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        let mut undo_manager = UndoManager::new();
        
        // Create files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();
        
        // Archive both files
        let result = archive_manager.archive_files(
            vec![file1.to_string_lossy().to_string(), file2.to_string_lossy().to_string()],
            &db
        );
        assert!(result.is_ok());
        
        // Simulate partial failure by removing one archived file
        let archive_path = archive_manager.get_config().get_daily_path();
        let archived_file1 = archive_path.join("file1.txt");
        if archived_file1.exists() {
            fs::remove_file(&archived_file1).unwrap();
        }
        
        // Attempt undo - should trigger rollback
        let undo_result = undo_manager.undo_last(&db);
        assert!(undo_result.is_ok());
        
        let undo = undo_result.unwrap();
        assert!(undo.rollback_performed);
        assert!(!undo.errors.is_empty());
    }

    #[test]
    fn test_progress_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let db = create_test_database();
        let mut archive_manager = ArchiveManager::new();
        
        // Create a large file
        let large_file = temp_dir.path().join("large.bin");
        let content = vec![0u8; 1024 * 1024]; // 1MB
        fs::write(&large_file, content).unwrap();
        
        // Archive the file
        let result = archive_manager.archive_files(vec![large_file.to_string_lossy().to_string()], &db);
        assert!(result.is_ok());
        
        let archive_result = result.unwrap();
        assert_eq!(archive_result.files_archived, 1);
        assert_eq!(archive_result.total_bytes, 1024 * 1024);
    }

    #[test]
    fn test_configuration_updates() {
        let mut archive_manager = ArchiveManager::new();
        let mut delete_manager = DeleteManager::new();
        
        // Test archive config update
        let mut config = ArchiveConfig::default();
        config.free_space_buffer = 10.0;
        archive_manager.update_config(config);
        
        let updated_config = archive_manager.get_config();
        assert_eq!(updated_config.free_space_buffer, 10.0);
        
        // Test delete config update
        delete_manager.set_permanent_delete(true);
        assert!(delete_manager.get_config().permanent_delete);
        assert!(!delete_manager.get_config().use_trash);
        
        delete_manager.set_use_trash(true);
        assert!(delete_manager.get_config().use_trash);
        assert!(!delete_manager.get_config().permanent_delete);
    }

    #[test]
    fn test_error_context() {
        let context = ErrorContext::new("test_operation")
            .with_file_path("/test/file.txt")
            .with_batch_id("test_batch_123");
        
        let context_str = context.to_string();
        assert!(context_str.contains("test_operation"));
        assert!(context_str.contains("/test/file.txt"));
        assert!(context_str.contains("test_batch_123"));
        assert!(context_str.contains("Time:"));
    }

    #[test]
    fn test_recovery_strategies() {
        let space_error = OpsError::SpaceError("No space".to_string());
        let permission_error = OpsError::PermissionError("Access denied".to_string());
        let file_not_found = OpsError::FileNotFound("Missing file".to_string());
        
        assert!(matches!(suggest_recovery_strategy(&space_error), RecoveryStrategy::Abort));
        assert!(matches!(suggest_recovery_strategy(&permission_error), RecoveryStrategy::Retry));
        assert!(matches!(suggest_recovery_strategy(&file_not_found), RecoveryStrategy::Skip));
    }

    #[test]
    fn test_bytes_formatting() {
        let space_manager = SpaceManager::new();
        
        assert_eq!(space_manager.format_bytes(0), "0 B");
        assert_eq!(space_manager.format_bytes(1024), "1.0 KB");
        assert_eq!(space_manager.format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(space_manager.format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_cleanup_impact_estimation() {
        let temp_dir = TempDir::new().unwrap();
        let space_manager = SpaceManager::new();
        
        // Create test files
        let file_paths = create_test_files(&temp_dir);
        
        // Estimate cleanup impact
        let impact = space_manager.estimate_cleanup_impact(file_paths);
        assert!(impact.is_ok());
        
        let bytes = impact.unwrap();
        assert!(bytes > 0);
        assert!(bytes >= 1024 * 1024); // At least 1MB
    }
}






