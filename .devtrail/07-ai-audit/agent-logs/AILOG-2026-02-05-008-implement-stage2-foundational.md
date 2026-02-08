---
id: AILOG-2026-02-05-008
title: Implement Stage 2 foundational infrastructure for Files-on-Demand
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, domain, config, repository, stage-2]
related: [T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T019, T020, T021, T022, T023, T024, T025, T106]
---

# AILOG: Implement Stage 2 foundational infrastructure for Files-on-Demand

## Summary

Implemented the foundational infrastructure for the Files-on-Demand feature (Stage 2), including domain model changes, configuration, repository extensions, error types, write serialization, and content caching. This establishes the core building blocks for the FUSE filesystem.

## Context

Stage 2 of spec 002-files-on-demand requires implementing foundational components that all user stories depend on. This includes extending the domain model with a `Pinned` state, adding FUSE configuration, extending the repository with inode-related methods, and implementing core infrastructure components.

## Actions Performed

### Domain Model Changes (T007-T012)

1. Added `Pinned` variant to `ItemState` enum
2. Added helper methods: `is_pinned()`, `can_dehydrate()`
3. Updated `is_local()` to return true for Pinned state
4. Added state transitions: Hydrated↔Pinned, Pinned→Modified, Modified→Pinned, Pinned→Deleted
5. Added fields to SyncItem: `inode`, `last_accessed`, `hydration_progress`
6. Added convenience methods: `pin()`, `unpin()`
7. Added unit tests for all new ItemState functionality

### Configuration Changes (T013-T016)

1. Created `FuseConfig` struct with 8 fields:
   - mount_point, auto_mount, cache_dir, cache_max_size_gb
   - dehydration_threshold_percent, dehydration_max_age_days
   - dehydration_interval_minutes, hydration_concurrency
2. Added `fuse` field to `Config` struct
3. Added validation rules (threshold 1-100, concurrency 1-32, etc.)
4. Updated `config/default-config.yaml` with fuse section
5. Added unit tests for FuseConfig

### Repository Extensions (T019-T020)

1. Extended `SqliteStateRepository` with methods:
   - `get_next_inode()` - Atomically increment inode counter
   - `update_inode()` - Update item's inode
   - `get_item_by_inode()` - Lookup item by inode
   - `update_last_accessed()` - Update access timestamp
   - `update_hydration_progress()` - Track hydration progress
   - `get_items_for_dehydration()` - Query candidates for dehydration
2. Added unit tests for new repository methods

### Error Types (T021)

1. Implemented `FuseError` enum with 15 variants using thiserror
2. Implemented `From<FuseError> for libc::c_int` for POSIX errno mapping
3. Maps variants to: ENOENT, EACCES, EEXIST, ENOTEMPTY, EIO, ENOTDIR, EISDIR, ENOSPC, ENODATA, ERANGE, EINVAL, ENAMETOOLONG

### Write Serializer (T022-T023)

1. Implemented `WriteOp` enum with operation variants
2. Implemented `WriteSerializer` struct with mpsc channel pattern
3. Implemented `WriteSerializerHandle` for async/blocking send
4. Single writer task prevents SQLITE_BUSY errors
5. Added unit tests for serialization behavior

### Content Cache (T024-T025)

1. Implemented `ContentCache` struct with SHA-256 hash-based storage
2. Methods: `cache_path()`, `partial_path()`, `store()`, `read()`, `exists()`, `remove()`, `disk_usage()`
3. Directory structure: `{cache_dir}/content/{hash_prefix}/{hash_rest}`
4. Added comprehensive unit tests

### InodeNumber Newtype (T106)

1. Created `InodeNumber(u64)` newtype for type safety
2. Implemented Display, Debug, Clone, Copy, Hash, Ord, etc.
3. Defined `ROOT` constant (inode 1)
4. Satisfies Constitution Principle II (Idiomatic Rust)

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-core/src/domain/sync_item.rs` | Pinned state, new fields, helper methods, tests |
| `crates/lnxdrive-core/src/config.rs` | FuseConfig struct, validation, builder methods |
| `config/default-config.yaml` | Added fuse section |
| `crates/lnxdrive-cache/src/repository.rs` | 6 new FUSE-related methods |
| `crates/lnxdrive-fuse/src/error.rs` | FuseError enum with errno mapping |
| `crates/lnxdrive-fuse/src/write_serializer.rs` | WriteOp, WriteSerializer, WriteSerializerHandle |
| `crates/lnxdrive-fuse/src/cache.rs` | ContentCache implementation |
| `crates/lnxdrive-fuse/src/inode_entry.rs` | InodeNumber newtype, InodeEntry struct |
| `crates/lnxdrive-fuse/src/inode.rs` | InodeTable with DashMap |

## Decisions Made

- `Pinned` is a separate ItemState variant (not a bool flag) per research decision R7
- WriteSerializer uses mpsc channel pattern per research decision R3
- ContentCache uses SHA-256 hashing per research decision R4
- InodeNumber newtype prevents mixing u64 identifiers (Constitution Principle II)

## Impact

- **Functionality**: Complete foundation for FUSE filesystem operations
- **Performance**: Lock-free InodeTable, serialized writes prevent contention
- **Security**: N/A - no security-sensitive changes

## Verification

- [x] Code compiles without errors
- [x] All tests pass
- [x] Clippy passes with -D warnings

## Additional Notes

Stage 2 checkpoint achieved: "Domain model updated, config extended, database migrated, all foundational infrastructure ready."

---

<!-- Template: DevTrail | https://enigmora.com -->
