# Feature Specification: Files-on-Demand (FUSE Virtual Filesystem)

**Feature Branch**: `feat/002-files-on-demand`
**Created**: 2026-02-04
**Status**: Draft
**Input**: User description: "Implement Fase 2 'Files-on-Demand' from the project roadmap — a FUSE-based virtual filesystem providing placeholder files, on-demand hydration, automatic dehydration, and extended attributes for file state metadata."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Browse OneDrive Files Without Downloading (Priority: P1)

A user mounts their OneDrive folder and browses its contents in a file manager (Nautilus, Dolphin, Thunar) or terminal. All files and directories from the cloud appear as local entries, showing their real names, sizes, and timestamps — but no content is actually downloaded until the user opens a file. The user sees the full folder tree instantly and can navigate freely.

**Why this priority**: This is the foundational capability of Files-on-Demand. Without it, none of the other stories work. It replaces the current "download everything" model with a space-efficient placeholder system that enables access to cloud files without consuming local disk space.

**Independent Test**: Mount the FUSE filesystem, list a directory with 100+ items using `ls -la`, confirm all entries appear with correct metadata (name, size, timestamps, permissions), and confirm zero network downloads occurred for file content.

**Acceptance Scenarios**:

1. **Given** a mounted FUSE filesystem with 500 synced items in the state repository, **When** the user runs `ls -la ~/OneDrive/Documents/`, **Then** all entries are listed with their real sizes, modification dates, and types (file/directory) within 10ms, and no file content is downloaded.
2. **Given** a placeholder file with `state=online` and `size=15MB`, **When** the user inspects the file with `stat`, **Then** the reported size matches the real remote file size (15MB), not the on-disk size (0 bytes).
3. **Given** a mounted filesystem, **When** a file manager opens the directory, **Then** directory entries load without delay and display overlay icons indicating file state (cloud/local/syncing).
4. **Given** the user navigates into a subdirectory, **When** the entries have not been cached locally, **Then** the system fetches the directory listing from the state repository and presents it within the expected latency.

---

### User Story 2 - Open a Cloud File and Have It Download Automatically (Priority: P1)

When the user opens a placeholder file (e.g., double-clicks in the file manager or runs `cat report.pdf`), the system transparently downloads the file content from OneDrive and delivers it to the requesting application. The user does not need to run any manual sync command — the file "just works" as if it were local.

**Why this priority**: On-demand hydration is the core value proposition of Files-on-Demand. Without transparent hydration, placeholder files are useless.

**Independent Test**: Create a placeholder file, open it with `cat` or an application, verify the full content is delivered and the file transitions from `online` to `hydrated` state.

**Acceptance Scenarios**:

1. **Given** a placeholder file (`state=online`, `size=50KB`), **When** the user opens it with `cat ~/OneDrive/notes.txt`, **Then** the system downloads the content from OneDrive, delivers it to `cat`, and the file transitions to `hydrated` state.
2. **Given** a placeholder file being hydrated, **When** a progress-aware application reads it, **Then** the system streams content as it downloads rather than blocking until the full download completes.
3. **Given** a large file (500MB) being hydrated, **When** the user checks its state, **Then** the system reports hydration progress as a percentage.
4. **Given** two different processes open the same placeholder file simultaneously, **When** both request content, **Then** only one download is initiated and both processes receive the same data without corruption.
5. **Given** a file is being hydrated, **When** the network connection drops, **Then** the system reports a meaningful error to the application, retains any partially downloaded data, and resumes the download when connectivity returns.

---

### User Story 3 - Edit Files Through the Virtual Filesystem (Priority: P1)

A user modifies a hydrated file using any application (text editor, office suite, image editor). The system detects the change, marks the file as modified, and queues it for upload back to OneDrive. Users can also create new files, delete files, and rename or move files within the mounted filesystem. The existing sync engine handles the actual upload.

**Why this priority**: Write support is essential for a functional filesystem. Without it, the FUSE mount would be read-only and severely limited in usefulness.

**Independent Test**: Mount the filesystem, write to a hydrated file, verify the file transitions to `modified` state and the sync engine picks it up for upload.

**Acceptance Scenarios**:

1. **Given** a hydrated file, **When** the user modifies it via `echo "new content" >> file.txt`, **Then** the write succeeds, the file transitions to `modified` state, and the change is queued for sync.
2. **Given** a placeholder file (`state=online`), **When** a process attempts to write to it, **Then** the system first hydrates the file completely, then applies the write, and transitions the file to `modified` state.
3. **Given** a file is being hydrated, **When** a write operation arrives, **Then** the write blocks until hydration completes, then proceeds normally.
4. **Given** a new file is created in the mounted directory (e.g., `touch ~/OneDrive/new.txt`), **Then** the system creates a local entry, assigns it `modified` state, and queues it for upload.
5. **Given** a file or directory is deleted from the mounted directory, **Then** the system marks it as `deleted` and queues the deletion for sync.
6. **Given** a file is renamed or moved within the mounted filesystem, **Then** the state repository is updated and the rename is queued for sync.

