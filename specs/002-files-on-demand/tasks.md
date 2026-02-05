# Tasks: Files-on-Demand (FUSE Virtual Filesystem)

**Input**: Design documents from `/specs/002-files-on-demand/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/cli-commands.md, contracts/fuse-operations.md, quickstart.md
**Branch**: `feat/002-files-on-demand`

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks in the same stage)
- **[Story]**: Which user story this task belongs to (US1–US6)
- Include exact file paths in descriptions
- "Stages" are internal to this spec. "Fases" are project-wide roadmap phases.

## Path Conventions

- **Workspace crate**: `crates/lnxdrive-fuse/src/` (main implementation)
- **Domain changes**: `crates/lnxdrive-core/src/` (ItemState, Config)
- **CLI commands**: `crates/lnxdrive-cli/src/commands/`
- **Database**: `crates/lnxdrive-cache/src/`
- **Config**: `config/default-config.yaml`
- **Tests**: `crates/lnxdrive-fuse/tests/`

---

## Stage 1: Setup (Project Initialization)

**Purpose**: Prepare the workspace, add dependencies, and create the skeleton structure for the FUSE crate.

- [ ] T001 [P] Add new workspace dependencies (`dashmap`, `sha2`, `libc`) to `Cargo.toml` root `[workspace.dependencies]` section: `dashmap = "6.0"`, `sha2 = "0.10"`, `libc = "0.2"`
- [ ] T002 [P] Update `crates/lnxdrive-fuse/Cargo.toml`: add dependencies on `lnxdrive-core`, `lnxdrive-cache`, `lnxdrive-graph`, `dashmap`, `sha2`, `chrono`, `anyhow`, `libc`, `serde`, `serde_json`, `reqwest`, `uuid`, `tokio` (ensure features include `sync`, `rt`), and `sqlx`
- [ ] T003 [P] Add `lnxdrive-fuse` as dependency to `crates/lnxdrive-cli/Cargo.toml` with `lnxdrive-fuse.workspace = true`
- [ ] T004 [P] Create empty module files for the FUSE crate structure: `crates/lnxdrive-fuse/src/error.rs`, `crates/lnxdrive-fuse/src/filesystem.rs`, `crates/lnxdrive-fuse/src/inode.rs`, `crates/lnxdrive-fuse/src/inode_entry.rs`, `crates/lnxdrive-fuse/src/hydration.rs`, `crates/lnxdrive-fuse/src/dehydration.rs`, `crates/lnxdrive-fuse/src/cache.rs`, `crates/lnxdrive-fuse/src/write_serializer.rs`, `crates/lnxdrive-fuse/src/xattr.rs` — each with a module-level doc comment describing its purpose
- [ ] T005 Update `crates/lnxdrive-fuse/src/lib.rs`: add `pub mod` declarations for all 9 modules (error, filesystem, inode, inode_entry, hydration, dehydration, cache, write_serializer, xattr), add public re-exports for `FuseError`, `LnxDriveFs`, `ContentCache`, `HydrationManager`, `DehydrationManager`
- [ ] T006 Verify workspace compiles: run `cargo check --workspace --exclude lnxdrive-conflict --exclude lnxdrive-audit --exclude lnxdrive-telemetry`

**Checkpoint**: All module files exist, workspace compiles with empty modules.

---

## Stage 2: Foundational (Blocking Prerequisites)

**Purpose**: Core domain changes and infrastructure that ALL user stories depend on. Nothing in Stages 3-8 can start until Stage 2 is complete.

### Domain Model Changes

- [ ] T007 [P] Add `Pinned` variant to `ItemState` enum in `crates/lnxdrive-core/src/domain/sync_item.rs`: add `Pinned` after `Hydrated`, update `#[serde(rename_all = "snake_case")]` compatibility, update `Display` impl to include `"Pinned"`, update `name()` to return `"pinned"`
- [ ] T008 [P] Update `ItemState` helper methods in `crates/lnxdrive-core/src/domain/sync_item.rs`: modify `is_local()` to return true for `Pinned` (alongside `Hydrated` and `Modified`), add new method `is_pinned(&self) -> bool` returning true for `Pinned`, add new method `can_dehydrate(&self) -> bool` returning true only for `Hydrated`
- [ ] T009 Update state transition rules in `can_transition_to()` in `crates/lnxdrive-core/src/domain/sync_item.rs`: add transitions `Hydrated → Pinned`, `Pinned → Hydrated`, `Online → Hydrating` (when pin triggers hydration, existing), `Pinned → Modified`, `Modified → Pinned` (sync complete on pinned file), `Pinned → Deleted`
- [ ] T010 [P] Add new fields to `SyncItem` struct in `crates/lnxdrive-core/src/domain/sync_item.rs`: add `inode: Option<u64>`, `last_accessed: Option<DateTime<Utc>>`, `hydration_progress: Option<u8>` — with getters `inode()`, `last_accessed()`, `hydration_progress()` and setters `set_inode()`, `set_last_accessed()`, `set_hydration_progress()`
- [ ] T011 Add convenience state transition methods to `SyncItem` in `crates/lnxdrive-core/src/domain/sync_item.rs`: add `pin()` (Hydrated→Pinned), `unpin()` (Pinned→Hydrated), update existing `start_hydrating()` to also set `hydration_progress` to `Some(0)`, update `complete_hydration()` to set `hydration_progress` to `None`
- [ ] T012 [P] Update existing unit tests for `ItemState` in `crates/lnxdrive-core/src/domain/sync_item.rs`: add tests for `is_pinned()`, `can_dehydrate()`, `is_local()` with Pinned, new transitions (Hydrated↔Pinned, Pinned→Modified, Modified→Pinned, Pinned→Deleted), verify illegal transitions are rejected (e.g., Online→Pinned directly without Hydrating)

### Configuration Changes

- [ ] T013 [P] Create `FuseConfig` struct in `crates/lnxdrive-core/src/config.rs`: add struct with fields `mount_point: String` (default `~/OneDrive`), `auto_mount: bool` (default `true`), `cache_dir: String` (default `~/.local/share/lnxdrive/cache`), `cache_max_size_gb: u32` (default `10`), `dehydration_threshold_percent: u8` (default `80`), `dehydration_max_age_days: u32` (default `30`), `dehydration_interval_minutes: u32` (default `60`), `hydration_concurrency: u8` (default `8`). Derive `Debug, Clone, Serialize, Deserialize`. Implement `Default`.
- [ ] T014 Add `fuse: FuseConfig` field to `Config` struct in `crates/lnxdrive-core/src/config.rs`, update `Config::Default` to include `FuseConfig::default()`, add validation rules in `validate()`: `cache_max_size_gb > 0`, `dehydration_threshold_percent` in 1..=100, `hydration_concurrency` in 1..=32, `dehydration_interval_minutes > 0`
- [ ] T015 [P] Update `config/default-config.yaml`: add `fuse` section with all 8 fields and their defaults matching `FuseConfig::default()`
- [ ] T016 [P] Add unit tests for `FuseConfig` in `crates/lnxdrive-core/src/config.rs`: test defaults, test deserialization from YAML, test validation (invalid threshold, zero concurrency, zero max_size), test that config with fuse section loads correctly

