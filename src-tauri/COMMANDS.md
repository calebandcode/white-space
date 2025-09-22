# Tauri Commands Documentation

This document describes the Tauri commands implemented for the White Space application, including security measures, error handling, and usage examples.

## Overview

The commands module provides a secure interface between the frontend and backend, with comprehensive validation, error handling, and security measures. All commands are async and return structured results with proper error codes.

## Security Features

### Path Validation

- **Path Traversal Protection**: Prevents `../` and similar traversal attempts
- **Directory Restrictions**: Only allows access to user directories (Home, Desktop, Downloads, Pictures)
- **Absolute Path Validation**: Ensures paths are within allowed directories

### Input Sanitization

- **String Sanitization**: Removes control characters and limits length
- **File ID Validation**: Ensures valid file IDs and reasonable limits
- **Parameter Validation**: Comprehensive validation of all input parameters

### Error Handling

- **Structured Errors**: All errors include error codes (ERR_VALIDATION, ERR_DATABASE, etc.)
- **User-Friendly Messages**: Errors are converted to readable messages
- **Internal Error Protection**: Internal errors are sanitized before exposure

## Command Reference

### Core Commands

#### `scan_roots(roots: Vec<String>) -> Result<ScanResult, String>`

Scans specified directory roots for files.

**Parameters:**

- `roots`: Vector of directory paths to scan (max 10)

**Security:**

- Validates all paths against allowed directories
- Sanitizes path strings
- Limits number of roots

**Returns:**

- `ScanResult`: Contains scan statistics and results

**Error Codes:**

- `ERR_VALIDATION`: Invalid input or too many roots
- `ERR_SCAN`: Scan operation failed

#### `daily_candidates(max_total: usize) -> Result<Vec<Candidate>, String>`

Retrieves daily cleanup candidates.

**Parameters:**

- `max_total`: Maximum number of candidates to return (1-1000)

**Security:**

- Validates max_total range
- Uses database queries with prepared statements

**Returns:**

- `Vec<Candidate>`: List of file candidates for cleanup

**Error Codes:**

- `ERR_VALIDATION`: Invalid max_total value
- `ERR_SELECTOR`: Selection operation failed

#### `gauge_state() -> Result<GaugeState, String>`

Gets current gauge state (Potential, Staged, Freed metrics).

**Returns:**

- `GaugeState`: Current gauge metrics

**Error Codes:**

- `ERR_GAUGE`: Gauge calculation failed

### File Operations

#### `archive_files(file_ids: Vec<i64>) -> Result<ArchiveOutcome, String>`

Archives selected files.

**Parameters:**

- `file_ids`: Vector of file IDs to archive (max 1000)

**Security:**

- Validates file IDs exist in database
- Validates file paths before operations
- Uses prepared statements

**Returns:**

- `ArchiveOutcome`: Archive operation results

**Error Codes:**

- `ERR_VALIDATION`: Invalid file IDs
- `ERR_NOT_FOUND`: File not found
- `ERR_DATABASE`: Database error
- `ERR_ARCHIVE`: Archive operation failed

#### `delete_files(file_ids: Vec<i64>, to_trash: bool) -> Result<DeleteOutcome, String>`

Deletes selected files.

**Parameters:**

- `file_ids`: Vector of file IDs to delete (max 1000)
- `to_trash`: Whether to move to trash or delete permanently

**Security:**

- Same validation as archive_files
- Respects user preference for trash vs permanent deletion

**Returns:**

- `DeleteOutcome`: Delete operation results

**Error Codes:**

- `ERR_VALIDATION`: Invalid file IDs
- `ERR_NOT_FOUND`: File not found
- `ERR_DATABASE`: Database error
- `ERR_DELETE`: Delete operation failed

#### `undo_last() -> Result<UndoResult, String>`

Undoes the last batch operation.

**Returns:**

- `UndoResult`: Undo operation results

**Error Codes:**

- `ERR_UNDO`: Undo operation failed

### Review and Thumbnails

#### `get_review_items(min_age_days: u32) -> Result<Vec<StagedFile>, String>`

Gets files staged for review.

**Parameters:**

- `min_age_days`: Minimum age in days (max 365)

**Security:**

- Validates age threshold
- Uses database queries with prepared statements

**Returns:**

- `Vec<StagedFile>`: List of staged files

**Error Codes:**

- `ERR_VALIDATION`: Invalid age threshold
- `ERR_DATABASE`: Database error

#### `get_thumbnail(file_id: i64, max_px: u32) -> Result<String, String>`

Gets thumbnail for a file.

**Parameters:**

- `file_id`: File ID
- `max_px`: Maximum thumbnail size (1-2048px)

**Security:**

- Validates file ID and size
- Checks file exists in database and on disk
- Validates file path

**Returns:**

- `String`: Base64 encoded thumbnail or file path

**Error Codes:**

- `ERR_VALIDATION`: Invalid parameters
- `ERR_NOT_FOUND`: File not found
- `ERR_DATABASE`: Database error

### Preferences

#### `get_prefs() -> Result<UserPrefs, String>`

Gets user preferences.

**Returns:**

- `UserPrefs`: Current user preferences

**Error Codes:**

- `ERR_DATABASE`: Database error

#### `set_prefs(prefs: PartialUserPrefs) -> Result<(), String>`

Sets user preferences.

**Parameters:**

- `prefs`: Partial preferences to update

**Security:**

- Validates all preference values
- Sanitizes string inputs
- Uses prepared statements

**Error Codes:**

- `ERR_VALIDATION`: Invalid preference values
- `ERR_DATABASE`: Database error

## Data Structures

