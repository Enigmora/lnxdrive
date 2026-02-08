# Data Model: Files-on-Demand (FUSE Virtual Filesystem)

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04

---

## Entity Changes

### Modified Entity: ItemState (enum)

**Location**: `crates/lnxdrive-core/src/domain/sync_item.rs`

**Current variants**: `Online`, `Hydrating`, `Hydrated`, `Modified`, `Conflicted`, `Error(String)`, `Deleted`

**Added variant**: `Pinned`

```
ItemState:
  - Online        # Placeholder — metadata only, no local content
  - Hydrating     # Content being downloaded from cloud
  - Hydrated      # Content fully available locally
  - Pinned        # Content available locally, immune to auto-dehydration
  - Modified      # Local changes pending upload
  - Conflicted    # Conflict between local and remote
  - Error(String) # Error state with reason
  - Deleted       # Marked for deletion
```

**New state transitions**:
```
Hydrated  → Pinned    (user pins file)
Pinned    → Hydrated  (user unpins file)
Online    → Pinned    (user pins placeholder — triggers hydration first)
Pinned    → Modified  (user edits pinned file)
Modified  → Pinned    (sync completes on pinned file)
```

**Updated helper methods**:
- `is_local()`: Returns true for `Hydrated`, `Pinned`, `Modified` (content exists locally)
- `is_pinned()`: New method, returns true for `Pinned`
- `can_dehydrate()`: New method, returns true only for `Hydrated` (not Pinned, Modified, etc.)

---

### Modified Entity: SyncItem (struct)

**Location**: `crates/lnxdrive-core/src/domain/sync_item.rs`

**New fields**:

| Field | Type | Description |
|-------|------|-------------|
| `inode` | `Option<u64>` | FUSE inode number, assigned when item enters the FUSE filesystem |
| `last_accessed` | `Option<DateTime<Utc>>` | Last time the file was opened/read via FUSE, for LRU dehydration |
| `hydration_progress` | `Option<u8>` | Hydration progress 0-100, set during `Hydrating` state |

---

### New Entity: InodeEntry

**Location**: `crates/lnxdrive-fuse/src/inode.rs`

In-memory representation of a FUSE inode, used by the inode table for fast lookups.

| Field | Type | Description |
|-------|------|-------------|
| `ino` | `u64` | FUSE inode number |
| `item_id` | `UniqueId` | Reference to the SyncItem |
| `remote_id` | `Option<RemoteId>` | OneDrive item ID |
| `parent_ino` | `u64` | Parent directory inode |
| `name` | `String` | Entry name in parent directory |
| `kind` | `FileType` | File or Directory |
| `size` | `u64` | Real file size (from cloud) |
| `perm` | `u16` | Unix permissions |
| `mtime` | `SystemTime` | Last modification time |
| `ctime` | `SystemTime` | Last metadata change time |
| `atime` | `SystemTime` | Last access time |
| `nlink` | `u32` | Number of hard links |
| `lookup_count` | `u64` | Kernel reference count |
| `open_handles` | `u64` | Number of open file handles |
| `state` | `ItemState` | Current sync/hydration state |

---

### New Entity: HydrationRequest

**Location**: `crates/lnxdrive-fuse/src/hydration.rs`

Represents a queued or in-progress hydration task.

| Field | Type | Description |
|-------|------|-------------|
| `ino` | `u64` | Target inode |
| `item_id` | `UniqueId` | SyncItem reference |
| `remote_id` | `RemoteId` | OneDrive item ID for download |
| `total_size` | `u64` | Expected file size |
| `downloaded` | `u64` | Bytes downloaded so far |
| `cache_path` | `PathBuf` | Destination path in cache dir |
| `priority` | `HydrationPriority` | User-initiated (high) vs auto (low) |
| `created_at` | `DateTime<Utc>` | When the request was created |

**HydrationPriority enum**:
```
HydrationPriority:
  - UserOpen    # User directly opened the file (highest)
  - PinRequest  # User pinned the file
  - Prefetch    # System prefetch (lowest)
```

---

### New Entity: DehydrationPolicy

**Location**: `crates/lnxdrive-fuse/src/dehydration.rs`

Runtime representation of the dehydration configuration.