### Database Schema Changes

- [ ] T017 [P] Create SQL migration file `crates/lnxdrive-cache/src/migrations/003_fuse_support.sql` (or next sequential number): `ALTER TABLE sync_items ADD COLUMN inode INTEGER`, `ALTER TABLE sync_items ADD COLUMN last_accessed DATETIME`, `ALTER TABLE sync_items ADD COLUMN hydration_progress INTEGER`, `CREATE UNIQUE INDEX idx_sync_items_inode ON sync_items(inode) WHERE inode IS NOT NULL`, `CREATE TABLE IF NOT EXISTS inode_counter (id INTEGER PRIMARY KEY CHECK (id = 1), next_inode INTEGER NOT NULL DEFAULT 2)`, `INSERT OR IGNORE INTO inode_counter (id, next_inode) VALUES (1, 2)`
- [ ] T018 Register the new migration in `crates/lnxdrive-cache/src/pool.rs` (or wherever `sqlx::migrate!()` is configured): ensure the migration runs on database initialization
- [ ] T019 [P] Extend `SqliteStateRepository` in `crates/lnxdrive-cache/src/repository.rs`: add methods `get_next_inode() -> Result<u64>` (atomically increment inode_counter and return the value), `update_inode(item_id: &UniqueId, inode: u64) -> Result<()>`, `get_item_by_inode(inode: u64) -> Result<Option<SyncItem>>`, `update_last_accessed(item_id: &UniqueId, accessed: DateTime<Utc>) -> Result<()>`, `update_hydration_progress(item_id: &UniqueId, progress: Option<u8>) -> Result<()>`, `get_items_for_dehydration(max_age_days: u32, limit: u32) -> Result<Vec<SyncItem>>` (returns hydrated items sorted by last_accessed ASC, excluding pinned/modified/deleted)
- [ ] T020 [P] Add unit tests for new repository methods in `crates/lnxdrive-cache/src/repository.rs` (or in a `tests/` module): test `get_next_inode` returns sequential values (2, 3, 4...), test `get_item_by_inode` returns correct item, test `get_items_for_dehydration` excludes pinned/modified/open items, test `update_hydration_progress` stores and retrieves values

### Error Types

- [ ] T021 Implement `FuseError` enum in `crates/lnxdrive-fuse/src/error.rs` using `thiserror`: variants `NotFound(String)`, `PermissionDenied(String)`, `AlreadyExists(String)`, `NotEmpty(String)`, `IoError(String)`, `NotADirectory(String)`, `IsADirectory(String)`, `DiskFull(String)`, `XattrNotFound(String)`, `XattrBufferTooSmall`, `InvalidArgument(String)`, `NameTooLong(String)`, `HydrationFailed(String)`, `CacheError(String)`, `DatabaseError(String)`. Add `impl From<FuseError> for libc::c_int` that maps each variant to the appropriate errno (ENOENT, EACCES, EEXIST, ENOTEMPTY, EIO, ENOTDIR, EISDIR, ENOSPC, ENODATA, ERANGE, EINVAL, ENAMETOOLONG, EIO, EIO, EIO). Add `impl From<CacheError> for FuseError` and `impl From<anyhow::Error> for FuseError`.

### Write Serializer

- [ ] T022 Implement `WriteOp` enum and `WriteSerializer` struct in `crates/lnxdrive-fuse/src/write_serializer.rs`: define `WriteOp` enum with variants for each write operation (UpdateState, UpdateInode, UpdateLastAccessed, UpdateHydrationProgress, CreateItem, DeleteItem, RenameItem, IncrementInodeCounter) each carrying the necessary data plus a `tokio::sync::oneshot::Sender<Result<WriteResult>>` for the response. Define `WriteResult` enum to return results. Implement `WriteSerializer` with `new(pool: DatabasePool) -> (Self, WriteSerializerHandle)`, where `Self` runs as a tokio task consuming from `mpsc::Receiver<WriteOp>` and `WriteSerializerHandle` exposes `async fn send(&self, op: WriteOp) -> Result<WriteResult>` and `fn blocking_send(&self, op: WriteOp) -> Result<WriteResult>`. The task should process operations sequentially using `SqliteStateRepository`.
- [ ] T023 [P] Add unit tests for `WriteSerializer` in `crates/lnxdrive-fuse/src/write_serializer.rs`: test that sequential writes are processed in order, test that concurrent sends from multiple tasks are serialized, test that oneshot response is received correctly, test error propagation

### Content Cache

- [ ] T024 Implement `ContentCache` struct in `crates/lnxdrive-fuse/src/cache.rs`: constructor `new(cache_dir: PathBuf)` creates `cache_dir/content/` if not exists. Methods: `cache_path(remote_id: &RemoteId) -> PathBuf` (compute SHA-256 of remote_id, return `content/{first_2_chars}/{rest}`), `store(remote_id: &RemoteId, data: &[u8]) -> Result<PathBuf>` (write data to cache path, create parent dirs), `read(remote_id: &RemoteId, offset: u64, size: u32) -> Result<Vec<u8>>` (read bytes from cached file at offset), `exists(remote_id: &RemoteId) -> bool`, `remove(remote_id: &RemoteId) -> Result<()>` (delete cached file), `disk_usage() -> Result<u64>` (sum of all files in content/), `partial_path(remote_id: &RemoteId) -> PathBuf` (same as cache_path but with `.partial` extension, for in-progress downloads)
- [ ] T025 [P] Add unit tests for `ContentCache` in `crates/lnxdrive-fuse/src/cache.rs`: test `cache_path` produces correct SHA-256 hash layout, test `store` and `read` round-trip, test `exists` returns correct bool, test `remove` deletes file, test `disk_usage` computes correctly, test `partial_path` has `.partial` suffix

### InodeNumber Newtype

