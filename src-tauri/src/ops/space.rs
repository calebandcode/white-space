use crate::ops::error::{OpsError, OpsResult};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SpaceInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub free_percentage: f64,
}

#[derive(Debug, Clone)]
pub struct SpaceCheck {
    pub path: String,
    pub required_bytes: u64,
    pub available_bytes: u64,
    pub sufficient: bool,
    pub free_percentage: f64,
}

pub struct SpaceManager;

impl SpaceManager {
    pub fn new() -> Self {
        Self
    }

    pub fn get_available_space(&self, path: &Path) -> OpsResult<u64> {
        #[cfg(target_os = "windows")]
        {
            self.get_available_space_windows(path)
        }

        #[cfg(unix)]
        {
            self.get_available_space_unix(path)
        }

        #[cfg(not(any(target_os = "windows", unix)))]
        {
            Err(OpsError::SpaceError(
                "Unsupported platform for space checking".to_string(),
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn get_available_space_windows(&self, path: &Path) -> OpsResult<u64> {
        use std::os::windows::fs::MetadataExt;

        let metadata = fs::metadata(path)
            .map_err(|e| OpsError::SpaceError(format!("Failed to read metadata: {}", e)))?;

        // Get the volume path
        let volume_path = self.get_volume_path(path)?;

        // Use Windows API to get free space
        unsafe {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;

            let wide_path: Vec<u16> = OsString::from(volume_path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut free_bytes: u64 = 0;
            let mut total_bytes: u64 = 0;

            let result = unsafe {
                windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    Some(&mut free_bytes),
                    Some(&mut total_bytes),
                    None,
                )
            };

            match result {
                Ok(_) => Ok(free_bytes),
                Err(_) => Err(OpsError::SpaceError(
                    "Failed to get disk free space".to_string(),
                )),
            }
        }
    }

    #[cfg(unix)]
    fn get_available_space_unix(&self, path: &Path) -> OpsResult<u64> {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(path)
            .map_err(|e| OpsError::SpaceError(format!("Failed to read metadata: {}", e)))?;

        // Get the device ID to find the mount point
        let device_id = metadata.dev();

        // Find the mount point for this device
        let mount_point = self.find_mount_point(path, device_id)?;

        // Read /proc/mounts or /etc/mtab to get filesystem info
        self.read_filesystem_info(&mount_point)
    }

    fn get_volume_path(&self, path: &Path) -> OpsResult<PathBuf> {
        // Get the root of the volume
        let mut current = path.to_path_buf();

        while let Some(parent) = current.parent() {
            if parent == current {
                break;
            }
            current = parent.to_path_buf();
        }

        Ok(current)
    }

    #[cfg(unix)]
    fn find_mount_point(&self, path: &Path, device_id: u64) -> OpsResult<PathBuf> {
        // Simple implementation - in practice, you'd parse /proc/mounts
        // For now, just return the path itself
        Ok(path.to_path_buf())
    }

    #[cfg(unix)]
    fn read_filesystem_info(&self, mount_point: &Path) -> OpsResult<u64> {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(mount_point)
            .map_err(|e| OpsError::SpaceError(format!("Failed to get metadata: {}", e)))?;

        // For Unix systems, we can use statvfs for more accurate space info
        // For now, return the available space from metadata
        Ok(metadata.blocks() * 512) // blocks * block_size
    }

    pub fn get_space_info(&self, path: &Path) -> OpsResult<SpaceInfo> {
        let available = self.get_available_space(path)?;
        let total = self.get_total_space(path)?;
        let used = total - available;
        let free_percentage = (available as f64 / total as f64) * 100.0;

        Ok(SpaceInfo {
            total_bytes: total,
            available_bytes: available,
            used_bytes: used,
            free_percentage,
        })
    }

    fn get_total_space(&self, path: &Path) -> OpsResult<u64> {
        #[cfg(target_os = "windows")]
        {
            self.get_total_space_windows(path)
        }

        #[cfg(unix)]
        {
            self.get_total_space_unix(path)
        }

        #[cfg(not(any(target_os = "windows", unix)))]
        {
            Err(OpsError::SpaceError(
                "Unsupported platform for space checking".to_string(),
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn get_total_space_windows(&self, path: &Path) -> OpsResult<u64> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        let volume_path = self.get_volume_path(path)?;
        let wide_path: Vec<u16> = OsString::from(volume_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let mut free_bytes: u64 = 0;
            let mut total_bytes: u64 = 0;

            let result = unsafe {
                windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                    windows::core::PCWSTR(wide_path.as_ptr()),
                    Some(&mut free_bytes),
                    Some(&mut total_bytes),
                    None,
                )
            };

            match result {
                Ok(_) => Ok(total_bytes),
                Err(_) => Err(OpsError::SpaceError(
                    "Failed to get total disk space".to_string(),
                )),
            }
        }
    }

    #[cfg(unix)]
    fn get_total_space_unix(&self, path: &Path) -> OpsResult<u64> {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::metadata(path)
            .map_err(|e| OpsError::SpaceError(format!("Failed to get metadata: {}", e)))?;

        // For Unix systems, we can use statvfs for more accurate space info
        // For now, return the total space from metadata
        Ok(metadata.blocks() * 512) // blocks * block_size
    }

    pub fn check_space_requirements(
        &self,
        paths: Vec<String>,
        required_bytes: u64,
    ) -> OpsResult<Vec<SpaceCheck>> {
        let mut checks = Vec::new();

        for path_str in paths {
            let path = Path::new(&path_str);
            let available = self.get_available_space(path)?;
            let total = self.get_total_space(path)?;
            let free_percentage = (available as f64 / total as f64) * 100.0;

            checks.push(SpaceCheck {
                path: path_str,
                required_bytes,
                available_bytes: available,
                sufficient: available >= required_bytes,
                free_percentage,
            });
        }

        Ok(checks)
    }

    pub fn format_bytes(&self, bytes: u64) -> String {
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

    pub fn calculate_directory_size(&self, path: &Path) -> OpsResult<u64> {
        let mut total_size = 0u64;

        if path.is_file() {
            return Ok(fs::metadata(path)?.len());
        }

        if path.is_dir() {
            let entries = fs::read_dir(path)
                .map_err(|e| OpsError::SpaceError(format!("Failed to read directory: {}", e)))?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    OpsError::SpaceError(format!("Failed to read directory entry: {}", e))
                })?;

                let entry_path = entry.path();
                total_size += self.calculate_directory_size(&entry_path)?;
            }
        }

        Ok(total_size)
    }

    pub fn get_largest_files(&self, path: &Path, limit: usize) -> OpsResult<Vec<(String, u64)>> {
        let mut files = Vec::new();
        self.collect_files(path, &mut files)?;

        // Sort by size (largest first)
        files.sort_by(|a, b| b.1.cmp(&a.1));

        // Take only the requested number
        files.truncate(limit);

        Ok(files)
    }

    fn collect_files(&self, path: &Path, files: &mut Vec<(String, u64)>) -> OpsResult<()> {
        if path.is_file() {
            let size = fs::metadata(path)?.len();
            files.push((path.to_string_lossy().to_string(), size));
        } else if path.is_dir() {
            let entries = fs::read_dir(path)
                .map_err(|e| OpsError::SpaceError(format!("Failed to read directory: {}", e)))?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    OpsError::SpaceError(format!("Failed to read directory entry: {}", e))
                })?;

                let entry_path = entry.path();
                self.collect_files(&entry_path, files)?;
            }
        }

        Ok(())
    }

    pub fn estimate_cleanup_impact(&self, files: Vec<String>) -> OpsResult<u64> {
        let mut total_bytes = 0u64;

        for file_path in files {
            let path = Path::new(&file_path);
            if path.exists() {
                total_bytes += self.calculate_directory_size(path)?;
            }
        }

        Ok(total_bytes)
    }
}

impl Default for SpaceManager {
    fn default() -> Self {
        Self::new()
    }
}