---

### User Story 4 - Pin Files for Permanent Offline Access (Priority: P2)

A user marks specific files or folders as "always available offline" (pinned). These pinned items are always kept fully hydrated and are never automatically dehydrated, ensuring the user can access them without an internet connection (e.g., on a plane or in a rural area).

**Why this priority**: Pinning is a critical user control mechanism that provides predictable offline access. It depends on hydration (US2) being functional first, but is essential for real-world usability.

**Independent Test**: Pin a file, verify it gets hydrated immediately, then trigger a dehydration sweep and confirm the pinned file remains hydrated while unpinned files are dehydrated.

**Acceptance Scenarios**:

1. **Given** a hydrated file, **When** the user pins it (via CLI: `lnxdrive pin ~/OneDrive/critical.pdf`), **Then** the file state changes to `pinned` and its extended attribute updates to `user.lnxdrive.state=pinned`.
2. **Given** a placeholder file, **When** the user pins it, **Then** the system immediately hydrates the file and sets its state to `pinned`.
3. **Given** a pinned file, **When** automatic dehydration runs due to low disk space, **Then** the pinned file is never dehydrated.
4. **Given** a pinned folder, **When** new files are synced into that folder from the cloud, **Then** the new files are automatically hydrated and pinned.
5. **Given** a pinned file, **When** the user unpins it, **Then** the file transitions to `hydrated` state and becomes eligible for future dehydration.

---

### User Story 5 - Automatic Dehydration to Reclaim Disk Space (Priority: P2)

The system automatically reclaims local disk space by dehydrating (removing local content of) files that have not been accessed recently and are not pinned. The user configures a disk space threshold, and when usage exceeds that threshold, the system dehydrates files in least-recently-accessed order.

**Why this priority**: Dehydration completes the Files-on-Demand lifecycle and enables users with limited disk space to access large OneDrive libraries. It depends on both hydration and the pinning mechanism.

**Independent Test**: Hydrate several files, configure a disk space threshold, verify that when the threshold is exceeded, the least-recently-accessed unpinned files are dehydrated back to placeholders.

**Acceptance Scenarios**:

1. **Given** the configured disk space threshold is 80%, **When** OneDrive folder usage exceeds 80% of the configured limit, **Then** the system dehydrates the least-recently-accessed unpinned files until usage drops below the threshold.
2. **Given** a hydrated file not accessed for 30 days (configurable), **When** a dehydration sweep runs, **Then** the file content is removed locally, the file becomes a placeholder again (`state=online`), and its metadata (name, size, timestamps) is preserved.
3. **Given** a file is currently open by a process, **When** a dehydration sweep selects it, **Then** the system skips that file and dehydrates the next eligible candidate instead.
4. **Given** a modified file with pending upload, **When** dehydration is triggered, **Then** the file is NOT dehydrated until the upload completes.
5. **Given** the user has configured dehydration policy in the configuration file, **When** the daemon starts, **Then** the configured thresholds and age limits are applied.

---

### User Story 6 - View File State via Extended Attributes (Priority: P3)

A power user or desktop integration component can query the state of any file using standard Linux extended attributes (`getfattr`). Each file exposes its LNXDrive state, real size, remote ID, and hydration progress through the `user.lnxdrive.*` namespace.

**Why this priority**: Extended attributes enable integration with file managers, scripts, and third-party tools. They are not required for core functionality but are critical for the desktop integration planned in Fase 3.

**Independent Test**: Use `getfattr -d` on files in different states and verify the correct attributes are returned for each state.

**Acceptance Scenarios**:

1. **Given** a placeholder file, **When** running `getfattr -n user.lnxdrive.state ~/OneDrive/file.txt`, **Then** the output shows `user.lnxdrive.state="online"`.
2. **Given** a file currently being hydrated at 45%, **When** reading `user.lnxdrive.progress`, **Then** the value is `"45"`.
3. **Given** any synced file, **When** reading `user.lnxdrive.size`, **Then** the value matches the real file size in the cloud.
4. **Given** any synced file with a remote ID, **When** reading `user.lnxdrive.remote_id`, **Then** the value matches the OneDrive item ID.

---

### Edge Cases