- [ ] T106 [P] Create `InodeNumber` newtype in `crates/lnxdrive-fuse/src/inode_entry.rs`: define `pub struct InodeNumber(u64)` with `impl InodeNumber { pub fn new(val: u64) -> Self`, `pub fn get(&self) -> u64` }. Implement `From<u64>`, `Into<u64>`, `Display`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`. Define constants: `ROOT_INO = InodeNumber(1)`. This satisfies Constitution Principle II (Idiomatic Rust — newtypes for type safety) and prevents accidental mixing of raw inode numbers with other u64 identifiers.

### Inode Entry & Table

- [ ] T026 Implement `InodeEntry` struct in `crates/lnxdrive-fuse/src/inode_entry.rs`: all fields per data-model.md (ino, item_id, remote_id, parent_ino, name, kind, size, perm, mtime, ctime, atime, nlink, lookup_count, open_handles, state). Constructor `new(...)`. Method `to_file_attr(&self) -> fuser::FileAttr` converting fields to FUSE format. Method `increment_lookup()`, `decrement_lookup()`, `increment_open_handles()`, `decrement_open_handles()`, `is_expired(lookup_count == 0 && open_handles == 0)`. Getters for all fields.
- [ ] T027 Implement `InodeTable` struct in `crates/lnxdrive-fuse/src/inode.rs`: backed by `DashMap<u64, InodeEntry>` for inode→entry and `DashMap<UniqueId, u64>` for item_id→inode. Constructor `new()`. Methods: `insert(entry: InodeEntry)`, `get(ino: u64) -> Option<Ref<InodeEntry>>`, `get_mut(ino: u64) -> Option<RefMut<InodeEntry>>`, `get_by_item_id(id: &UniqueId) -> Option<u64>`, `remove(ino: u64) -> Option<InodeEntry>`, `children(parent_ino: u64) -> Vec<InodeEntry>` (iterate and filter by parent_ino), `lookup(parent_ino: u64, name: &str) -> Option<InodeEntry>` (iterate children matching name), `len() -> usize`. Use `DashMap` for lock-free concurrent access from multiple FUSE threads.
- [ ] T028 [P] Add unit tests for `InodeTable` in `crates/lnxdrive-fuse/src/inode.rs`: test insert/get/remove, test get_by_item_id reverse lookup, test children returns correct entries, test lookup by parent+name, test concurrent access from multiple threads

**Checkpoint**: Domain model updated, config extended, database migrated, all foundational infrastructure ready. No user story can start until this checkpoint passes with `cargo check` and `cargo test`.

---

## Stage 3: User Story 1 — Browse OneDrive Files Without Downloading (Priority: P1) MVP

**Goal**: Mount the FUSE filesystem, list directories, and show file metadata — all from the local state repository without network requests.

**Independent Test**: Mount the FUSE filesystem, `ls -la` a directory, confirm entries appear with correct metadata and zero network downloads.

**Depends on**: Stage 2 complete

### Core FUSE Filesystem Struct

- [ ] T029 [US1] Implement `LnxDriveFs` struct in `crates/lnxdrive-fuse/src/filesystem.rs`: fields for `rt_handle: tokio::runtime::Handle`, `inode_table: Arc<InodeTable>`, `write_handle: WriteSerializerHandle`, `cache: Arc<ContentCache>`, `config: FuseConfig`, `db_pool: DatabasePool`. Constructor `new(rt_handle, db_pool, config, cache) -> Self` that initializes all components and spawns the WriteSerializer task.
- [ ] T030 [US1] Implement `fuser::Filesystem::init()` in `crates/lnxdrive-fuse/src/filesystem.rs`: negotiate kernel capabilities (set `FUSE_CAP_EXPORT_SUPPORT` if available), load all SyncItems from the state repository via `rt_handle.block_on()`, create root inode (ino=1) for the mount point, assign inodes to all items (using `get_next_inode()` for items without an inode), populate the `InodeTable` with `InodeEntry` for each item. Log the count of loaded items.
- [ ] T031 [US1] Implement `fuser::Filesystem::destroy()` in `crates/lnxdrive-fuse/src/filesystem.rs`: log shutdown, drop the WriteSerializer handle to signal the writer task to stop.

### Metadata Operations

- [ ] T032 [US1] Implement `fuser::Filesystem::lookup()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent: u64` and `name: &OsStr`, search `inode_table.lookup(parent, name)`. If found: increment `lookup_count`, return `ReplyEntry` with TTL (1 second), `FileAttr` from `InodeEntry::to_file_attr()`, and generation=0. If not found: reply with `ENOENT`.
- [ ] T033 [US1] Implement `fuser::Filesystem::getattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino: u64`, look up in `inode_table.get(ino)`. Return real file size (from the `size` field on `InodeEntry`, which holds the remote size even for placeholders). Reply with `ReplyAttr`, TTL 1 second. If not found: `ENOENT`. Target: <1ms. **U2 Note (stale entries)**: If a file was deleted from OneDrive since the last sync, FUSE will still return cached metadata until the sync engine runs a delta query and removes the entry from the state repository. This is intentional — FUSE serves from local state for performance. Stale entry cleanup is the sync engine's responsibility (Fase 1). During hydration, if the download URL returns 404, transition the entry to `Deleted` state and return `ENOENT`.
- [ ] T034 [P] [US1] Implement `fuser::Filesystem::setattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: handle permission changes (update `perm` field), timestamp changes (update `mtime`/`atime`/`ctime`), truncate (if size changes, mark as modified). Persist changes via WriteSerializer. Reply with updated `FileAttr`.
- [ ] T035 [P] [US1] Implement `fuser::Filesystem::statfs()` in `crates/lnxdrive-fuse/src/filesystem.rs`: return filesystem statistics. Use `cache.disk_usage()` for used blocks, `config.cache_max_size_gb * 1024^3` for total blocks. Block size = 4096. Reply with `ReplyStatfs`.
- [ ] T036 [US1] Implement `fuser::Filesystem::forget()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino: u64` and `nlookup: u64`, decrement `lookup_count` by `nlookup` on the InodeEntry. If `lookup_count == 0 && open_handles == 0`, the entry is eligible for GC (but do not remove yet — may be needed by parent directory listing).

### Directory Operations

- [ ] T037 [US1] Implement `fuser::Filesystem::readdir()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino: u64` and `offset: i64`, get children from `inode_table.children(ino)`. Prepend `.` (current dir) and `..` (parent dir) entries. Skip `offset` entries. For each remaining entry, call `reply.add(ino, offset+1, kind, name)`. If buffer full, stop. No network requests — purely local inode table. Target: <10ms for 1000 entries.
- [ ] T038 [P] [US1] Implement `fuser::Filesystem::opendir()` in `crates/lnxdrive-fuse/src/filesystem.rs`: validate the inode exists and is a directory. Allocate a file handle (use atomic u64 counter). Reply with `ReplyOpen` containing the fh and flags.
- [ ] T039 [P] [US1] Implement `fuser::Filesystem::releasedir()` in `crates/lnxdrive-fuse/src/filesystem.rs`: release the file handle allocated in `opendir`. No-op beyond cleanup.

### Mount/Unmount Public API

