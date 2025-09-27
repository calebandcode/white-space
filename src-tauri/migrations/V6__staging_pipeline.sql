BEGIN TRANSACTION;

-- Track whether a file is currently staged and optional cool-off timestamp
ALTER TABLE files ADD COLUMN IF NOT EXISTS is_staged INTEGER NOT NULL DEFAULT 0;
ALTER TABLE files ADD COLUMN IF NOT EXISTS cooloff_until TEXT;

-- Extend actions with origin and note metadata for richer auditing
ALTER TABLE actions ADD COLUMN IF NOT EXISTS origin TEXT;
ALTER TABLE actions ADD COLUMN IF NOT EXISTS note TEXT;

-- Table that records staged items and their lifecycle state
CREATE TABLE IF NOT EXISTS staged_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    staged_at TEXT NOT NULL DEFAULT (DATETIME('now')),
    expires_at TEXT,
    batch_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    note TEXT,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_staged_files_status ON staged_files(status);
CREATE INDEX IF NOT EXISTS idx_staged_files_expires_at ON staged_files(expires_at);
CREATE INDEX IF NOT EXISTS idx_staged_files_file_id ON staged_files(file_id);

COMMIT;
