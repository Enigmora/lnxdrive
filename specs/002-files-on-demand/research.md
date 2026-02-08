# Research: Files-on-Demand (FUSE Virtual Filesystem)

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04

---

## R1: FUSE Implementation Crate

**Decision**: Use `fuser` crate (v0.16+) directly.

**Rationale**: `fuser` is a complete Rust rewrite of the FUSE protocol (no C libfuse dependency). It is the most mature and actively maintained FUSE binding in the Rust ecosystem. The design guide (`04-Componentes/01-files-on-demand-fuse.md`) explicitly selects it.

**Alternatives considered**:
- `fuser-async`: Provides an async wrapper over fuser, but adds an abstraction layer that reduces control over the sync-to-async bridge. We need fine-grained control for the hydration/dehydration lifecycle.
- `easy_fuser`: Higher-level API, but less mature and fewer features than fuser.
- Raw libfuse3 via FFI: Unnecessary complexity; fuser already provides a safe Rust API.

---

## R2: Sync-to-Async Bridge Pattern

**Decision**: Store a `tokio::runtime::Handle` in the `LnxDriveFs` struct and use `handle.block_on()` inside synchronous FUSE callbacks to invoke async code.

**Rationale**: fuser's `Filesystem` trait is synchronous (`&mut self`). Using `spawn_mount2()` runs FUSE callbacks in a dedicated background thread (not a tokio worker thread), making `Handle::block_on()` safe. This is simpler than channel-based message passing and avoids the overhead of a per-operation oneshot channel for read-heavy workloads.

**Alternatives considered**:
- Channel-based message passing (mpsc + oneshot): More decoupled but adds per-operation allocation overhead. Suitable for write operations (see R3) but overkill for reads.
- `fuser-async` crate: Internally uses the same `Handle::block_on()` pattern but adds `Arc<RwLock>` overhead.

---

## R3: SQLite Write Serialization

**Decision**: Use a single tokio task consuming `WriteOp` messages from an `mpsc` channel. FUSE threads use `blocking_send()`; async tasks use `send().await`. Results returned via `oneshot` channels.

**Rationale**: SQLite in WAL mode supports concurrent readers but only one writer. The FUSE daemon has multiple threads that may need to write (state transitions, access time updates, audit entries). A dedicated writer task eliminates `SQLITE_BUSY` errors. This matches the design guide's `WriteSerializer` pattern (`04-Componentes/01-files-on-demand-fuse.md`, risk A2).

**Alternatives considered**:
- Mutex around write operations: Simpler but blocks FUSE threads waiting for the mutex, reducing throughput.
- Separate SQLite connection per thread with retry logic: More concurrent but `SQLITE_BUSY` retries add unpredictable latency.

---

## R4: Content Storage Location

**Decision**: Store hydrated file content in a separate cache directory at `~/.local/share/lnxdrive/cache/content/`, using a hash-based directory structure (first 2 chars of SHA-256 as subdirectory).

**Rationale**: The FUSE mount point is a virtual filesystem — no real files exist in it. Storing content separately avoids FUSE intercepting its own I/O (circular calls), enables clean dehydration (delete cache file), and follows the separation of concerns principle. This is the same approach used by rclone's VFS cache.

**Alternatives considered**:
- Store in mount directory: Creates circular FUSE calls; complex dehydration.
- `~/.cache/lnxdrive/`: XDG cache spec allows cleanup by users/tools, risking data loss.

---

## R5: Hydration Strategy

**Decision**: Full download for files < 100MB (from `large_files.threshold_mb` config); HTTP Range-based chunked download for larger files. First `open()` triggers hydration; `read()` blocks until the requested byte range is available locally.

**Rationale**: Small files download quickly and full download avoids the complexity of range management. Large files benefit from streaming because users shouldn't wait for a 500MB file to fully download before the first byte is readable. Microsoft Graph supports HTTP Range requests via the `@microsoft.graph.downloadUrl`.

**Alternatives considered**:
- Always full download: Unacceptable latency for large files.
- Always range-based: Unnecessary complexity and extra API calls for small files.
- On-demand range-only (no full download): Would require sophisticated chunk tracking and cache management.

---

## R6: Inode Management

**Decision**: Monotonically increasing `u64` counter starting at 2 (inode 1 = root). Bidirectional mapping between inodes and OneDrive item IDs, persisted in SQLite. `lookup_count` and `open_file_handles` tracked per inode for reference counting.

**Rationale**: Stable inode numbers are needed across remounts. Persisting the mapping in SQLite (which already stores SyncItem data) avoids a separate persistence mechanism. The monotonic counter is simple and avoids inode reuse issues.

**Alternatives considered**:
- Hash-based inodes (hash of remote ID to u64): Risk of collisions.
- Non-persisted in-memory-only mapping: Inodes change on every mount, breaking caches and hardlinks.

---

## R7: ItemState Extension — Pinned State

**Decision**: Add `Pinned` variant to the existing `ItemState` enum in `lnxdrive-core`. Update state transition rules to allow `Hydrated <-> Pinned` and `Online -> Pinned` (triggers hydration).

**Rationale**: The design guide defines `Pinned` as a distinct file state with special behavior (immune to dehydration). The current `ItemState` enum has `Online`, `Hydrating`, `Hydrated`, `Modified`, `Conflicted`, `Error`, `Deleted` but lacks `Pinned`.

**Alternatives considered**:
- Separate `is_pinned: bool` flag on `SyncItem`: Simpler but breaks the state machine model and requires checking two fields instead of one.
- Pinned as metadata (not state): Would need special-case logic everywhere dehydration decisions are made.

---

## R8: Dehydration Policy

**Decision**: LRU-based eviction with configurable threshold (percentage of configured cache limit) and maximum age (days since last access). Periodic background task (interval from config). Never dehydrate pinned files, files with open handles, or files with pending modifications.

**Rationale**: LRU eviction is the industry standard for cache management and matches the design guide's recommendation. Tracking open handles via atomic counters in FUSE `open()`/`release()` prevents I/O errors during dehydration (risk C2).

**Alternatives considered**:
- LFU (least frequently used): More complex to implement, marginal benefit for this use case.
- Size-weighted LRU: Evict large files first — adds complexity without clear benefit.

---

## R9: FUSE Configuration Schema

**Decision**: Add a `fuse` section to the existing YAML config with: `mount_point` (default: `~/OneDrive`), `auto_mount` (default: true), `cache_dir` (default: `~/.local/share/lnxdrive/cache`), `cache_max_size_gb` (default: 10), `dehydration_threshold_percent` (default: 80), `dehydration_max_age_days` (default: 30), `hydration_concurrency` (default: 8), `dehydration_interval_minutes` (default: 60).

**Rationale**: These configuration knobs cover all the tunable parameters identified in the spec (FR-043, FR-044). Defaults are sensible for typical home users with 10GB+ free space.

**Alternatives considered**:
- Separate FUSE config file: Unnecessary fragmentation; the existing config.yaml is well-structured.
- No configuration (hardcoded defaults): Insufficient for power users and different disk space situations.