- [ ] T040 [US1] Implement public `mount()` function in `crates/lnxdrive-fuse/src/lib.rs`: takes `config: FuseConfig`, `db_pool: DatabasePool`, `rt_handle: Handle`. Creates `ContentCache`, `LnxDriveFs` instance. Calls `fuser::spawn_mount2(filesystem, mount_point, &mount_options)` with options from `contracts/fuse-operations.md` (AutoUnmount, FSName, Subtype, DefaultPermissions, NoAtime, Async). Returns `fuser::BackgroundSession` (RAII handle). Validate mount point exists and is empty before mounting.
- [ ] T041 [US1] Implement public `unmount()` function in `crates/lnxdrive-fuse/src/lib.rs`: takes the `BackgroundSession` handle and drops it (triggers `destroy()` and kernel unmount). Alternatively, if force unmount is needed, call `fuser::MountOption` to force.

### CLI Commands: mount/unmount

- [ ] T042 [US1] Create `crates/lnxdrive-cli/src/commands/mount.rs`: define `MountCommand` struct with clap derive: `--path <PATH>` optional override, `--foreground` / `-f` flag. Implement `execute()`: load config, open DB pool, get tokio Handle, validate prerequisites (authenticated account, mount point exists, mount point empty, FUSE available via `Path::new("/dev/fuse").exists()`), call `lnxdrive_fuse::mount()`, print success output per `contracts/cli-commands.md` (human or JSON based on `--json` flag). Define `UnmountCommand` struct: `--force` flag. Implement `execute()`: call unmount, print result.
- [ ] T043 [US1] Register mount/unmount commands in `crates/lnxdrive-cli/src/commands/mod.rs`: add `pub mod mount;`, update `crates/lnxdrive-cli/src/main.rs` `Commands` enum with `Mount(MountCommand)` and `Unmount(UnmountCommand)` variants, add match arms in the execute function

### Unit Tests for Stage 3

- [ ] T044 [P] [US1] Add unit tests for `LnxDriveFs::init()` in `crates/lnxdrive-fuse/src/filesystem.rs`: test that init loads items from DB into inode table, test root inode is 1, test inode assignment for new items, test re-mount preserves existing inodes
- [ ] T045 [P] [US1] Add unit tests for `lookup` and `getattr` in `crates/lnxdrive-fuse/src/filesystem.rs`: test lookup returns correct entry, test lookup increments lookup_count, test getattr returns real size for Online (placeholder) items, test ENOENT for non-existent items
- [ ] T046 [P] [US1] Add unit tests for `readdir` in `crates/lnxdrive-fuse/src/filesystem.rs`: test readdir includes `.` and `..`, test readdir returns all children, test offset-based pagination, test empty directory

**Checkpoint**: FUSE filesystem mounts, `ls -la` shows files with correct metadata, no network calls. US1 acceptance scenarios 1-4 pass.

---

## Stage 4: User Story 2 — Open a Cloud File and Have It Download Automatically (Priority: P1)

**Goal**: When a user opens a placeholder file, the system downloads the content from OneDrive and delivers it to the reading application.

**Independent Test**: Open a placeholder file with `cat`, verify content is delivered and state transitions to `hydrated`.

**Depends on**: Stage 3 complete (mount + readdir + getattr working)

### Hydration Manager

- [ ] T047 [US2] Implement `HydrationPriority` enum in `crates/lnxdrive-fuse/src/hydration.rs`: variants `UserOpen` (highest), `PinRequest`, `Prefetch` (lowest). Derive `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord`.
- [ ] T048 [US2] Implement `HydrationRequest` struct in `crates/lnxdrive-fuse/src/hydration.rs`: fields `ino: u64`, `item_id: UniqueId`, `remote_id: RemoteId`, `total_size: u64`, `downloaded: AtomicU64`, `cache_path: PathBuf`, `priority: HydrationPriority`, `created_at: DateTime<Utc>`, `progress_tx: tokio::sync::watch::Sender<u8>`. Method `progress() -> u8` computes `(downloaded / total_size * 100) as u8`.
- [ ] T049 [US2] Implement `HydrationManager` struct in `crates/lnxdrive-fuse/src/hydration.rs`: fields `active: DashMap<u64, Arc<HydrationRequest>>` (dedup by inode), `semaphore: Arc<Semaphore>` (concurrency limit from config), `cache: Arc<ContentCache>`, `write_handle: WriteSerializerHandle`, `rt_handle: Handle`. Constructor `new(config, cache, write_handle, rt_handle)`.
- [ ] T050 [US2] Implement `HydrationManager::hydrate()` in `crates/lnxdrive-fuse/src/hydration.rs`: takes `ino, item_id, remote_id, total_size, priority`. Check if already in `active` map (dedup — return existing watch::Receiver). If new: create `HydrationRequest`, insert into `active`, acquire semaphore permit, spawn download task. Download task: get download URL from Graph API (`GraphCloudProvider::get_download_url()` or equivalent), for files < 100MB do full download, for files >= 100MB use HTTP Range requests. Write to `cache.partial_path()`, update `downloaded` atomically, send progress via watch channel, update `hydration_progress` in DB via WriteSerializer. On completion: rename `.partial` to final path, update state to `Hydrated` via WriteSerializer, update InodeTable entry state, remove from `active` map. On error: set state to `Error`, remove from `active`. Return `watch::Receiver<u8>` for progress.
- [ ] T051 [US2] Implement `HydrationManager::wait_for_completion()` in `crates/lnxdrive-fuse/src/hydration.rs`: takes `ino`, blocks until hydration completes by subscribing to the watch channel and waiting for 100%. Returns `Result<()>`.
- [ ] T052 [US2] Implement `HydrationManager::wait_for_range()` in `crates/lnxdrive-fuse/src/hydration.rs`: takes `ino, offset, size`, blocks until the requested byte range is available in the cache file. For full downloads this waits for completion. For range-based downloads this can return sooner.
- [ ] T053 [P] [US2] Implement `HydrationManager::cancel()` in `crates/lnxdrive-fuse/src/hydration.rs`: takes `ino`, cancels the in-progress hydration (drop the task), remove from `active`, delete partial file, set state back to `Online`.
- [ ] T054 [P] [US2] Implement `HydrationManager::is_hydrating()` and `HydrationManager::progress()` in `crates/lnxdrive-fuse/src/hydration.rs`: check if ino is in `active` map, return current progress.

### FUSE File Operations (Read Path)

