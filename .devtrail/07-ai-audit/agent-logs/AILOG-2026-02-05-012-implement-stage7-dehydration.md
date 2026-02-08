---
id: AILOG-2026-02-05-012
title: Implement Stage 7 - Automatic Dehydration to Reclaim Disk Space
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, dehydration, space-reclaim, stage-7, us5]
related: [T079, T080, T081, T082, T083, T084, T085, T087]
---

# AILOG: Implement Stage 7 - Automatic Dehydration to Reclaim Disk Space

## Summary

Implemented Stage 7 of the Files-on-Demand feature (User Story 5: Automatic Dehydration to Reclaim Disk Space). This includes DehydrationPolicy, DehydrationManager with run_sweep() and start_periodic(), manual dehydration methods, CLI commands for hydrate/dehydrate, and comprehensive unit tests.

## Context

Stage 7 enables automatic disk space reclamation by dehydrating least-recently-accessed files when the cache exceeds a configurable threshold. This is essential for managing disk space on devices with limited storage while maintaining the Files-on-Demand experience.

## Actions Performed

### Dehydration Policy and Manager (T079-T083)

1. **T079: DehydrationPolicy struct**:
   - Fields: `cache_max_bytes`, `threshold_percent`, `max_age_days`, `interval_minutes`
   - Constructor `from_config(config: &FuseConfig)` converts GB to bytes
   - Method `threshold_bytes()` calculates trigger threshold
   - Implements Default for testing

2. **T080: DehydrationManager struct**:
   - Holds references to: `policy`, `cache`, `inode_table`, `write_handle`, `db_pool`
   - Includes `shutdown` flag (Arc<RwLock<bool>>) for graceful termination
   - Constructor `new()` initializes all components

3. **T081: run_sweep()**:
   - Checks cache.disk_usage() against threshold
   - Queries DB for candidates via `get_items_for_dehydration()`
   - For each candidate:
     - Checks open_handles (skips if open)
     - Verifies state is Hydrated
     - Removes cached content
     - Updates state to Online via WriteSerializer
   - Returns detailed DehydrationReport

4. **T082: start_periodic()**:
   - Spawns tokio task that runs sweep at configured interval
   - Skips first immediate tick to avoid immediate sweep on mount
   - Checks shutdown flag each iteration
   - Returns JoinHandle for cancellation

5. **T083: Manual dehydration methods**:
   - `dehydrate_path(ino)` - Dehydrates single file, returns freed bytes
   - `dehydrate_paths(inos)` - Batch dehydration with comprehensive report
   - Validates eligibility (Hydrated state, no open handles)

### CLI Commands (T084-T085)

1. **T084: HydrateCommand and DehydrateCommand** in `commands/hydrate.rs`:
   - Accepts multiple paths
   - Validates paths exist
   - DehydrateCommand includes `--force` flag
   - Outputs results in human or JSON format
   - Includes `format_bytes()` helper for readable output
   - Note: Full FUSE IPC integration pending

2. **T085: Command registration**:
   - Added `pub mod hydrate;` to commands/mod.rs
   - Added `Hydrate(HydrateCommand)` and `Dehydrate(DehydrateCommand)` to Commands enum
   - Added match arms in main.rs

### Unit Tests (T087)

13 tests in dehydration.rs:
- DehydrationPolicy tests: from_config, threshold_bytes, default, edge_cases, clone, debug
- DehydrationReport tests: default, merge, debug
- DehydrationManager tests: policy_accessor, item_state_can_dehydrate, report_skipped_reasons

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-fuse/src/dehydration.rs` | Complete implementation: DehydrationPolicy, DehydrationManager, run_sweep(), start_periodic(), dehydrate_path(), dehydrate_paths(), 13 unit tests |
| `crates/lnxdrive-fuse/src/lib.rs` | Added exports for DehydrationManager, DehydrationPolicy, DehydrationReport |
| `crates/lnxdrive-cli/src/commands/hydrate.rs` | NEW: HydrateCommand, DehydrateCommand, format_bytes() |
| `crates/lnxdrive-cli/src/commands/mod.rs` | Added `pub mod hydrate;` |
| `crates/lnxdrive-cli/src/main.rs` | Added Hydrate/Dehydrate variants and match arms |
| `specs/002-files-on-demand/tasks.md` | Marked T079-T085, T087 as complete |

## Decisions Made

- **Database is source of truth for state**: InodeTable entry state updates are not done during dehydration since InodeEntry doesn't have `set_state()` and is stored in Arc. The database is the authoritative source and the inode table will be refreshed on next access.
- **DehydrationReport struct**: Created comprehensive report type to track counts, bytes freed, skipped items, and errors for both automated and manual dehydration.
- **Shutdown via RwLock**: Used `Arc<RwLock<bool>>` shutdown flag for clean periodic task termination.
- **T086 deferred**: Integration into mount lifecycle requires more invasive changes to LnxDriveFs architecture.

## Impact

- **Functionality**: System can automatically reclaim disk space by dehydrating old, unused files
- **Performance**: Background sweeps run at configurable intervals, batch processing in chunks of 100
- **Security**: N/A - local state management only

## Verification

- [x] Code compiles without errors
- [x] 13 dehydration tests pass
- [x] Clippy passes with -D warnings

## Additional Notes

Stage 7 progress:
- T079-T085: Complete (DehydrationPolicy, DehydrationManager, CLI commands)
- T086: Pending (integration into mount lifecycle requires LnxDriveFs changes)
- T087: Complete (unit tests)

Total tasks implemented: 7 of 9 (T079-T085, T087)
Remaining: T086 (mount lifecycle integration)

---

<!-- Template: DevTrail | https://enigmora.com -->
