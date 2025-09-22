# Database Schema and API

This document describes the SQLite database schema and Rust API for file tracking and actions.

## Schema Overview

The database consists of four main tables:

### `files` Table

Tracks file metadata and lifecycle information.

| Column           | Type                | Description                       |
| ---------------- | ------------------- | --------------------------------- |
| `id`             | INTEGER PRIMARY KEY | Auto-incrementing ID              |
| `path`           | TEXT UNIQUE         | Full file path                    |
| `parent_dir`     | TEXT                | Parent directory path             |
| `mime`           | TEXT                | MIME type (optional)              |
| `size_bytes`     | INTEGER             | File size in bytes                |
| `created_at`     | DATETIME            | When record was created           |
| `last_opened_at` | DATETIME            | Last time file was accessed       |
| `sha1`           | TEXT                | SHA1 hash (optional)              |
| `first_seen_at`  | DATETIME            | When file was first discovered    |
| `last_seen_at`   | DATETIME            | When file was last seen           |
| `is_deleted`     | BOOLEAN             | Whether file is marked as deleted |

### `actions` Table

Tracks file operations and actions.

| Column       | Type                | Description                                 |
| ------------ | ------------------- | ------------------------------------------- |
| `id`         | INTEGER PRIMARY KEY | Auto-incrementing ID                        |
| `file_id`    | INTEGER             | Foreign key to files.id                     |
| `action`     | TEXT                | Action type: 'archive', 'delete', 'restore' |
| `batch_id`   | TEXT                | Batch identifier (optional)                 |
| `src_path`   | TEXT                | Source path (optional)                      |
| `dst_path`   | TEXT                | Destination path (optional)                 |
| `created_at` | DATETIME            | When action was performed                   |

### `prefs` Table

Application preferences and settings.

| Column  | Type             | Description      |
| ------- | ---------------- | ---------------- |
| `key`   | TEXT PRIMARY KEY | Preference key   |
| `value` | TEXT             | Preference value |

### `metrics` Table

Analytics and performance metrics.

| Column       | Type                | Description                   |
| ------------ | ------------------- | ----------------------------- |
| `id`         | INTEGER PRIMARY KEY | Auto-incrementing ID          |
| `metric`     | TEXT                | Metric name                   |
| `value`      | REAL                | Metric value                  |
| `context`    | TEXT                | Additional context (optional) |
| `created_at` | DATETIME            | When metric was recorded      |

## Indexes

The following indexes are created for performance:

- `idx_files_parent_dir` - For directory-based queries
- `idx_files_last_seen_at` - For age-based queries
- `idx_files_is_deleted` - For filtering deleted files
- `idx_files_mime` - For MIME type filtering
- `idx_files_created_at` - For creation date queries
- `idx_actions_batch_id` - For batch operations
- `idx_actions_action_created_at` - For action type and date queries
- `idx_actions_file_id` - For file-specific actions
- `idx_actions_created_at` - For action date queries
- `idx_metrics_metric` - For metric name queries
- `idx_metrics_created_at` - For metric date queries
- `idx_metrics_context` - For context-based queries

## Rust API

### Database Connection

```rust
use crate::db::Database;

// Open database connection
let db = Database::open_db("path/to/database.db")?;

// Run migrations
db.run_migrations()?;
```

### Query Helpers

#### `by_age(min_days, max_days)`

Find files by age range.

```rust
// Files older than 30 days
let old_files = db.by_age(30, None)?;

// Files between 7 and 30 days old
let recent_files = db.by_age(7, Some(30))?;
```

#### `by_dir(parent_dir)`

Find files in a specific directory.

```rust
let user_files = db.by_dir("/home/user")?;
```

#### `latest_action(file_id)`

Get the most recent action for a file.

```rust
if let Some(action) = db.latest_action(file_id)? {
    println!("Latest action: {:?}", action);
}
```

#### `weekly_totals(weeks_back)`

Get weekly statistics for the last N weeks.

```rust
let stats = db.weekly_totals(4)?; // Last 4 weeks
for week in stats {
    println!("Week {}: {} files processed", week.week_start, week.total_files);
}
```

### CRUD Operations

#### Insert File

```rust
let new_file = NewFile {
    path: "/path/to/file.txt".to_string(),
    parent_dir: "/path/to".to_string(),
    mime: Some("text/plain".to_string()),
    size_bytes: 1024,
    sha1: Some("abc123".to_string()),
};
let file_id = db.insert_file(new_file)?;
```

#### Insert Action

```rust
let new_action = NewAction {
    file_id: 1,
    action: ActionType::Archive,
    batch_id: Some("batch_001".to_string()),
    src_path: Some("/original/path".to_string()),
    dst_path: Some("/archive/path".to_string()),
};
let action_id = db.insert_action(new_action)?;
```

#### Insert Metric

```rust
let new_metric = NewMetric {
    metric: "files_processed".to_string(),
    value: 1.0,
    context: Some("daily_scan".to_string()),
};
let metric_id = db.insert_metric(new_metric)?;
```

#### Preferences

```rust
// Set preference
db.set_preference("last_scan", "2024-01-01T00:00:00Z")?;

// Get preference
if let Some(value) = db.get_preference("last_scan")? {
    println!("Last scan: {}", value);
}
```

## Tauri Commands

The following Tauri commands are available from the frontend:

- `get_files_by_age(min_days: i64, max_days: Option<i64>)` - Get files by age
- `get_files_by_dir(parent_dir: String)` - Get files by directory
- `get_weekly_totals(weeks_back: i64)` - Get weekly statistics

## Migration System

The database uses a simple migration system:

1. Creates `refinery_schema_history` table to track migrations
2. Runs `V1__initial.sql` migration on first run
3. Can be extended with additional migrations

## Default Preferences

The following default preferences are created:

- `app_version`: "0.1.0"
- `last_scan`: ""
- `scan_interval_hours`: "24"
- `auto_archive_days`: "30"
- `enable_metrics`: "true"

## Performance Considerations

- WAL mode is enabled for better concurrency
- Indexes are created for common query patterns
- Foreign key constraints ensure data integrity
- Batch operations are supported via `batch_id`

## Error Handling

All database operations return `Result<T, rusqlite::Error>` and are properly handled in the Tauri commands with error conversion to strings for frontend consumption.






