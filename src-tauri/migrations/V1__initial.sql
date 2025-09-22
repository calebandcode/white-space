-- Enable WAL mode for better concurrency
PRAGMA journal_mode=WAL;

-- Files table for tracking file metadata
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    parent_dir TEXT NOT NULL,
    mime TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_opened_at DATETIME,
    sha1 TEXT,
    first_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE
);

-- Actions table for tracking file operations
CREATE TABLE actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('archive', 'delete', 'restore')),
    batch_id TEXT,
    src_path TEXT,
    dst_path TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- Preferences table for application settings
CREATE TABLE prefs (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Metrics table for analytics and statistics
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric TEXT NOT NULL,
    value REAL NOT NULL,
    context TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance optimization
CREATE INDEX idx_files_parent_dir ON files(parent_dir);
CREATE INDEX idx_files_last_seen_at ON files(last_seen_at);
CREATE INDEX idx_files_is_deleted ON files(is_deleted);
CREATE INDEX idx_files_mime ON files(mime);
CREATE INDEX idx_files_created_at ON files(created_at);

CREATE INDEX idx_actions_batch_id ON actions(batch_id);
CREATE INDEX idx_actions_action_created_at ON actions(action, created_at);
CREATE INDEX idx_actions_file_id ON actions(file_id);
CREATE INDEX idx_actions_created_at ON actions(created_at);

CREATE INDEX idx_metrics_metric ON metrics(metric);
CREATE INDEX idx_metrics_created_at ON metrics(created_at);
CREATE INDEX idx_metrics_context ON metrics(context);

-- Insert default preferences
INSERT INTO prefs (key, value) VALUES 
    ('app_version', '0.1.0'),
    ('last_scan', ''),
    ('scan_interval_hours', '24'),
    ('auto_archive_days', '30'),
    ('enable_metrics', 'true');