- What happens when the FUSE daemon crashes or is killed while files are being hydrated? Partially downloaded files must be detectable on restart, and hydration must resume or restart cleanly.
- What happens when the underlying storage (disk) runs out of space during hydration? The system must report a clear error (`ENOSPC`) to the application and leave the file in a consistent state (either fully hydrated or reverted to placeholder).
- What happens when the user moves or renames a file within the mounted filesystem? The rename must be reflected in the state repository and queued for sync.
- What happens when the same file is opened by multiple processes, one reading and one writing? The system must ensure data consistency — reads see either the old or new content, never a corrupted mix.
- What happens when an application reads a file using `mmap()` instead of `read()`? The FUSE layer must handle memory-mapped I/O correctly or fall back gracefully.
- What happens when the state repository (SQLite) is unavailable or locked? The FUSE layer must return appropriate errors (e.g., `EIO`) without crashing.
- What happens when the daemon is shut down gracefully? All in-progress hydrations must complete or be cleanly interrupted, and the filesystem must unmount cleanly.
- What happens when a user accesses a file that has been deleted from OneDrive since the last sync? The system must detect the stale entry and return an appropriate error after attempting a refresh.
- What happens when the mount point directory already contains files? The system must refuse to mount and report a clear error, or mount over the existing directory with appropriate warnings.
- What happens when the system has thousands of files in `hydrating` state due to a bulk pin operation? The hydration queue must respect concurrency limits and process requests fairly.

---

## Requirements *(mandatory)*

### Functional Requirements

#### Mounting and Filesystem Operations

- **FR-001**: System MUST provide a FUSE-based virtual filesystem that can be mounted at a user-configurable path (default: `~/OneDrive`).
- **FR-002**: System MUST present all synced OneDrive items (files and directories) as regular filesystem entries with correct names, sizes, timestamps, and permissions.
- **FR-003**: System MUST support standard POSIX filesystem operations: `getattr`, `readdir`, `open`, `read`, `write`, `create`, `unlink`, `mkdir`, `rmdir`, `rename`, `setattr`.
- **FR-004**: The `getattr` operation MUST return the real remote file size for placeholder files, not the on-disk size.
- **FR-005**: The `readdir` operation MUST return directory listings from the local state repository without triggering network requests.
- **FR-006**: System MUST support a clean unmount process that completes or safely interrupts all in-progress operations.

#### Hydration (On-Demand Download)

- **FR-007**: System MUST automatically hydrate (download content from OneDrive) a placeholder file when a process opens it for reading.
- **FR-008**: System MUST stream file content during hydration, allowing applications to read data as it downloads rather than waiting for the full download.
- **FR-009**: System MUST deduplicate concurrent hydration requests for the same file — if two processes open the same placeholder, only one download occurs.
- **FR-010**: System MUST track hydration progress as a percentage and expose it via extended attributes.
- **FR-011**: System MUST handle hydration failures gracefully, reporting errors to the requesting application and allowing retry on next access.
- **FR-012**: System MUST support resumable hydration — if interrupted, partial data is retained and the download resumes from where it left off.
- **FR-013**: System MUST queue hydration requests and process them with configurable concurrency to avoid overwhelming network and storage.

#### Dehydration (Space Reclamation)

- **FR-014**: System MUST support automatic dehydration of hydrated files based on a configurable disk space threshold.
- **FR-015**: System MUST use a least-recently-accessed eviction policy for selecting files to dehydrate.
- **FR-016**: System MUST NOT dehydrate files that are pinned, currently open by a process, or have pending modifications awaiting sync.
- **FR-017**: System MUST preserve file metadata (name, size, timestamps, remote ID) when dehydrating, reverting the file to placeholder state.
- **FR-018**: System MUST support a configurable maximum age for hydrated files, dehydrating files not accessed within the configured period.

#### Pinning

- **FR-019**: System MUST allow users to pin files and directories for permanent offline availability.
- **FR-020**: Pinning a placeholder file MUST trigger immediate hydration.
- **FR-021**: Pinning a directory MUST apply recursively to all current and future contents.
- **FR-022**: System MUST allow users to unpin files and directories, making them eligible for dehydration.

#### Write Support

- **FR-023**: System MUST support write operations on hydrated files, transitioning them to `modified` state.
- **FR-024**: Writing to a placeholder file MUST first hydrate the file completely, then apply the write.
- **FR-025**: Writing to a file currently being hydrated MUST block until hydration completes, then proceed.
- **FR-026**: System MUST support creating new files and directories in the mounted filesystem.
- **FR-027**: System MUST support deleting files and directories, marking them for sync deletion.
- **FR-028**: System MUST support renaming and moving files within the mounted filesystem.
- **FR-029**: All write operations MUST be queued for the sync engine to upload changes to OneDrive.

#### Extended Attributes

