-- LNXDrive FUSE Support Schema Extensions

-- Add FUSE-related columns to sync_items
ALTER TABLE sync_items ADD COLUMN inode INTEGER;
ALTER TABLE sync_items ADD COLUMN last_accessed DATETIME;
ALTER TABLE sync_items ADD COLUMN hydration_progress INTEGER;

-- Index for inode lookups
CREATE UNIQUE INDEX idx_sync_items_inode ON sync_items(inode) WHERE inode IS NOT NULL;

-- Table to persist the next inode number across restarts
CREATE TABLE IF NOT EXISTS inode_counter (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_inode INTEGER NOT NULL DEFAULT 2
);

-- Initialize with inode 2 (inode 1 is reserved for root)
INSERT OR IGNORE INTO inode_counter (id, next_inode) VALUES (1, 2);
