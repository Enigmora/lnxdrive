---
id: AILOG-2026-02-05-005
title: Implement LnxDriveFs struct with init/destroy lifecycle
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, filesystem, stage-3, us1]
related: [T029, T030, T031]
---

# AILOG: Implement LnxDriveFs struct with init/destroy lifecycle

## Summary

Implemented the core `LnxDriveFs` struct that implements the `fuser::Filesystem` trait, including the `init()` and `destroy()` lifecycle methods. This is the foundation for the FUSE virtual filesystem in the Files-on-Demand feature.

## Context

Stage 3 of spec 002-files-on-demand requires implementing User Story 1 (Browse OneDrive Files Without Downloading). The `LnxDriveFs` struct is the central component that bridges the FUSE kernel interface with the LNXDrive state repository and cache.

## Actions Performed

1. Implemented `LnxDriveFs` struct with fields:
   - `rt_handle: Handle` - Tokio runtime for async operations
   - `inode_table: Arc<InodeTable>` - Bidirectional inode mapping
   - `write_handle: WriteSerializerHandle` - Serialized DB writes
   - `cache: Arc<ContentCache>` - Content cache for hydrated files
   - `config: FuseConfig` - FUSE configuration
   - `db_pool: DatabasePool` - Database connection pool
   - `next_fh: AtomicU64` - Atomic file handle counter

2. Implemented `new()` constructor that:
   - Creates WriteSerializer and spawns it as tokio task
   - Initializes empty InodeTable
   - Returns configured LnxDriveFs instance

3. Implemented `fuser::Filesystem::init()` that:
   - Negotiates kernel capabilities (FUSE_CAP_EXPORT_SUPPORT)
   - Loads all SyncItems from SQLite via `rt_handle.block_on()`
   - Creates root inode (ino=1) for mount point
   - Assigns inodes to items using `get_next_inode()`
   - Populates InodeTable with InodeEntry for each item
   - Logs count of loaded items

4. Implemented `fuser::Filesystem::destroy()` that:
   - Logs shutdown with item count
   - WriteSerializer handle is dropped automatically

5. Added helper function `sync_item_to_inode_entry()` for converting domain objects

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-fuse/src/filesystem.rs` | Implemented LnxDriveFs struct, new(), init(), destroy() |
| `crates/lnxdrive-cache/src/pool.rs` | Added `#[derive(Clone)]` to DatabasePool |

## Decisions Made

- Used `rt_handle.block_on()` for async-to-sync bridge in FUSE callbacks (per research decision R2)
- Root inode is always 1 per FUSE convention
- Items without existing inodes get new ones via WriteSerializer's `increment_inode_counter()`
- Two-pass algorithm for parent inode resolution to handle any ordering in DB results

## Impact

- **Functionality**: Enables FUSE filesystem initialization with state loaded from SQLite
- **Performance**: O(n) initialization where n = number of items in state repository
- **Security**: N/A - no security-sensitive changes

## Verification

- [x] Code compiles without errors
- [x] Tests pass (5 unit tests for LnxDriveFs)
- [x] Manual review performed

## Additional Notes

The `init()` method uses a two-pass approach:
1. First pass: Create all InodeEntries with parent_ino=ROOT
2. Second pass: Update parent_ino based on path hierarchy

This handles cases where children might be loaded before their parents from the database.

---

<!-- Template: DevTrail | https://enigmora.com -->
