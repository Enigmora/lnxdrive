---
id: AILOG-2026-02-05-009
title: Implement Stage 4 - Automatic File Hydration (On-Demand Download)
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, hydration, download, stage-4, us2]
related: [T047, T048, T049, T050, T051, T052, T053, T054, T055, T056, T057, T058, T059, T060, T061, T062, T096]
---

# AILOG: Implement Stage 4 - Automatic File Hydration

## Summary

Implemented Stage 4 of the Files-on-Demand feature (User Story 2: Open a Cloud File and Have It Download Automatically). This includes the HydrationManager for concurrent download management, FUSE file operations (open/read/release/flush), Graph API download methods, and crash recovery for interrupted hydrations.

## Context

Stage 4 enables the core Files-on-Demand functionality: when a user opens a placeholder file (state=Online), the system automatically downloads the content from OneDrive and delivers it to the reading application. The file state transitions Online→Hydrating→Hydrated.

## Actions Performed

### HydrationManager (T047-T054)

1. **T047: HydrationPriority enum** - Three priority levels:
   - `Prefetch` (0) - Lowest, anticipated access
   - `PinRequest` (1) - Medium, user pinned file
   - `UserOpen` (2) - Highest, active file open

2. **T048: HydrationRequest struct** - Tracks in-progress downloads:
   - Atomic progress tracking via `AtomicU64`
   - Watch channel for progress notifications (0-100%)
   - Methods: `progress()`, `add_downloaded()`, `mark_complete()`, `subscribe()`

3. **T049: HydrationManager struct** - Manages concurrent hydrations:
   - `DashMap<u64, ActiveHydration>` for deduplication by inode
   - `Semaphore` for concurrency limiting (from config)
   - `CancellationToken` for cooperative task cancellation

4. **T050: hydrate()** - Main download method:
   - Deduplication: returns existing receiver if already hydrating
   - Spawns download task with semaphore permit
   - Full download (<100MB) or chunked via HTTP Range (≥100MB)
   - Progress updates via watch channel and DB
   - State transitions: Online→Hydrating→Hydrated (or Error)

5. **T051-T054: Helper methods**:
   - `wait_for_completion()` - Blocks until 100%
   - `wait_for_range()` - Blocks until byte range available
   - `cancel()` - Cancels download, cleans up partial file
   - `is_hydrating()`, `progress()` - Query active state

### FUSE File Operations (T055-T058)

1. **T055: open()** - Opens file, triggers hydration if needed
2. **T056: read()** - Reads from cache, returns EIO if not hydrated
3. **T057: release()** - Decrements open_handles, marks dehydration eligible
4. **T058: flush()** - No-op per FUSE contract

### Graph API Integration (T059-T060)

1. **T059: get_download_url()** - Gets pre-authenticated download URL
2. **T060: download_file_to_disk()** - Streaming download to disk
3. **T060: download_range()** - Partial download via HTTP Range header

### Crash Recovery (T096)

- During init(), scans for stale `Hydrating` items from crash
- Cleans up partial files
- Resets state to `Online` using new `reset_state_for_crash_recovery()` method

### Unit Tests (T061-T062)

- 15 tests for HydrationManager (dedup, concurrency, cancel, progress)
- 19 tests for FUSE file operations (open, read, release)

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-fuse/src/hydration.rs` | HydrationPriority, HydrationRequest, HydrationManager |
| `crates/lnxdrive-fuse/src/filesystem.rs` | open(), read(), release(), flush(), crash recovery |
| `crates/lnxdrive-graph/src/provider.rs` | get_download_url(), download_file_to_disk(), download_range() |
| `crates/lnxdrive-graph/src/client.rs` | Added base_url() and client() accessors |
| `crates/lnxdrive-core/src/domain/sync_item.rs` | reset_state_for_crash_recovery() method |
| `crates/lnxdrive-fuse/Cargo.toml` | Added tokio-util dependency |
| `crates/lnxdrive-graph/Cargo.toml` | Added futures-util dependency |

## Decisions Made

- 100MB threshold for full vs chunked downloads (research decision R5)
- 10MB chunk size for HTTP Range requests
- CancellationToken for cooperative task cancellation
- Progress updates throttled to every 5% change
- Crash recovery bypasses state machine validation (special method)

## Impact

- **Functionality**: Files download automatically when opened
- **Performance**: Concurrent downloads limited by config, streaming prevents memory issues
- **Security**: N/A - downloads use pre-authenticated URLs from Graph API

## Verification

- [x] Code compiles without errors
- [x] 112 tests pass in lnxdrive-fuse
- [x] Clippy passes with -D warnings

## Additional Notes

Stage 4 checkpoint achieved:
- Opening a placeholder triggers download
- `cat` delivers content
- State transitions Online→Hydrating→Hydrated
- Crash recovery handles interrupted hydrations

Total tasks implemented: 16 (T047-T062, T096)

---

<!-- Template: DevTrail | https://enigmora.com -->
