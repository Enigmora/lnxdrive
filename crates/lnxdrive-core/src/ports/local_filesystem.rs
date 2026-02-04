//! Local filesystem port (driven/secondary port)
//!
//! This module defines the interface for interacting with the local
//! filesystem, including reading/writing files, computing hashes,
//! and watching for changes via inotify or similar mechanisms.
//!
//! ## Design Notes
//!
//! - Uses `anyhow::Result` because filesystem errors are adapter-specific.
//! - The `IFileObserver` trait uses synchronous callbacks because file
//!   system events are delivered synchronously by the OS.
//! - `WatchHandle` is an RAII guard: dropping it stops watching.
//! - File watching is decoupled from the filesystem trait to allow
//!   different implementations (e.g., inotify on Linux, polling fallback).

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::domain::newtypes::{FileHash, SyncPath};

// ============================================================================
// T055: FileSystemState struct
// ============================================================================

/// Snapshot of a file's state on the local filesystem
///
/// Captures essential metadata about a file or directory at a point in time,
/// used for determining what has changed and whether a file is safe to modify.
#[derive(Debug, Clone)]
pub struct FileSystemState {
    /// Whether the file/directory exists on disk
    pub exists: bool,
    /// Whether this is a regular file (false for directories and other types)
    pub is_file: bool,
    /// Size in bytes (0 for directories or non-existent files)
    pub size: u64,
    /// Last modification time (None if not available or file doesn't exist)
    pub modified: Option<DateTime<Utc>>,
    /// Whether the file is currently locked by another process
    pub is_locked: bool,
}

impl FileSystemState {
    /// Returns a state representing a non-existent path
    pub fn not_found() -> Self {
        Self {
            exists: false,
            is_file: false,
            size: 0,
            modified: None,
            is_locked: false,
        }
    }

    /// Returns true if the file exists and is a regular file
    pub fn is_regular_file(&self) -> bool {
        self.exists && self.is_file
    }

    /// Returns true if the file exists and is a directory
    pub fn is_directory(&self) -> bool {
        self.exists && !self.is_file
    }
}

// ============================================================================
// T056: IFileObserver trait
// ============================================================================

/// Observer for filesystem change events
///
/// Implementations receive notifications when files within a watched
/// directory are created, modified, deleted, or renamed. These events
/// are used to detect local changes that need to be synced to the cloud.
///
/// ## Threading
///
/// Callbacks may be invoked from a background thread (e.g., the inotify
/// event loop), so implementations must be thread-safe.
pub trait IFileObserver: Send + Sync {
    /// Called when a new file or directory is created
    fn on_created(&self, path: PathBuf);

    /// Called when an existing file is modified
    fn on_modified(&self, path: PathBuf);

    /// Called when a file or directory is deleted
    fn on_deleted(&self, path: PathBuf);

    /// Called when a file or directory is renamed
    ///
    /// # Arguments
    /// * `from` - The original path before renaming
    /// * `to` - The new path after renaming
    fn on_renamed(&self, from: PathBuf, to: PathBuf);
}

// ============================================================================
// T057: WatchHandle struct
// ============================================================================

/// RAII handle for an active filesystem watch
///
/// When this handle is dropped, the associated filesystem watch is
/// automatically stopped and resources are released. This ensures
/// that watches don't leak even if the caller forgets to stop them.
///
/// ## Usage
///
/// ```ignore
/// let handle = filesystem.watch(&sync_root).await?;
/// // ... watch is active ...
/// drop(handle); // watch is stopped
/// ```
pub struct WatchHandle {
    /// Callback to invoke when the handle is dropped to stop the watch
    stop_fn: Option<Box<dyn FnOnce() + Send>>,
}

impl WatchHandle {
    /// Creates a new WatchHandle with the given stop callback
    ///
    /// The callback will be invoked exactly once when the handle is dropped.
    pub fn new(stop_fn: impl FnOnce() + Send + 'static) -> Self {
        Self {
            stop_fn: Some(Box::new(stop_fn)),
        }
    }

    /// Explicitly stops the watch, consuming the handle
    pub fn stop(mut self) {
        if let Some(stop_fn) = self.stop_fn.take() {
            stop_fn();
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        if let Some(stop_fn) = self.stop_fn.take() {
            stop_fn();
        }
    }
}

impl std::fmt::Debug for WatchHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchHandle")
            .field("active", &self.stop_fn.is_some())
            .finish()
    }
}

// ============================================================================
// T058: ILocalFileSystem trait
// ============================================================================

/// Port trait for local filesystem operations
///
/// This is the interface for all local filesystem interactions, including
/// file I/O, directory management, hash computation, and change watching.
///
/// ## Implementation Notes
///
/// - All paths are `SyncPath` instances, which are guaranteed to be absolute.
/// - `compute_hash` should produce a quickXorHash compatible with OneDrive
///   for efficient comparison of local and remote file contents.
/// - `watch` returns a `WatchHandle` that, when dropped, stops watching.
/// - Implementations should handle concurrent access gracefully.
#[async_trait::async_trait]
pub trait ILocalFileSystem: Send + Sync {
    /// Reads the entire contents of a file
    ///
    /// # Arguments
    /// * `path` - Absolute path to the file
    ///
    /// # Returns
    /// The file contents as a byte vector
    ///
    /// # Errors
    /// Returns an error if the file doesn't exist or cannot be read
    async fn read_file(&self, path: &SyncPath) -> anyhow::Result<Vec<u8>>;

    /// Writes data to a file, creating it if necessary
    ///
    /// If the file already exists, its contents are replaced.
    /// Parent directories are NOT automatically created.
    ///
    /// # Arguments
    /// * `path` - Absolute path to the file
    /// * `data` - The data to write
    async fn write_file(&self, path: &SyncPath, data: &[u8]) -> anyhow::Result<()>;

    /// Deletes a file from the filesystem
    ///
    /// # Arguments
    /// * `path` - Absolute path to the file to delete
    ///
    /// # Errors
    /// Returns an error if the file doesn't exist or cannot be deleted
    async fn delete_file(&self, path: &SyncPath) -> anyhow::Result<()>;

    /// Gets the current state of a file or directory
    ///
    /// Returns `FileSystemState::not_found()` if the path doesn't exist
    /// (does not return an error for missing paths).
    ///
    /// # Arguments
    /// * `path` - Absolute path to check
    async fn get_state(&self, path: &SyncPath) -> anyhow::Result<FileSystemState>;

    /// Computes the quickXorHash of a file
    ///
    /// The hash is compatible with OneDrive's quickXorHash algorithm
    /// for comparing local and remote file integrity.
    ///
    /// # Arguments
    /// * `path` - Absolute path to the file
    ///
    /// # Errors
    /// Returns an error if the file doesn't exist or cannot be read
    async fn compute_hash(&self, path: &SyncPath) -> anyhow::Result<FileHash>;

    /// Creates a directory and all parent directories as needed
    ///
    /// This is equivalent to `mkdir -p` behavior.
    ///
    /// # Arguments
    /// * `path` - Absolute path to the directory to create
    async fn create_directory(&self, path: &SyncPath) -> anyhow::Result<()>;

    /// Starts watching a directory for filesystem changes
    ///
    /// Returns a `WatchHandle` that stops watching when dropped.
    /// The `IFileObserver` registered with the implementation will
    /// receive change notifications.
    ///
    /// # Arguments
    /// * `path` - Absolute path to the directory to watch
    ///
    /// # Returns
    /// An RAII handle that stops the watch on drop
    async fn watch(&self, path: &SyncPath) -> anyhow::Result<WatchHandle>;
}