- **FR-030**: System MUST expose file state via the `user.lnxdrive.state` extended attribute with values: `online`, `hydrating`, `hydrated`, `pinned`, `modified`.
- **FR-031**: System MUST expose the real file size via the `user.lnxdrive.size` extended attribute.
- **FR-032**: System MUST expose the OneDrive item ID via the `user.lnxdrive.remote_id` extended attribute.
- **FR-033**: System MUST expose hydration progress (0-100) via the `user.lnxdrive.progress` extended attribute for files in `hydrating` state.

#### State Management and Concurrency

- **FR-034**: System MUST serialize write operations to the state repository to prevent race conditions between FUSE operations and the sync engine.
- **FR-035**: System MUST track open file handles to prevent dehydration of files currently in use.
- **FR-036**: System MUST maintain an inode-to-item mapping for efficient filesystem lookups.
- **FR-037**: System MUST handle concurrent access to the same file from multiple processes safely.

#### CLI Integration

- **FR-038**: System MUST provide CLI commands for FUSE management: `lnxdrive mount`, `lnxdrive unmount`.
- **FR-039**: System MUST provide CLI commands for pinning: `lnxdrive pin <path>`, `lnxdrive unpin <path>`.
- **FR-040**: System MUST provide a CLI command to manually hydrate a file: `lnxdrive hydrate <path>`.
- **FR-041**: System MUST provide a CLI command to manually dehydrate a file: `lnxdrive dehydrate <path>`.
- **FR-042**: System MUST integrate with the existing daemon so the FUSE filesystem is mounted automatically when the daemon starts.

#### Configuration

- **FR-043**: System MUST support configuration via the existing configuration file with a `fuse` section including: mount_point, auto_mount, dehydration_threshold_percent, dehydration_max_age_days, hydration_concurrency.
- **FR-044**: System MUST support overriding the mount point via CLI flag.

### Key Entities

- **PlaceholderFile**: A virtual filesystem entry that shows metadata (name, size, timestamps) without local content. Transitions to HydratedFile upon access. On-disk representation is a sparse/empty file with extended attributes carrying state and metadata.
- **HydratedFile**: A file whose content has been fully downloaded from the cloud and is available locally. Can transition back to PlaceholderFile via dehydration, or to ModifiedFile upon user edit.
- **PinnedFile**: A hydrated file explicitly marked by the user for permanent offline access. Immune to automatic dehydration. Transitions to HydratedFile when unpinned.
- **HydrationRequest**: A queued request to download a file's content. Tracks: file ID, priority, progress, requestor, and retry state.
- **DehydrationPolicy**: Configuration governing when and how files are dehydrated. Includes: disk threshold, max age, exclusion rules (pinned, open, modified).
- **InodeMap**: A mapping between FUSE inode numbers and item identifiers, enabling efficient lookups for filesystem operations.

### Assumptions

- The state repository already contains metadata for all OneDrive items from the delta sync process (Fase 1). The FUSE filesystem reads from this repository and does not perform its own cloud enumeration.
- The sync engine from Fase 1 continues to handle delta queries, upload queuing, and conflict detection. The FUSE layer adds hydration/dehydration operations but does not replace the sync engine.
- The user has FUSE support available on their system (`libfuse3` installed, `/dev/fuse` accessible). The system will check and report if FUSE is not available.
- Files-on-Demand operates on a single OneDrive account. Multi-account support is deferred to Fase 6.
- The FUSE filesystem runs as a userspace process under the user's account — no root privileges are required.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can browse a directory with 1,000 entries in the mounted filesystem within 10ms (directory listing from cache, no network requests).
- **SC-002**: File metadata queries (`stat`, `ls -la`) return results within 1ms for any file, regardless of whether it is a placeholder or hydrated.
- **SC-003**: Opening a 1MB placeholder file delivers the first byte of content to the reading application within 2 seconds (including network round-trip for hydration).
- **SC-004**: Opening a 100MB placeholder file begins streaming content to the reading application within 3 seconds, without requiring the full file to download first.
- **SC-005**: Automatic dehydration reclaims disk space within 5 minutes when the configured threshold is exceeded, without affecting files currently in use.
- **SC-006**: The FUSE daemon consumes less than 50MB of memory when idle with 10,000 tracked files.
- **SC-007**: Users can pin, unpin, hydrate, and dehydrate files via CLI commands that complete within 1 second (excluding network transfer time).
- **SC-008**: All file state transitions are correctly reflected in extended attributes, queryable by standard Linux tools (`getfattr`).
- **SC-009**: The system handles 50 concurrent file accesses without data corruption or deadlocks.
- **SC-010**: After an unclean daemon shutdown, the system recovers to a consistent state on restart — no orphaned partial downloads, no corrupted file entries.