- [ ] T055 [US2] Implement `fuser::Filesystem::open()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino` and `flags`, look up InodeEntry. If entry is a directory: reply `EISDIR`. Increment `open_handles`. If state is `Online`: trigger hydration via `HydrationManager::hydrate()` with priority `UserOpen`. If state is `Hydrating`: get existing watch receiver. Update `last_accessed` via WriteSerializer. Reply with `ReplyOpen` containing fh and flags (set `FOPEN_KEEP_CACHE` if already hydrated).
- [ ] T056 [US2] Implement `fuser::Filesystem::read()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino, fh, offset, size`, look up InodeEntry. If state is `Hydrating`: call `hydration_manager.wait_for_range(ino, offset, size)` to block until data available. Read from `cache.read(remote_id, offset, size)`. Reply with data bytes. On error: reply `EIO`.
- [ ] T057 [US2] Implement `fuser::Filesystem::release()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino` and `fh`, decrement `open_handles` on InodeEntry. If `open_handles` reaches 0 and state is `Hydrated` (not pinned), file becomes eligible for future dehydration.
- [ ] T058 [P] [US2] Implement `fuser::Filesystem::flush()` in `crates/lnxdrive-fuse/src/filesystem.rs`: no-op per contract (writes go to cache immediately). Reply with Ok.

### Graph API Integration for Download

- [ ] T059 [US2] Add download URL method to `crates/lnxdrive-graph/src/provider.rs` (or verify it exists): method `get_download_url(remote_id: &RemoteId) -> Result<String>` that calls Microsoft Graph `GET /me/drive/items/{id}` and returns the `@microsoft.graph.downloadUrl` field. If this method already exists, verify its signature and adapt the call in HydrationManager accordingly.
- [ ] T060 [P] [US2] Add file content download method to `crates/lnxdrive-graph/src/provider.rs` (or verify it exists): method `download_file(download_url: &str, dest: &Path) -> Result<u64>` for full downloads, and `download_range(download_url: &str, dest: &Path, offset: u64, length: u64) -> Result<u64>` for range-based downloads. Use `reqwest` with streaming response body. Return bytes written.

### Unit Tests for Stage 4

- [ ] T061 [P] [US2] Add unit tests for `HydrationManager` in `crates/lnxdrive-fuse/src/hydration.rs`: test deduplication (two hydrate() calls for same ino return same watch receiver), test concurrency limit (semaphore blocks when limit reached), test progress tracking (watch receiver receives updates), test cancel removes from active map
- [ ] T062 [P] [US2] Add unit tests for `open` and `read` in `crates/lnxdrive-fuse/src/filesystem.rs`: test open on placeholder triggers hydration, test open increments open_handles, test read returns cached data, test release decrements open_handles, test double open with dedup

### Crash Recovery (FR-012)

- [ ] T096 Implement crash recovery in `LnxDriveFs::init()` in `crates/lnxdrive-fuse/src/filesystem.rs`: during init, scan for items with state `Hydrating` (stale from a crash). For each: check if `.partial` file exists in cache. If exists and size > 0: resume hydration from the downloaded offset. If not: reset state to `Online`. This handles edge case: "What happens when the FUSE daemon crashes while files are being hydrated?" Placed in Stage 4 because FR-012 (resumable hydration) requires crash recovery to be available before higher stages depend on hydration reliability.

**Checkpoint**: Opening a placeholder file triggers download, `cat` delivers content, state transitions Online→Hydrating→Hydrated. Crash recovery resumes interrupted hydrations. US2 acceptance scenarios 1-5 pass.

---

## Stage 5: User Story 3 — Edit Files Through the Virtual Filesystem (Priority: P1)

**Goal**: Users can write to files, create new files, delete files, and rename/move files in the mounted filesystem.

**Independent Test**: Write to a hydrated file, verify it transitions to `modified` state and the change is queued for sync.

**Depends on**: Stage 4 complete (hydration working)

### FUSE Write Operations

