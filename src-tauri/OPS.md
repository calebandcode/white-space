# File Operations Implementation

This document describes the comprehensive file operations system for the White Space application, including archive, delete, undo, and space management capabilities.

## Overview

The operations system provides safe, reliable file management with comprehensive error handling, progress tracking, and rollback capabilities. It supports cross-platform operations with platform-specific optimizations.

## Architecture

### Core Components

1. **ArchiveManager** (`ops/archive.rs`) - File archiving with preflight checks
2. **DeleteManager** (`ops/delete.rs`) - File deletion with trash support
3. **UndoManager** (`ops/undo.rs`) - Batch rollback functionality
4. **SpaceManager** (`ops/space.rs`) - Disk space management and validation
5. **Error Handling** (`ops/error.rs`) - Comprehensive error types and user messages

### Key Features

- **Safe Operations**: Preflight checks and validation before operations
- **Cross-Platform**: Windows, macOS, and Linux support
- **Progress Tracking**: Real-time progress for large file operations
- **Rollback Support**: Complete batch rollback on failures
- **Space Management**: Disk space validation and monitoring
- **User-Friendly Errors**: Clear error messages with recovery suggestions

## Archive Operations

### Archive Configuration

```rust
struct ArchiveConfig {
    base_path: PathBuf,           // Archive root directory
    date_format: String,          // Date format for daily folders
    free_space_buffer: f64,       // Free space buffer percentage
    progress_threshold: u64,      // Progress reporting threshold
}
```

### Archive Process

1. **Preflight Checks**:

   - Verify archive directory can be created
   - Check write permissions
   - Validate sufficient disk space (required + 5% buffer)
   - Confirm all source files exist

2. **File Processing**:

   - Attempt fast move operation first
   - Fallback to copy + verify + delete for cross-volume
   - Handle filename conflicts with " (n)" suffix
   - Log each action with batch ID

3. **Progress Tracking**:
   - Emit progress for files > 500MB
   - Real-time status updates
   - Error collection and reporting

### Archive Destinations

- **Windows**: `C:\Users\<user>\Archive\WhiteSpace\YYYY-MM-DD`
- **macOS/Linux**: `~/Archive/White Space/YYYY-MM-DD`

## Delete Operations

### Delete Configuration

```rust
struct DeleteConfig {
    use_trash: bool,                    // Use system trash
    permanent_delete: bool,             // Permanent deletion
    archive_age_threshold_days: i64,     // Archive age threshold
    confirm_permanent: bool,             // Confirm permanent deletion
}
```

### Delete Process

1. **Trash Support**:

   - **Windows**: Recycle Bin (`%USERPROFILE%\AppData\Local\Microsoft\Windows\Explorer`)
   - **macOS**: Trash (`~/.Trash`)
   - **Linux**: Trash (`~/.local/share/Trash/files`)

2. **Conflict Resolution**:

   - Handle filename conflicts in trash
   - Append " (n)" suffix for duplicates

3. **Weekly Review**:
   - Preselect archives ≥ 7 days old
   - Default to trash, allow permanent in settings

## Undo Operations

### Undo Capabilities

- **Archive → Restore**: Move files back from archive
- **Delete → Restore**: Restore from trash (if supported)
- **Batch Rollback**: Revert entire batch on any failure

### Undo Process

1. **Batch Identification**: Find most recent batch
2. **Action Reversal**: Reverse each action in the batch
3. **Rollback on Failure**: If any action fails, rollback all successful moves
4. **Logging**: Record restore actions in database

### Supported Actions

- ✅ Archive operations
- ✅ Delete operations
- ❌ Restore operations (cannot undo)

## Space Management

### Space Information

```rust
struct SpaceInfo {
    total_bytes: u64,        // Total disk space
    available_bytes: u64,    // Available space
    used_bytes: u64,         // Used space
    free_percentage: f64,    // Free space percentage
}
```

### Platform-Specific Implementation

- **Windows**: Uses `GetDiskFreeSpaceExW` API
- **macOS/Linux**: Uses `statvfs` system call
- **Fallback**: Generic implementation for other platforms

### Space Validation

- **Preflight Checks**: Verify sufficient space before operations
- **Buffer Requirements**: Require 5% additional free space
- **Real-time Monitoring**: Continuous space monitoring during operations

## Error Handling

### Error Types

```rust
enum OpsError {
    ArchiveError(String),
    DeleteError(String),
    UndoError(String),
    SpaceError(String),
    PermissionError(String),
    FileNotFound(String),
    InvalidPath(String),
    CrossVolumeError(String),
    BatchError(String),
    DatabaseError(String),
}
```

### User-Friendly Messages

Each error includes:

- **Title**: Clear error category
- **Message**: Detailed error description
- **Suggestion**: Recovery recommendation
- **Recoverable**: Whether error can be retried

### Error Context

```rust
struct ErrorContext {
    operation: String,           // Operation being performed
    file_path: Option<String>,   // File being processed
    batch_id: Option<String>,    // Batch identifier
    timestamp: DateTime<Utc>,    // Error timestamp
}
```

### Recovery Strategies

- **Retry**: Permission errors, temporary failures
- **Skip**: File not found, individual file failures
- **Abort**: Insufficient space, critical failures
- **Fallback**: Cross-volume operations

## API Reference

### Tauri Commands

#### `archive_files(file_paths: Vec<String>) -> ArchiveResult`

Archives files to daily-organized directories.

```typescript
const result = await invoke("archive_files", {
  file_paths: ["/path/to/file1.txt", "/path/to/file2.txt"],
});
```

**Returns:**