### ArchiveOutcome

```rust
struct ArchiveOutcome {
    success: bool,
    files_processed: usize,
    total_bytes: u64,
    duration_ms: u64,
    errors: Vec<String>,
    dry_run: bool,
}
```

### DeleteOutcome

```rust
struct DeleteOutcome {
    success: bool,
    files_processed: usize,
    total_bytes_freed: u64,
    duration_ms: u64,
    errors: Vec<String>,
    to_trash: bool,
}
```

### StagedFile

```rust
struct StagedFile {
    file_id: i64,
    path: String,
    size_bytes: u64,
    archived_at: DateTime<Utc>,
    age_days: u32,
    parent_dir: String,
}
```

### UserPrefs

```rust
struct UserPrefs {
    dry_run_default: bool,
    tidy_day: String,
    tidy_hour: u32,
    rolling_window_days: i64,
    max_candidates_per_day: usize,
    thumbnail_max_size: u32,
    auto_scan_enabled: bool,
    scan_interval_hours: u32,
    archive_age_threshold_days: u32,
    delete_age_threshold_days: u32,
}
```

### PartialUserPrefs

```rust
struct PartialUserPrefs {
    dry_run_default: Option<bool>,
    tidy_day: Option<String>,
    tidy_hour: Option<u32>,
    rolling_window_days: Option<i64>,
    max_candidates_per_day: Option<usize>,
    thumbnail_max_size: Option<u32>,
    auto_scan_enabled: Option<bool>,
    scan_interval_hours: Option<u32>,
    archive_age_threshold_days: Option<u32>,
    delete_age_threshold_days: Option<u32>,
}
```

## Error Codes

All commands return structured error messages with codes:

- **ERR_VALIDATION**: Input validation failed
- **ERR_DATABASE**: Database operation failed
- **ERR_SCAN**: File scanning failed
- **ERR_SELECTOR**: File selection failed
- **ERR_GAUGE**: Gauge calculation failed
- **ERR_ARCHIVE**: Archive operation failed
- **ERR_DELETE**: Delete operation failed
- **ERR_UNDO**: Undo operation failed
- **ERR_NOT_FOUND**: Resource not found
- **ERR_PERMISSION**: Permission denied
- **ERR_INTERNAL**: Internal error

## Usage Examples

### Frontend Integration

```typescript
// Scan directories
const scanResult = await invoke("scan_roots", {
  roots: ["/Users/me/Desktop", "/Users/me/Downloads"],
});

// Get daily candidates
const candidates = await invoke("daily_candidates", {
  maxTotal: 12,
});

// Archive files
const outcome = await invoke("archive_files", {
  fileIds: [1, 2, 3],
});

// Get preferences
const prefs = await invoke("get_prefs");

// Set preferences
await invoke("set_prefs", {
  prefs: {
    dryRunDefault: true,
    tidyDay: "Mon",
    tidyHour: 9,
  },
});
```

### Error Handling

```typescript
try {
  const result = await invoke("archive_files", { fileIds: [1, 2, 3] });
  if (result.success) {
    console.log(`Archived ${result.files_processed} files`);
  } else {
    console.error("Archive failed:", result.errors);
  }
} catch (error) {
  if (error.includes("ERR_VALIDATION")) {
    console.error("Invalid input:", error);
  } else if (error.includes("ERR_NOT_FOUND")) {
    console.error("File not found:", error);
  } else {
    console.error("Unexpected error:", error);
  }
}
```

## Security Considerations

### Path Security

- All paths are validated against allowed directories
- Path traversal attempts are blocked
- Only user directories are accessible

### Input Validation

- All inputs are validated for type and range
- String inputs are sanitized
- File IDs are validated against database

### Database Security

- All database queries use prepared statements
- No raw SQL injection possible
- Proper error handling prevents information leakage

### Error Handling

- Internal errors are sanitized
- User-friendly error messages
- Structured error codes for frontend handling

## Performance Considerations

### Async Operations

- All commands are async for non-blocking execution
- Database operations are optimized
- File operations include progress tracking

### Resource Limits

- Maximum file selection limits (1000 files)
- Maximum scan root limits (10 directories)
- Thumbnail size limits (2048px)

### Caching

- Database connections are reused
- Preferences are cached
- Thumbnails can be cached (future enhancement)

## Testing

The commands module includes comprehensive tests:

- **Unit Tests**: Individual function testing
- **Integration Tests**: End-to-end command testing
- **Security Tests**: Validation and sanitization testing
- **Error Tests**: Error handling and edge cases
- **Serialization Tests**: Data structure serialization

Run tests with:

```bash
cargo test commands
```

## Future Enhancements

### Planned Features

- **Thumbnail Caching**: Cache generated thumbnails
- **Batch Operations**: Optimize bulk operations
- **Progress Callbacks**: Real-time progress updates
- **Compression**: Compress archived files
- **Encryption**: Encrypt sensitive data

### Performance Improvements

- **Connection Pooling**: Database connection pooling
- **Async File Operations**: Non-blocking file operations
- **Memory Optimization**: Reduce memory usage
- **Parallel Processing**: Parallel file operations

## Troubleshooting

### Common Issues

**ERR_VALIDATION Errors**

- Check input parameters are within valid ranges
- Ensure file IDs are positive integers
- Verify paths are within allowed directories

**ERR_DATABASE Errors**

- Check database connection
- Verify database schema is up to date
- Check for database locks

**ERR_NOT_FOUND Errors**

- Verify files exist in database
- Check file paths are correct
- Ensure files exist on disk

### Debug Mode

Enable debug logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run
```

This will provide detailed logging of command execution and error details.