- [ ] T063 [US3] Implement `fuser::Filesystem::write()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino, fh, offset, data, flags`, look up InodeEntry. If state is `Online`: first hydrate fully (block), then write. If state is `Hydrating`: wait for completion, then write. Write data to cache file at offset via `cache.write_at(remote_id, offset, data)`. Update InodeEntry size if file grows. Transition state to `Modified` via WriteSerializer (unless already Modified). Reply with bytes written.
- [ ] T064 [P] [US3] Implement `ContentCache::write_at()` in `crates/lnxdrive-fuse/src/cache.rs`: method `write_at(remote_id: &RemoteId, offset: u64, data: &[u8]) -> Result<u32>` opens the cache file, seeks to offset, writes data, returns bytes written. Creates the file if it doesn't exist (for new files).
- [ ] T065 [US3] Implement `fuser::Filesystem::create()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent, name, mode, umask, flags`. Create a new SyncItem via WriteSerializer (with state `Modified`, no remote_id yet). Assign a new inode. Create InodeEntry in inode_table. Create an empty cache file. Reply with `ReplyCreate` containing the new entry's FileAttr, fh, and flags.
- [ ] T066 [US3] Implement `fuser::Filesystem::unlink()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent, name`. Look up the child inode via `inode_table.lookup(parent, name)`. Verify it's not a directory (EISDIR). Transition state to `Deleted` via WriteSerializer. Remove cached content via `cache.remove()`. Remove from inode_table. Reply Ok.
- [ ] T067 [US3] Implement `fuser::Filesystem::mkdir()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent, name, mode, umask`. Create a new directory SyncItem via WriteSerializer (state `Modified`). Assign inode. Create InodeEntry with `kind=Directory`. Reply with `ReplyEntry`.
- [ ] T068 [US3] Implement `fuser::Filesystem::rmdir()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent, name`. Look up child inode. Verify it's a directory (ENOTDIR otherwise). Verify it's empty (ENOTEMPTY if has children). Transition to `Deleted` via WriteSerializer. Remove from inode_table. Reply Ok.
- [ ] T069 [US3] Implement `fuser::Filesystem::rename()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `parent, name, newparent, newname, flags`. Look up source entry. If newname exists in newparent: handle replacement (unlink target first). Update InodeEntry: change `parent_ino` to `newparent`, change `name` to `newname`. Update state repo via WriteSerializer (update local_path). Mark as `Modified` if not already. Reply Ok.

### Sync Engine Integration

- [ ] T070 [US3] Verify sync engine picks up `Modified` and `Deleted` items: read `crates/lnxdrive-sync/src/engine.rs` to confirm it queries items with `Modified` state for upload and `Deleted` state for remote deletion. If the sync engine already handles these states, no changes needed. If not, add a query in the sync engine to include FUSE-created items in the upload queue. Document findings as a comment in the task.

### Unit Tests for Stage 5

- [ ] T071 [P] [US3] Add unit tests for `write` in `crates/lnxdrive-fuse/src/filesystem.rs`: test write to hydrated file transitions to Modified, test write to placeholder triggers hydration first, test write updates file size, test write_at creates file if needed
- [ ] T072 [P] [US3] Add unit tests for `create`, `unlink`, `mkdir`, `rmdir`, `rename` in `crates/lnxdrive-fuse/src/filesystem.rs`: test create assigns new inode with Modified state, test unlink removes entry and cached content, test mkdir creates directory entry, test rmdir fails on non-empty dir (ENOTEMPTY), test rename updates parent_ino and name

**Checkpoint**: Write, create, delete, rename all work. Modified files are queued for sync. US3 acceptance scenarios 1-6 pass.

---

## Stage 6: User Story 4 — Pin Files for Permanent Offline Access (Priority: P2)

**Goal**: Users can pin files/directories for permanent offline availability. Pinned files are hydrated immediately and never auto-dehydrated.

**Independent Test**: Pin a file, verify it gets hydrated and its state changes to `pinned`. Trigger dehydration sweep and confirm pinned file survives.

**Depends on**: Stage 4 complete (hydration working). Can run in parallel with Stage 5.

### Pin/Unpin Logic

- [ ] T073 [US4] Implement pin logic in `crates/lnxdrive-fuse/src/hydration.rs`: add method `HydrationManager::pin(ino, item_id, remote_id, total_size) -> Result<()>`. If state is `Online`: trigger hydration with priority `PinRequest`, on completion set state to `Pinned`. If state is `Hydrated`: directly transition to `Pinned`. If state is `Pinned`: no-op. If state is `Hydrating`: wait for completion, then set `Pinned`. Update via WriteSerializer.
- [ ] T074 [US4] Implement unpin logic in `crates/lnxdrive-fuse/src/hydration.rs`: add method `HydrationManager::unpin(ino) -> Result<()>`. If state is `Pinned`: transition to `Hydrated`. Otherwise: no-op or error. Update via WriteSerializer.
- [ ] T075 [US4] Implement recursive pin for directories in `crates/lnxdrive-fuse/src/hydration.rs`: add method `HydrationManager::pin_recursive(parent_ino, inode_table) -> Result<Vec<(u64, ItemState)>>`. Iterate all children of `parent_ino` in inode_table, pin each file (skip directories but recurse into them). Return list of (ino, new_state) for reporting.

### CLI Commands: pin/unpin

- [ ] T076 [P] [US4] Create `crates/lnxdrive-cli/src/commands/pin.rs`: define `PinCommand` struct with clap derive: `paths: Vec<PathBuf>` (required, one or more). Implement `execute()`: validate each path is within the mount point, resolve to inode, call pin logic (via IPC or direct if in-process). Output per `contracts/cli-commands.md`. Define `UnpinCommand` struct similarly. Implement `execute()` for unpin.
- [ ] T077 [US4] Register pin/unpin commands in `crates/lnxdrive-cli/src/commands/mod.rs`: add `pub mod pin;`, update `crates/lnxdrive-cli/src/main.rs` `Commands` enum with `Pin(PinCommand)` and `Unpin(UnpinCommand)` variants, add match arms

### Unit Tests for Stage 6

- [ ] T078 [P] [US4] Add unit tests for pin/unpin in `crates/lnxdrive-fuse/src/hydration.rs`: test pin on Online triggers hydration then sets Pinned, test pin on Hydrated directly sets Pinned, test pin is idempotent (Pinned→Pinned), test unpin sets Hydrated, test pin_recursive processes all children

**Checkpoint**: Pin/unpin CLI works, pinned files have state=pinned, pinned files survive dehydration sweeps. US4 acceptance scenarios 1-5 pass.

---

## Stage 7: User Story 5 — Automatic Dehydration to Reclaim Disk Space (Priority: P2)

**Goal**: System automatically reclaims disk space by dehydrating least-recently-accessed unpinned files when cache exceeds threshold.

**Independent Test**: Hydrate several files, set a low cache threshold, verify dehydration sweep removes LRU unpinned files.

**Depends on**: Stage 4 complete (hydration working). Can run in parallel with Stages 5 and 6.

### Dehydration Manager

- [ ] T079 [US5] Implement `DehydrationPolicy` struct in `crates/lnxdrive-fuse/src/dehydration.rs`: fields `cache_max_bytes: u64`, `threshold_percent: u8`, `max_age_days: u32`, `interval_minutes: u32`. Constructor `from_config(config: &FuseConfig) -> Self` that converts `cache_max_size_gb` to bytes, copies other fields.
- [ ] T080 [US5] Implement `DehydrationManager` struct in `crates/lnxdrive-fuse/src/dehydration.rs`: fields `policy: DehydrationPolicy`, `cache: Arc<ContentCache>`, `inode_table: Arc<InodeTable>`, `write_handle: WriteSerializerHandle`, `db_pool: DatabasePool`. Constructor `new(policy, cache, inode_table, write_handle, db_pool)`.
- [ ] T081 [US5] Implement `DehydrationManager::run_sweep()` in `crates/lnxdrive-fuse/src/dehydration.rs`: check `cache.disk_usage()` against `policy.cache_max_bytes * policy.threshold_percent / 100`. If below threshold: return. If above: query DB for dehydration candidates via `get_items_for_dehydration(max_age_days, limit=100)`. For each candidate: check `inode_table` for `open_handles > 0` (skip if open), check state is `Hydrated` (not Pinned/Modified/Hydrating), call `cache.remove(remote_id)`, transition state to `Online` via WriteSerializer, update InodeEntry state. Continue until usage drops below threshold or no more candidates.
- [ ] T082 [US5] Implement `DehydrationManager::start_periodic()` in `crates/lnxdrive-fuse/src/dehydration.rs`: spawn a tokio task that calls `run_sweep()` every `policy.interval_minutes` using `tokio::time::interval`. Return a `JoinHandle` for cancellation on shutdown.
- [ ] T083 [US5] Implement manual dehydration in `DehydrationManager`: method `dehydrate_path(ino: u64) -> Result<u64>` that dehydrates a specific file (checking it's not pinned, not open, not modified). Returns freed bytes. Method `dehydrate_paths(inos: Vec<u64>) -> Result<DehydrationReport>` for batch.

### CLI Commands: hydrate/dehydrate

- [ ] T084 [P] [US5] Create `crates/lnxdrive-cli/src/commands/hydrate.rs`: define `HydrateCommand` struct with clap derive: `paths: Vec<PathBuf>`. Implement `execute()`: resolve paths to inodes, call hydration for each, show progress, output per `contracts/cli-commands.md`. Define `DehydrateCommand` struct: `paths: Vec<PathBuf>`, `--force` flag. Implement `execute()`: resolve paths, call manual dehydrate, output freed bytes.
- [ ] T085 [US5] Register hydrate/dehydrate commands in `crates/lnxdrive-cli/src/commands/mod.rs`: add `pub mod hydrate;`, update `crates/lnxdrive-cli/src/main.rs` `Commands` enum with `Hydrate(HydrateCommand)` and `Dehydrate(DehydrateCommand)` variants, add match arms

### Integration with Mount Lifecycle

- [ ] T086 [US5] Wire `DehydrationManager::start_periodic()` into `LnxDriveFs` lifecycle in `crates/lnxdrive-fuse/src/filesystem.rs` or `crates/lnxdrive-fuse/src/lib.rs`: start the periodic sweep when mount() is called, cancel the task when unmount() occurs (on destroy).

### Unit Tests for Stage 7

- [ ] T087 [P] [US5] Add unit tests for `DehydrationManager` in `crates/lnxdrive-fuse/src/dehydration.rs`: test sweep dehydrates LRU files when over threshold, test sweep skips pinned files, test sweep skips files with open handles, test sweep skips modified files, test sweep stops when under threshold, test manual dehydrate_path works, test manual dehydrate_path rejects pinned files

**Checkpoint**: Dehydration sweeps run periodically, LRU files are reclaimed, pinned/open/modified files are protected. US5 acceptance scenarios 1-5 pass.

---

## Stage 8: User Story 6 — View File State via Extended Attributes (Priority: P3)

**Goal**: Files expose their LNXDrive state, size, remote ID, and hydration progress via the `user.lnxdrive.*` xattr namespace.

**Independent Test**: Use `getfattr -d` on files in different states and verify correct attributes.

**Depends on**: Stage 3 complete (mount working). Can run in parallel with Stages 4-7.

### Extended Attributes Handler

- [ ] T088 [US6] Implement xattr constants in `crates/lnxdrive-fuse/src/xattr.rs`: define `XATTR_STATE: &str = "user.lnxdrive.state"`, `XATTR_SIZE: &str = "user.lnxdrive.size"`, `XATTR_REMOTE_ID: &str = "user.lnxdrive.remote_id"`, `XATTR_PROGRESS: &str = "user.lnxdrive.progress"`. Define `ALL_XATTRS: &[&str]` array.
- [ ] T089 [US6] Implement `fuser::Filesystem::getxattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino` and `name`, look up InodeEntry. Match on xattr name: `user.lnxdrive.state` → return state as string ("online", "hydrating", "hydrated", "pinned", "modified"), `user.lnxdrive.size` → return size as string, `user.lnxdrive.remote_id` → return remote_id as string (or ENODATA if None), `user.lnxdrive.progress` → return progress as string (or ENODATA if not hydrating). If `size` param is 0: reply with the data length. If buffer too small: reply `ERANGE`. Otherwise reply with data.
- [ ] T090 [US6] Implement `fuser::Filesystem::listxattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: given `ino` and `size`, return the null-separated list of `user.lnxdrive.*` attribute names. If `user.lnxdrive.progress` should only appear when state is Hydrating, conditionally include it. If size is 0: reply with total length. If buffer too small: reply `ERANGE`.
- [ ] T091 [P] [US6] Implement `fuser::Filesystem::setxattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: reject all external writes to `user.lnxdrive.*` namespace with `EACCES`. These attributes are system-managed.
- [ ] T092 [P] [US6] Implement `fuser::Filesystem::removexattr()` in `crates/lnxdrive-fuse/src/filesystem.rs`: reject all removals with `EACCES`. Attributes are system-managed.

### Unit Tests for Stage 8

- [ ] T093 [P] [US6] Add unit tests for xattr operations in `crates/lnxdrive-fuse/src/xattr.rs` or `crates/lnxdrive-fuse/src/filesystem.rs`: test getxattr returns correct state string for each ItemState, test getxattr returns correct size, test getxattr returns ENODATA for missing remote_id, test listxattr returns all names, test setxattr rejects writes, test removexattr rejects removals, test progress only returned when hydrating

**Checkpoint**: All `user.lnxdrive.*` attributes are readable via `getfattr`. US6 acceptance scenarios 1-4 pass.

---

## Stage 9: User Story Integration — Status & Daemon

**Goal**: Integrate FUSE status into existing `status` command and daemon auto-mount.

**Depends on**: Stages 3-8 complete

### Status Command Extension

- [ ] T094 [P] Extend `crates/lnxdrive-cli/src/commands/status.rs`: add FUSE section to output. Query inode_table for counts (hydrated, pinned, online, hydrating). Query cache for disk usage. Output per `contracts/cli-commands.md` (human format: mount state, cache usage, file counts; JSON format: `fuse` object).

### Daemon Auto-Mount

- [ ] T095 Extend `crates/lnxdrive-daemon/` (or `crates/lnxdrive-cli/src/commands/daemon.rs`): when daemon starts and `config.fuse.auto_mount` is true, call `lnxdrive_fuse::mount()`. On daemon stop, call unmount. Ensure graceful shutdown (complete or cancel in-progress hydrations).

---

## Stage 10: Polish & Cross-Cutting Concerns

**Purpose**: Performance, robustness, documentation, and final validation.

- [ ] T097 [P] Add tracing instrumentation to all FUSE operations in `crates/lnxdrive-fuse/src/filesystem.rs`: add `#[tracing::instrument]` to each Filesystem method, log at `debug` level for normal operations, `warn` for errors, `info` for state transitions
- [ ] T098 [P] Add input validation to all FUSE operations in `crates/lnxdrive-fuse/src/filesystem.rs`: validate file name length (< 255 bytes → ENAMETOOLONG), validate inode exists before operations, validate file type matches expected (e.g., readdir on file → ENOTDIR)
- [ ] T099 [P] Handle concurrent access edge cases in `crates/lnxdrive-fuse/src/filesystem.rs`: ensure mmap compatibility (read returns correct data for memory-mapped files), ensure two processes reading same file during hydration both get correct data, ensure write during hydration blocks correctly. **U1 Note (mmap)**: FUSE handles mmap via `read()` by default — no special implementation needed. For unhydrated files accessed via mmap, the kernel will issue `read()` calls which trigger hydration. If hydration fails or times out, return `EIO` to the mmap read. Document this behavior in code comments.
- [ ] T100 [P] Add documentation comments to all public APIs in `crates/lnxdrive-fuse/src/lib.rs`: document `mount()`, `unmount()`, re-exported types with usage examples
- [ ] T101 [P] Run performance validation: verify `getattr` completes in <1ms (add benchmark test or tracing metric), verify `readdir` completes in <10ms for 1000 entries, verify idle memory <50MB with 10k tracked files
- [ ] T102 Run full test suite: `cargo test --workspace --exclude lnxdrive-conflict --exclude lnxdrive-audit --exclude lnxdrive-telemetry`
- [ ] T103 Run clippy: `cargo clippy --workspace --exclude lnxdrive-conflict --exclude lnxdrive-audit --exclude lnxdrive-telemetry -- -D warnings`
- [ ] T104 Verify all 44 functional requirements (FR-001 through FR-044) from spec.md are addressed. Mark any gaps.
- [ ] T105 Run quickstart.md validation steps end-to-end (requires FUSE support on host or container)