| Field | Type | Description |
|-------|------|-------------|
| `cache_max_bytes` | `u64` | Maximum cache size |
| `threshold_percent` | `u8` | Trigger dehydration at this % of max |
| `max_age_days` | `u32` | Dehydrate files not accessed in this many days |
| `interval_minutes` | `u32` | How often to run dehydration sweep |

---

### New Entity: FuseConfig

**Location**: `crates/lnxdrive-core/src/config.rs` (added to existing Config struct)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mount_point` | `String` | `~/OneDrive` | FUSE mount point path |
| `auto_mount` | `bool` | `true` | Mount automatically when daemon starts |
| `cache_dir` | `String` | `~/.local/share/lnxdrive/cache` | Content cache directory |
| `cache_max_size_gb` | `u32` | `10` | Maximum cache size in GB |
| `dehydration_threshold_percent` | `u8` | `80` | Trigger dehydration at this % of cache_max |
| `dehydration_max_age_days` | `u32` | `30` | Dehydrate files not accessed for N days |
| `dehydration_interval_minutes` | `u32` | `60` | Dehydration sweep interval |
| `hydration_concurrency` | `u8` | `8` | Max concurrent hydration downloads |

---

## State Machine Diagram

```
                  ┌─────────┐
                  │  Online  │ (placeholder)
                  └────┬─────┘
                       │
              ┌────────┼────────┐
              │        │        │
         open/read   pin     delete
              │        │        │
              ▼        │        ▼
        ┌───────────┐  │  ┌─────────┐
        │ Hydrating │  │  │ Deleted │
        └─────┬─────┘  │  └─────────┘
              │        │
         completed     │
              │        │
              ▼        │
        ┌───────────┐  │
        │ Hydrated  │◄─┘ (pin triggers hydrate first, then pin)
        └─────┬─────┘
              │
     ┌────────┼────────┬──────────┐
     │        │        │          │
    pin     edit    dehydrate   delete
     │        │        │          │
     ▼        ▼        ▼          ▼
┌────────┐ ┌──────────┐ ┌───────┐ ┌─────────┐
│ Pinned │ │ Modified │ │Online │ │ Deleted │
└────┬───┘ └────┬─────┘ └───────┘ └─────────┘
     │          │
   unpin    sync done
     │          │
     ▼          ▼
┌──────────┐ ┌──────────┐
│ Hydrated │ │ Hydrated │
└──────────┘ └──────────┘

Error can occur from: Hydrating, Modified (sync failure)
Conflicted can occur from: Modified (conflict detected during sync)
```

---

## SQLite Schema Changes

### Modified Table: `sync_items`

**New columns**:

```sql
ALTER TABLE sync_items ADD COLUMN inode INTEGER;
ALTER TABLE sync_items ADD COLUMN last_accessed DATETIME;
ALTER TABLE sync_items ADD COLUMN hydration_progress INTEGER;

CREATE UNIQUE INDEX idx_sync_items_inode ON sync_items(inode) WHERE inode IS NOT NULL;
```

### New Table: `inode_counter`

Persists the next available inode number across restarts.

```sql
CREATE TABLE IF NOT EXISTS inode_counter (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_inode INTEGER NOT NULL DEFAULT 2
);

INSERT OR IGNORE INTO inode_counter (id, next_inode) VALUES (1, 2);
```

---

## Extended Attributes Schema

All extended attributes use the `user.lnxdrive.*` namespace.

| Attribute | Type | Description | Present When |
|-----------|------|-------------|--------------|
| `user.lnxdrive.state` | `string` | One of: `online`, `hydrating`, `hydrated`, `pinned`, `modified` | Always |
| `user.lnxdrive.size` | `string` (u64) | Real file size in bytes | Always |
| `user.lnxdrive.remote_id` | `string` | OneDrive item ID | When synced |
| `user.lnxdrive.progress` | `string` (u8) | Hydration progress 0-100 | When `state=hydrating` |

---

## Cache Directory Structure

```
~/.local/share/lnxdrive/
├── lnxdrive.db              # SQLite state database (existing)
└── cache/
    └── content/
        ├── a3/
        │   └── f5e2b1c4d...  # SHA-256(remote_id)[2:]
        ├── 7b/
        │   └── 8a91d0e3f...
        └── ...
```

Cache path for a file: `cache/content/{sha256(remote_id)[0:2]}/{sha256(remote_id)[2:]}`
