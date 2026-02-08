//! Inode entry representation for the FUSE filesystem.
//!
//! Contains file metadata and state for FUSE operations.

use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

use lnxdrive_core::domain::{ItemState, RemoteId, UniqueId};

/// A newtype wrapper for FUSE inode numbers.
///
/// Provides type safety to prevent accidental mixing of raw u64 values
/// with inode identifiers. Satisfies Constitution Principle II (Idiomatic Rust).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InodeNumber(u64);

impl InodeNumber {
    /// Root inode number (always 1 per FUSE convention)
    pub const ROOT: InodeNumber = InodeNumber(1);

    /// Create a new inode number
    pub fn new(val: u64) -> Self {
        InodeNumber(val)
    }

    /// Get the raw u64 value
    pub fn get(&self) -> u64 {
        self.0
    }
}

impl From<u64> for InodeNumber {
    fn from(val: u64) -> Self {
        InodeNumber(val)
    }
}

impl From<InodeNumber> for u64 {
    fn from(ino: InodeNumber) -> Self {
        ino.0
    }
}

impl fmt::Display for InodeNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// T026: InodeEntry struct
// ============================================================================

/// In-memory representation of a FUSE inode.
///
/// Contains all metadata needed for FUSE operations, including reference
/// counting for kernel lookups and open file handles. This struct is designed
/// for fast lookups in the inode table without requiring database queries.
///
/// ## Reference Counting
///
/// - `lookup_count`: Tracks FUSE kernel references via `lookup()`/`forget()`
/// - `open_handles`: Tracks open file handles via `open()`/`release()`
///
/// An entry is eligible for eviction from the in-memory table when both
/// counters reach zero (see `is_expired()`).
///
/// ## Atomic Operations
///
/// The reference counters use `AtomicU64` to allow lock-free increment/decrement
/// from concurrent FUSE operations, satisfying the zero-GC requirement for
/// predictable latency.
#[derive(Debug)]
pub struct InodeEntry {
    /// FUSE inode number (unique within this filesystem instance)
    pub ino: InodeNumber,

    /// Reference to the SyncItem in the database
    pub item_id: UniqueId,

    /// OneDrive item ID (None for newly created local items)
    pub remote_id: Option<RemoteId>,

    /// Parent directory inode (ROOT for top-level items)
    pub parent_ino: InodeNumber,

    /// Entry name in parent directory
    pub name: String,

    /// File type (Regular file or Directory)
    pub kind: fuser::FileType,

    /// File size in bytes (real size from cloud, not local cache)
    pub size: u64,

    /// Unix permissions (e.g., 0o644 for files, 0o755 for directories)
    pub perm: u16,

    /// Last modification time
    pub mtime: SystemTime,

    /// Last metadata change time
    pub ctime: SystemTime,

    /// Last access time
    pub atime: SystemTime,

    /// Number of hard links (always 1 for OneDrive files)
    pub nlink: u32,

    /// Kernel reference count (incremented by lookup, decremented by forget)
    lookup_count: AtomicU64,

    /// Number of open file handles
    open_handles: AtomicU64,

    /// Current sync/hydration state
    pub state: ItemState,
}