---

## Dependencies & Execution Order

### Stage Dependencies

```
Stage 1 (Setup)
    │
    ▼
Stage 2 (Foundational) ──── BLOCKS ALL user stories
    │
    ├─── Stage 3 (US1: Browse) ──── MVP
    │        │
    │        ├─── Stage 4 (US2: Hydration) ─── depends on US1
    │        │        │
    │        │        ├─── Stage 5 (US3: Write) ─── depends on US2
    │        │        │
    │        │        ├─── Stage 6 (US4: Pin) ─── depends on US2, PARALLEL with US3
    │        │        │
    │        │        └─── Stage 7 (US5: Dehydration) ─── depends on US2, PARALLEL with US3, US4
    │        │
    │        └─── Stage 8 (US6: Xattr) ─── depends on US1 only, PARALLEL with US2-US5
    │
    └─── Stage 9 (Integration) ─── depends on Stages 3-8
              │
              ▼
         Stage 10 (Polish) ─── depends on Stage 9
```

### Parallel Opportunities Summary

**Within Stage 1** (4 tasks in parallel):
- T001, T002, T003, T004 can all run simultaneously (different files)

**Within Stage 2** (multiple parallel groups):
- Group A: T007, T008, T010, T012 (domain model, different methods)
- Group B: T013, T015, T016 (config changes, different files)
- Group C: T017, T020 (database, tests)
- Group D: T019 (repository methods, depends on T017)
- Group E: T021 (error types, independent)
- Group F: T022, T023 (write serializer, independent)
- Group G: T024, T025 (cache, independent)
- Group H: T026, T027, T028 (inode, independent)

