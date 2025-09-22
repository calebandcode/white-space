# File Scanner Implementation

This document describes the file scanner implementation for the White Space application, including file walking, active project detection, and performance optimization.

## Overview

The scanner is designed to efficiently traverse user-selected directory roots, detect active development projects, and track file metadata with performance targets of processing 50,000 files in under 90 seconds.

## Architecture

### Core Components

1. **FileWalker** (`scanner/file_walker.rs`) - Directory traversal and file processing
2. **ActiveProjectDetector** (`scanner/active_project.rs`) - Development repository detection
3. **Scanner** (`scanner/mod.rs`) - Main orchestrator and API

### Key Features

- **Smart Filtering**: Skips `.git`, `node_modules`, `.DS_Store`, `Thumbs.db`, and symlinks
- **Depth Limiting**: Maximum depth of 5 levels to prevent infinite recursion
- **Dev Repo Detection**: Identifies Git repositories and analyzes project activity
- **Performance Tracking**: Monitors scan duration and throughput
- **Incremental Scanning**: Updates only changed files for subsequent scans

## API Reference

### Tauri Commands

#### `scan_roots(roots: Vec<String>) -> ScanResult`

Performs a full scan of the specified directory roots.

```typescript
const result = await invoke("scan_roots", {
  roots: ["/Users/username/Desktop", "/Users/username/Downloads"],
});
```

**Returns:**

```typescript
interface ScanResult {
  counted: number; // Number of files processed
  skipped: number; // Number of files/directories skipped
  duration_ms: number; // Scan duration in milliseconds
  errors: string[]; // Any errors encountered
}
```

#### `incremental_scan() -> ScanResult`

Performs an incremental scan, updating only changed files.

```typescript
const result = await invoke("incremental_scan");
```

#### `get_default_scan_roots() -> Vec<String>`

Returns default scan directories based on the user's home directory.

```typescript
const roots = await invoke("get_default_scan_roots");
// Returns: ['/Users/username/Desktop', '/Users/username/Downloads', ...]
```

#### `analyze_recent_burst(directory: String) -> RecentBurst`

Analyzes recent file modification activity in a directory.

```typescript
const burst = await invoke("analyze_recent_burst", {
  directory: "/Users/username/Projects",
});
```

**Returns:**

```typescript
interface RecentBurst {
  directory: string; // Directory path
  modified_count: number; // Files modified in time window
  time_window_hours: number; // Time window (72 hours)
  is_burst: boolean; // Whether threshold was exceeded
}
```

## File Processing

### Metadata Extraction

For each file, the scanner extracts:

- **Path**: Full file system path
- **Parent Directory**: Immediate parent directory
- **Size**: File size in bytes
- **Timestamps**: Creation and modification times (best-effort)
- **MIME Type**: Detected from file extension
- **SHA1 Hash**: Optional for duplicate detection

### MIME Type Detection

The scanner includes built-in MIME type detection for common file types:

| Extension       | MIME Type                |
| --------------- | ------------------------ |
| `.txt`          | `text/plain`             |
| `.md`           | `text/markdown`          |
| `.html`         | `text/html`              |
| `.css`          | `text/css`               |
| `.js`           | `application/javascript` |
| `.json`         | `application/json`       |
| `.pdf`          | `application/pdf`        |
| `.jpg`, `.jpeg` | `image/jpeg`             |
| `.png`          | `image/png`              |
| `.gif`          | `image/gif`              |
| `.mp4`          | `video/mp4`              |
| `.mp3`          | `audio/mpeg`             |
| `.zip`          | `application/zip`        |
| `.tar`          | `application/x-tar`      |
| `.gz`           | `application/gzip`       |

## Active Project Detection

### Git Repository Detection

The scanner automatically detects Git repositories by:

1. Walking directory trees looking for `.git` directories
2. Analyzing repository metadata and activity
3. Identifying project keywords in directory names

### Keyword Flags

Projects are flagged based on directory names containing:

- `current` - Currently active projects
- `project` - General project directories
- `active` - Explicitly active projects
- `wip` - Work in progress
- `final` - Final/completed projects

### Activity Analysis

The scanner determines project activity by:

- **Recent Git Activity**: Last commit within 7 days
- **File Modification Bursts**: 3+ files modified within 72 hours
- **Directory Structure**: Presence of development artifacts