impl InodeEntry {
    /// Creates a new inode entry.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    /// * `item_id` - Reference to the SyncItem
    /// * `remote_id` - OneDrive item ID (None for new local items)
    /// * `parent_ino` - Parent directory inode
    /// * `name` - Entry name
    /// * `kind` - File type (Regular or Directory)
    /// * `size` - File size in bytes
    /// * `perm` - Unix permissions
    /// * `mtime` - Last modification time
    /// * `ctime` - Last metadata change time
    /// * `atime` - Last access time
    /// * `nlink` - Number of hard links
    /// * `state` - Current sync/hydration state
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ino: InodeNumber,
        item_id: UniqueId,
        remote_id: Option<RemoteId>,
        parent_ino: InodeNumber,
        name: String,
        kind: fuser::FileType,
        size: u64,
        perm: u16,
        mtime: SystemTime,
        ctime: SystemTime,
        atime: SystemTime,
        nlink: u32,
        state: ItemState,
    ) -> Self {
        Self {
            ino,
            item_id,
            remote_id,
            parent_ino,
            name,
            kind,
            size,
            perm,
            mtime,
            ctime,
            atime,
            nlink,
            lookup_count: AtomicU64::new(0),
            open_handles: AtomicU64::new(0),
            state,
        }
    }

    /// Converts this inode entry to a FUSE FileAttr structure.
    ///
    /// This is used to respond to `getattr()` and `lookup()` calls.
    pub fn to_file_attr(&self) -> fuser::FileAttr {
        fuser::FileAttr {
            ino: self.ino.get(),
            size: self.size,
            blocks: self.size.div_ceil(512), // Round up to 512-byte blocks
            atime: self.atime,
            mtime: self.mtime,
            ctime: self.ctime,
            crtime: self.ctime, // Creation time = metadata change time
            kind: self.kind,
            perm: self.perm,
            nlink: self.nlink,
            uid: unsafe { libc::getuid() }, // Current user
            gid: unsafe { libc::getgid() }, // Current group
            rdev: 0,                        // Not a device file
            blksize: 4096,                  // Standard block size
            flags: 0,                       // No special flags
        }
    }

    /// Atomically increments the lookup count.
    ///
    /// Called when the kernel issues a `lookup()` operation.
    pub fn increment_lookup(&self) {
        self.lookup_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Atomically decrements the lookup count and returns the new value.
    ///
    /// Called when the kernel issues a `forget()` operation.
    ///
    /// # Returns
    ///
    /// The new lookup count after decrementing.
    pub fn decrement_lookup(&self) -> u64 {
        self.lookup_count.fetch_sub(1, Ordering::SeqCst) - 1
    }

    /// Atomically decrements the lookup count by a specified amount.
    ///
    /// Called when the kernel issues a `forget()` operation with nlookup > 1.
    ///
    /// # Arguments
    ///
    /// * `count` - The number of lookups to decrement
    ///
    /// # Returns
    ///
    /// The new lookup count after decrementing.
    pub fn decrement_lookup_by(&self, count: u64) -> u64 {
        self.lookup_count.fetch_sub(count, Ordering::SeqCst) - count
    }

    /// Atomically increments the open handles count.
    ///
    /// Called when a file is opened via `open()` or `opendir()`.
    pub fn increment_open_handles(&self) {
        self.open_handles.fetch_add(1, Ordering::SeqCst);
    }

    /// Atomically decrements the open handles count and returns the new value.
    ///
    /// Called when a file is closed via `release()` or `releasedir()`.
    ///
    /// # Returns
    ///
    /// The new open handles count after decrementing.
    pub fn decrement_open_handles(&self) -> u64 {
        self.open_handles.fetch_sub(1, Ordering::SeqCst) - 1
    }

    /// Returns true if this entry is eligible for eviction from memory.
    ///
    /// An entry can be evicted when:
    /// - No kernel references exist (lookup_count == 0)
    /// - No open file handles exist (open_handles == 0)
    ///
    /// This follows the FUSE kernel contract: entries must be retained
    /// while they have active references or open handles.
    pub fn is_expired(&self) -> bool {
        self.lookup_count.load(Ordering::SeqCst) == 0
            && self.open_handles.load(Ordering::SeqCst) == 0
    }

    // ========================================================================
    // Getters
    // ========================================================================

    /// Returns the inode number.
    pub fn ino(&self) -> InodeNumber {
        self.ino
    }

    /// Returns the SyncItem reference.
    pub fn item_id(&self) -> &UniqueId {
        &self.item_id
    }

    /// Returns the OneDrive remote ID, if available.
    pub fn remote_id(&self) -> Option<&RemoteId> {
        self.remote_id.as_ref()
    }

    /// Returns the parent inode number.
    pub fn parent_ino(&self) -> InodeNumber {
        self.parent_ino
    }

    /// Returns the entry name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the file type.
    pub fn kind(&self) -> fuser::FileType {
        self.kind
    }

    /// Returns the file size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns the Unix permissions.
    pub fn perm(&self) -> u16 {
        self.perm
    }

    /// Returns the last modification time.
    pub fn mtime(&self) -> SystemTime {
        self.mtime
    }

    /// Returns the last metadata change time.
    pub fn ctime(&self) -> SystemTime {
        self.ctime
    }

    /// Returns the last access time.
    pub fn atime(&self) -> SystemTime {
        self.atime
    }

    /// Returns the number of hard links.
    pub fn nlink(&self) -> u32 {
        self.nlink
    }

    /// Returns the current lookup count.
    pub fn lookup_count(&self) -> u64 {
        self.lookup_count.load(Ordering::SeqCst)
    }

    /// Returns the current open handles count.
    pub fn open_handles(&self) -> u64 {
        self.open_handles.load(Ordering::SeqCst)
    }

    /// Returns the current sync/hydration state.
    pub fn state(&self) -> &ItemState {
        &self.state
    }
}