**After Stage 2 (cross-stage parallelism)**:
- Stage 8 (US6: Xattr) can start immediately after Stage 3, in parallel with Stages 4-7
- Stage 6 (US4: Pin) can start after Stage 4, in parallel with Stage 5
- Stage 7 (US5: Dehydration) can start after Stage 4, in parallel with Stages 5 and 6

**Within Stage 3**:
- T034, T035 (setattr, statfs) are parallel with each other
- T038, T039 (opendir, releasedir) are parallel with each other
- T044, T045, T046 (tests) are all parallel

**Within Stage 4**:
- T053, T054 (cancel, progress query) are parallel
- T058 (flush) is parallel with other file ops
- T060 (download methods) is parallel with T059
- T061, T062 (tests) are all parallel

**Within Stage 5**:
- T064 (cache write_at) is parallel with T063 (depends logically but different file)
- T071, T072 (tests) are all parallel

**Within Stage 6**:
- T076 (CLI commands) is parallel with T073-T075 (core logic)
- T078 (tests) is parallel with other test tasks

**Within Stage 7**:
- T084 (CLI commands) is parallel with T079-T083 (core logic)
- T087 (tests) is parallel with other test tasks

**Within Stage 8**:
- T091, T092 (setxattr, removexattr) are parallel
- T093 (tests) is parallel

**Within Stage 10**:
- T097, T098, T099, T100, T101 are all parallel

### Parallel Execution Examples

```bash
# Stage 1 — All 4 tasks in parallel:
Agent A: "T001 — Add workspace dependencies to Cargo.toml"
Agent B: "T002 — Update lnxdrive-fuse/Cargo.toml"
Agent C: "T003 — Add lnxdrive-fuse to lnxdrive-cli/Cargo.toml"
Agent D: "T004 — Create empty module files for FUSE crate"

# Stage 2 — Launch parallel groups:
Agent A: "T007 — Add Pinned variant to ItemState"
Agent B: "T013 — Create FuseConfig struct"
Agent C: "T017 — Create SQL migration for FUSE support"
Agent D: "T021 — Implement FuseError enum"
Agent E: "T024 — Implement ContentCache"
Agent F: "T026 — Implement InodeEntry struct"
# Then after these complete:
Agent A: "T009 — Update state transition rules"
Agent B: "T014 — Add FuseConfig to Config struct"
Agent C: "T019 — Extend SqliteStateRepository"
Agent D: "T022 — Implement WriteSerializer"
Agent E: "T027 — Implement InodeTable"

# Post-Stage 3 — Cross-story parallelism:
Agent A: "Stage 4 (US2: Hydration)"
Agent B: "Stage 8 (US6: Xattr)" ← can start immediately, no US2 dependency

# Post-Stage 4 — Three stories in parallel:
Agent A: "Stage 5 (US3: Write)"
Agent B: "Stage 6 (US4: Pin)"
Agent C: "Stage 7 (US5: Dehydration)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Stage 1: Setup
2. Complete Stage 2: Foundational (CRITICAL — blocks all stories)
3. Complete Stage 3: User Story 1 (Browse without downloading)
4. **STOP and VALIDATE**: Mount the filesystem, run `ls -la`, confirm metadata is correct
5. This is a useful MVP: users can see their OneDrive files without downloading anything

### Incremental Delivery

1. Setup + Foundational → Infrastructure ready
2. Add US1 (Browse) → Test independently → **MVP!**
3. Add US2 (Hydration) → Test independently → Files open and download on demand
4. Add US3 (Write) + US4 (Pin) + US5 (Dehydration) → in parallel → Full lifecycle
5. Add US6 (Xattr) → Desktop integration ready
6. Integration + Polish → Production ready

### Parallel Subagent Strategy

With subagents working simultaneously:

1. **Round 1**: All Stage 1 tasks (4 agents in parallel)
2. **Round 2**: Stage 2 foundational tasks, 6+ agents in parallel on independent groups
3. **Round 3**: Stage 2 dependent tasks, 5 agents in parallel
4. **Round 4**: Stage 3 (US1) — mostly sequential but some parallel ops
5. **Round 5**: Stage 4 (US2) + Stage 8 (US6) — 2 agents
6. **Round 6**: Stage 5 (US3) + Stage 6 (US4) + Stage 7 (US5) — 3 agents
7. **Round 7**: Stage 9 (Integration) — 2 agents
8. **Round 8**: Stage 10 (Polish) — 5 agents on parallel tasks

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks in the same stage
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable after its stage
- "Stages" are spec-internal; "Fases" refer to the project-wide roadmap
- Commit after each task or logical group
- Stop at any checkpoint to validate the story independently
- The FUSE crate currently has only doc comments — all implementation is new
- Integration tests requiring `/dev/fuse` must run in a container (not on standard CI)