## Performance Optimization

### Skip Lists

The scanner automatically skips:

**Directories:**

- `.git` - Git repositories (scanned separately)
- `node_modules` - Node.js dependencies
- `.DS_Store` - macOS metadata
- `Thumbs.db` - Windows thumbnails

**Files:**

- `.DS_Store` - macOS metadata
- `Thumbs.db` - Windows thumbnails
- Symlinks - Symbolic links

### Depth Limiting

- Maximum depth of 5 levels prevents infinite recursion
- Protects against deeply nested directory structures
- Balances thoroughness with performance

### Performance Targets

- **Target**: 50,000 files in < 90 seconds
- **Monitoring**: Real-time performance metrics
- **Optimization**: Efficient directory traversal and metadata extraction

## Database Integration

### Metrics Recording

The scanner records performance metrics:

- `scan_duration_ms` - Total scan time
- `files_counted` - Files processed
- `files_skipped` - Files/directories skipped
- `files_per_second` - Throughput rate
- `performance_target_met` - Whether target was achieved

### Project Metrics

- `project_active` - Number of active projects
- `project_inactive` - Number of inactive projects
- `total_dev_repos` - Total Git repositories found
- `flag_current`, `flag_wip`, etc. - Keyword flag counts

## Error Handling

### Graceful Degradation

- Continues scanning on individual file errors
- Records errors in `ScanResult.errors`
- Logs detailed error information

### Common Error Scenarios

- **Permission Denied**: Skip inaccessible directories
- **Broken Symlinks**: Skip and continue
- **Corrupted Files**: Skip and log error
- **Network Drives**: Handle timeouts gracefully

## Testing

### Test Corpus

The test suite creates a comprehensive temporary directory structure:

```
temp_dir/
├── Desktop/
│   ├── document.txt
│   └── image.jpg
├── Downloads/
│   ├── file.zip
│   └── video.mp4
├── Pictures/Screenshots/
│   └── screenshot.png
├── Documents/Projects/
│   ├── current-project/
│   ├── wip-project/
│   ├── final-project/
│   └── old-project/
├── Code/
│   ├── active-repo/
│   └── inactive-repo/
├── node_modules/
└── .git/
```

### Test Coverage

- **Basic Scanning**: File counting and skipping
- **Depth Limiting**: Deep directory structures
- **Project Detection**: Git repository identification
- **Burst Detection**: Recent modification analysis
- **Performance**: Timing and throughput validation
- **Error Handling**: Graceful failure scenarios

## Usage Examples

### Basic Scanning

```rust
use crate::scanner::Scanner;
use crate::db::Database;

let db = Database::open_db("database.db")?;
let mut scanner = Scanner::new();
let roots = vec!["/Users/username/Desktop".to_string()];

let result = scanner.scan_roots(roots, &db);
println!("Scanned {} files in {}ms", result.counted, result.duration_ms);
```

### Project Analysis

```rust
use crate::scanner::active_project::ActiveProjectDetector;

let detector = ActiveProjectDetector::new();
let roots = vec!["/Users/username/Code".to_string()];
let repos = detector.detect_dev_repos(&roots);

for repo in repos {
    println!("Repo: {:?}, Active: {}", repo.path, repo.is_active);
}
```

### Burst Detection

```rust
let burst = detector.detect_recent_burst("/Users/username/Projects")?;
if burst.is_burst {
    println!("Burst detected: {} files modified", burst.modified_count);
}
```

## Configuration

### Default Scan Roots

The scanner automatically detects common user directories:

- `~/Desktop`
- `~/Downloads`
- `~/Pictures`
- `~/Documents`
- `~/Projects`
- `~/Code`
- `~/dev`

### Customization

Scan behavior can be customized by modifying:

- **Skip Lists**: Add additional directories/files to skip
- **Depth Limit**: Adjust maximum traversal depth
- **Performance Target**: Modify timing expectations
- **Keyword Patterns**: Add new project identification patterns

## Future Enhancements

- **Parallel Processing**: Multi-threaded directory traversal
- **Incremental Hashing**: SHA1 calculation for duplicate detection
- **Smart Caching**: Cache metadata to avoid re-scanning
- **Network Support**: Handle network drives and cloud storage
- **Real-time Monitoring**: Watch for file system changes