```typescript
interface ArchiveResult {
  batch_id: string; // Unique batch identifier
  files_archived: number; // Number of files successfully archived
  total_bytes: number; // Total bytes processed
  duration_ms: number; // Operation duration
  errors: string[]; // Any errors encountered
}
```

#### `delete_files(file_paths: Vec<String>) -> DeleteResult`

Deletes files to trash or permanently.

```typescript
const result = await invoke("delete_files", {
  file_paths: ["/path/to/file1.txt", "/path/to/file2.txt"],
});
```

**Returns:**

```typescript
interface DeleteResult {
  batch_id: string; // Unique batch identifier
  files_deleted: number; // Number of files successfully deleted
  total_bytes_freed: number; // Total bytes freed
  duration_ms: number; // Operation duration
  errors: string[]; // Any errors encountered
  trash_path?: string; // Trash directory path
}
```

#### `undo_last() -> UndoResult`

Reverses the most recent batch operation.

```typescript
const result = await invoke("undo_last");
```

**Returns:**

```typescript
interface UndoResult {
  batch_id: string; // Batch that was undone
  actions_reversed: number; // Number of actions reversed
  files_restored: number; // Number of files restored
  duration_ms: number; // Operation duration
  errors: string[]; // Any errors encountered
  rollback_performed: boolean; // Whether rollback was needed
}
```

#### `get_space_info(path: string) -> SpaceInfo`

Gets disk space information for a path.

```typescript
const info = await invoke("get_space_info", { path: "/path/to/directory" });
```

#### `check_space_requirements(paths: string[], required_bytes: number) -> SpaceCheck[]`

Checks if multiple paths have sufficient space.

```typescript
const checks = await invoke("check_space_requirements", {
  paths: ["/path1", "/path2"],
  required_bytes: 1024 * 1024 * 100, // 100MB
});
```

## Usage Examples

### Basic Archive Operation

```rust
use crate::ops::ArchiveManager;
use crate::db::Database;

let db = Database::open_db("database.db")?;
let mut archive_manager = ArchiveManager::new();
let file_paths = vec!["/path/to/file1.txt".to_string()];

let result = archive_manager.archive_files(file_paths, &db)?;
println!("Archived {} files in {}ms", result.files_archived, result.duration_ms);
```

### Delete with Trash Support

```rust
use crate::ops::DeleteManager;

let mut delete_manager = DeleteManager::new();
delete_manager.set_use_trash(true);

let result = delete_manager.delete_files(file_paths, &db)?;
if let Some(trash_path) = result.trash_path {
    println!("Files moved to trash: {}", trash_path);
}
```

### Undo Operation

```rust
use crate::ops::UndoManager;

let mut undo_manager = UndoManager::new();
let result = undo_manager.undo_last(&db)?;

if result.rollback_performed {
    println!("Rollback performed due to errors");
}
```

### Space Validation

```rust
use crate::ops::SpaceManager;

let space_manager = SpaceManager::new();
let space_info = space_manager.get_space_info(path)?;

if space_info.free_percentage < 10.0 {
    println!("Warning: Low disk space ({:.1}%)", space_info.free_percentage);
}
```

## Testing

### Test Coverage

The test suite covers:

- **Archive Operations**: File archiving with conflict resolution
- **Delete Operations**: Trash and permanent deletion
- **Undo Operations**: Batch rollback and restoration
- **Space Management**: Disk space validation and monitoring
- **Error Handling**: Comprehensive error scenarios
- **Cross-Volume Simulation**: Cross-volume operation testing
- **Configuration**: Config updates and validation

### Integration Tests

- **Temp Directory Corpus**: Realistic file structures
- **Cross-Volume Simulation**: Different mount points
- **Batch Operations**: Multi-file operations
- **Error Scenarios**: Permission, space, and file errors
- **Rollback Testing**: Failure recovery validation

### Edge Cases Tested

- **Filename Conflicts**: Duplicate name resolution
- **Permission Errors**: Access denied scenarios
- **Space Exhaustion**: Insufficient disk space
- **Cross-Volume Operations**: Different file systems
- **Partial Failures**: Batch rollback scenarios
- **Large Files**: Progress tracking validation

## Performance Considerations

### Optimization Strategies

- **Fast Move First**: Attempt rename before copy+delete
- **Batch Processing**: Process multiple files efficiently
- **Progress Tracking**: Real-time updates for large files
- **Space Validation**: Preflight checks prevent failures
- **Error Recovery**: Graceful degradation on failures

### Scalability

- **Large Files**: Efficient handling of multi-gigabyte files
- **Batch Operations**: Optimized multi-file processing
- **Memory Usage**: Minimal memory footprint
- **Cross-Platform**: Platform-specific optimizations

## Security Considerations

### File Safety

- **Preflight Validation**: Verify operations before execution
- **Atomic Operations**: Ensure operations complete or rollback
- **Permission Checks**: Validate access rights
- **Space Validation**: Prevent disk space exhaustion

### Data Integrity

- **Copy Verification**: Verify file integrity after copy
- **Sync Operations**: Force data to disk
- **Rollback Support**: Complete operation reversal
- **Error Logging**: Comprehensive operation logging

## Future Enhancements

### Planned Features

- **Parallel Processing**: Multi-threaded file operations
- **Compression**: Archive compression for space savings
- **Encryption**: Secure archive encryption
- **Cloud Integration**: Cloud storage support
- **Advanced Scheduling**: Automated cleanup scheduling

### Potential Improvements

- **Incremental Operations**: Only process changed files
- **Smart Caching**: Cache operation results
- **Advanced Progress**: More detailed progress information
- **User Preferences**: Customizable operation behavior
- **Analytics**: Operation performance metrics






