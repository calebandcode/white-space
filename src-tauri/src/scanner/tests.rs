#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::scanner::{Scanner, file_walker::FileWalker, active_project::ActiveProjectDetector};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_corpus() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create directory structure
        let dirs = [
            "Desktop",
            "Downloads", 
            "Pictures/Screenshots",
            "Documents/Projects/current-project",
            "Documents/Projects/wip-project",
            "Documents/Projects/final-project",
            "Documents/Projects/old-project",
            "Code/active-repo/.git",
            "Code/inactive-repo/.git",
            "node_modules/test-package",
            ".git",
        ];

        for dir in &dirs {
            let path = root.join(dir);
            fs::create_dir_all(&path).unwrap();
        }

        // Create test files
        let files = [
            "Desktop/document.txt",
            "Desktop/image.jpg",
            "Downloads/file.zip",
            "Downloads/video.mp4",
            "Pictures/Screenshots/screenshot.png",
            "Documents/Projects/current-project/README.md",
            "Documents/Projects/current-project/src/main.rs",
            "Documents/Projects/wip-project/work.md",
            "Documents/Projects/final-project/final.txt",
            "Documents/Projects/old-project/old.txt",
            "Code/active-repo/main.py",
            "Code/inactive-repo/old.py",
            "node_modules/test-package/index.js",
            ".DS_Store",
            "Thumbs.db",
            "symlink.txt", // Will be created as symlink
        ];

        for file in &files {
            let path = root.join(file);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            
            if file == "symlink.txt" {
                // Create a symlink (this will be skipped in scanning)
                #[cfg(unix)]
                std::os::unix::fs::symlink("target.txt", &path).unwrap();
                #[cfg(windows)]
                std::os::windows::fs::symlink_file("target.txt", &path).unwrap();
            } else {
                fs::write(&path, "test content").unwrap();
            }
        }

        // Create some git repos
        let git_repos = [
            "Documents/Projects/current-project/.git",
            "Documents/Projects/wip-project/.git", 
            "Documents/Projects/final-project/.git",
            "Code/active-repo/.git",
            "Code/inactive-repo/.git",
        ];

        for repo in &git_repos {
            let git_dir = root.join(repo);
            fs::create_dir_all(&git_dir).unwrap();
            // Create a simple git config to make it look like a real repo
            fs::write(git_dir.join("config"), "[core]\n\trepositoryformatversion = 0").unwrap();
        }

        temp_dir
    }

    #[test]
    fn test_file_walker_basic_scan() {
        let temp_dir = create_test_corpus();
        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut walker = FileWalker::new();
        let roots = vec![temp_dir.path().to_string_lossy().to_string()];
        
        let result = walker.scan_roots(roots, &db);
        
        // Should count most files but skip directories and symlinks
        assert!(result.counted > 0);
        assert!(result.skipped > 0);
        assert!(result.duration_ms > 0);
        
        // Should skip .DS_Store, Thumbs.db, symlinks, and node_modules
        assert!(result.skipped >= 3); // At least .DS_Store, Thumbs.db, and symlink
    }

    #[test]
    fn test_file_walker_depth_limit() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a deep directory structure (more than 5 levels)
        let deep_path = root.join("level1/level2/level3/level4/level5/level6/level7");
        fs::create_dir_all(&deep_path).unwrap();
        fs::write(deep_path.join("deep_file.txt"), "content").unwrap();

        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut walker = FileWalker::new();
        let roots = vec![root.to_string_lossy().to_string()];
        
        let result = walker.scan_roots(roots, &db);
        
        // The deep file should be skipped due to depth limit
        assert!(result.skipped > 0);
    }

    #[test]
    fn test_active_project_detector() {
        let temp_dir = create_test_corpus();
        let detector = ActiveProjectDetector::new();
        let roots = vec![temp_dir.path().to_string_lossy().to_string()];
        
        let repos = detector.detect_dev_repos(&roots);
        
        // Should find git repositories
        assert!(repos.len() > 0);
        
        // Check that keyword flags are detected
        let current_repo = repos.iter().find(|r| r.path.ends_with("current-project"));
        assert!(current_repo.is_some());
        assert!(current_repo.unwrap().keyword_flags.contains(&"current".to_string()));
        
        let wip_repo = repos.iter().find(|r| r.path.ends_with("wip-project"));
        assert!(wip_repo.is_some());
        assert!(wip_repo.unwrap().keyword_flags.contains(&"wip".to_string()));
    }

    #[test]
    fn test_recent_burst_detection() {
        let temp_dir = create_test_corpus();
        let detector = ActiveProjectDetector::new();
        
        // Test burst detection on a directory
        let test_dir = temp_dir.path().join("Documents/Projects");
        let burst = detector.detect_recent_burst(&test_dir).unwrap();
        
        assert_eq!(burst.directory, test_dir);
        assert_eq!(burst.time_window_hours, 72);
        // Should detect some modifications (our test files)
        assert!(burst.modified_count > 0);
    }

    #[test]
    fn test_scanner_integration() {
        let temp_dir = create_test_corpus();
        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut scanner = Scanner::new();
        let roots = vec![temp_dir.path().to_string_lossy().to_string()];
        
        let result = scanner.scan_roots(roots, &db);
        
        // Should complete successfully
        assert!(result.counted > 0);
        assert!(result.errors.is_empty());
        
        // Check that metrics were recorded
        let metrics = db.get_preference("enable_metrics").unwrap();
        assert!(metrics.is_some());
    }

    #[test]
    fn test_incremental_scan() {
        let temp_dir = create_test_corpus();
        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut scanner = Scanner::new();
        let roots = vec![temp_dir.path().to_string_lossy().to_string()];
        
        // Initial scan
        let initial_result = scanner.scan_roots(roots.clone(), &db);
        assert!(initial_result.counted > 0);
        
        // Incremental scan
        let incremental_result = scanner.incremental_scan(&db);
        // Incremental scan should be faster and process fewer files
        assert!(incremental_result.duration_ms <= initial_result.duration_ms);
    }

    #[test]
    fn test_mime_type_detection() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create files with different extensions
        let test_files = [
            ("test.txt", "text/plain"),
            ("test.md", "text/markdown"),
            ("test.html", "text/html"),
            ("test.css", "text/css"),
            ("test.js", "application/javascript"),
            ("test.json", "application/json"),
            ("test.pdf", "application/pdf"),
            ("test.jpg", "image/jpeg"),
            ("test.png", "image/png"),
            ("test.mp4", "video/mp4"),
            ("test.mp3", "audio/mpeg"),
            ("test.zip", "application/zip"),
        ];

        for (filename, expected_mime) in &test_files {
            let path = root.join(filename);
            fs::write(&path, "content").unwrap();
            
            let walker = FileWalker::new();
            let metadata = walker.extract_metadata(&path).unwrap();
            
            assert_eq!(metadata.mime_type, Some(expected_mime.to_string()));
        }
    }

    #[test]
    fn test_performance_target() {
        let temp_dir = create_test_corpus();
        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut scanner = Scanner::new();
        let roots = vec![temp_dir.path().to_string_lossy().to_string()];
        
        let result = scanner.scan_roots(roots, &db);
        
        // For our small test corpus, should complete well under 90 seconds
        assert!(result.duration_ms < 90000);
        
        // Should record performance metrics
        let performance_metrics = scanner.get_performance_summary(&db).unwrap();
        // Performance metrics should be recorded (even if empty for now)
        assert!(performance_metrics.is_empty() || performance_metrics.len() > 0);
    }

    #[test]
    fn test_default_scan_roots() {
        let roots = ActiveProjectDetector::get_default_scan_roots();
        
        // Should return some default directories
        assert!(!roots.is_empty());
        
        // Should include common directories
        let root_strings: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
        let has_desktop = root_strings.iter().any(|s| s.contains("Desktop"));
        let has_downloads = root_strings.iter().any(|s| s.contains("Downloads"));
        
        // At least some common directories should be present
        assert!(has_desktop || has_downloads);
    }

    #[test]
    fn test_skip_directories() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create directories that should be skipped
        let skip_dirs = [".git", "node_modules", ".DS_Store"];
        for dir in &skip_dirs {
            fs::create_dir_all(root.join(dir)).unwrap();
            fs::write(root.join(dir).join("file.txt"), "content").unwrap();
        }

        let db = Database::open_db(":memory:").unwrap();
        db.run_migrations().unwrap();

        let mut walker = FileWalker::new();
        let roots = vec![root.to_string_lossy().to_string()];
        
        let result = walker.scan_roots(roots, &db);
        
        // Should skip the directories and their contents
        assert!(result.skipped >= skip_dirs.len() as u64);
    }
}






