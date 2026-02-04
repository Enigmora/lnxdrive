-- LNXDrive Initial Schema

-- Accounts table
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    onedrive_id TEXT NOT NULL,
    sync_root TEXT NOT NULL,
    quota_used INTEGER NOT NULL DEFAULT 0,
    quota_total INTEGER NOT NULL DEFAULT 0,
    delta_token TEXT,
    last_sync TEXT,
    state TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync items table
CREATE TABLE IF NOT EXISTS sync_items (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    local_path TEXT NOT NULL,
    remote_id TEXT,
    remote_path TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'online',
    content_hash TEXT,
    local_hash TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_sync TEXT,
    last_modified_local TEXT,
    last_modified_remote TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    error_info TEXT,
    UNIQUE(account_id, local_path),
    UNIQUE(account_id, remote_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_items_state ON sync_items(state);
CREATE INDEX IF NOT EXISTS idx_sync_items_path ON sync_items(local_path);
CREATE INDEX IF NOT EXISTS idx_sync_items_remote ON sync_items(remote_id);

-- Sync sessions table
CREATE TABLE IF NOT EXISTS sync_sessions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    items_total INTEGER NOT NULL DEFAULT 0,
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_succeeded INTEGER NOT NULL DEFAULT 0,
    items_failed INTEGER NOT NULL DEFAULT 0,
    bytes_uploaded INTEGER NOT NULL DEFAULT 0,
    bytes_downloaded INTEGER NOT NULL DEFAULT 0,
    delta_token_start TEXT,
    delta_token_end TEXT,
    errors TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_sync_sessions_account ON sync_sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_sync_sessions_status ON sync_sessions(status);

-- Audit log table
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    session_id TEXT REFERENCES sync_sessions(id),
    item_id TEXT,
    action TEXT NOT NULL,
    result TEXT NOT NULL,
    details TEXT NOT NULL DEFAULT '{}',
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_log_item ON audit_log(item_id);

-- Conflicts table
CREATE TABLE IF NOT EXISTS conflicts (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES sync_items(id) ON DELETE CASCADE,
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    local_version TEXT NOT NULL,
    remote_version TEXT NOT NULL,
    resolution TEXT,
    resolved_at TEXT,
    resolved_by TEXT
);

CREATE INDEX IF NOT EXISTS idx_conflicts_item ON conflicts(item_id);
CREATE INDEX IF NOT EXISTS idx_conflicts_unresolved ON conflicts(resolution) WHERE resolution IS NULL;

-- Configuration table
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
