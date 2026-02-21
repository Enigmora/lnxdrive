//! LNXDrive FUSE - Files-on-Demand filesystem
//!
//! Implements a FUSE filesystem that provides:
//! - Placeholder files (sparse files with metadata)
//! - On-demand hydration when files are accessed
//! - Automatic dehydration for space management
//! - Extended attributes for file state
//!
//! # Architecture
//!
//! The FUSE filesystem is implemented as an adapter in the hexagonal architecture:
//! - [`LnxDriveFs`] implements `fuser::Filesystem` trait
//! - [`HydrationManager`] handles on-demand content downloads
//! - [`DehydrationManager`] reclaims disk space via LRU eviction
//! - [`ContentCache`] manages the local file cache
//! - [`WriteSerializer`] serializes SQLite writes to prevent SQLITE_BUSY
//!
//! # Usage
//!
//! ```ignore
//! use lnxdrive_fuse::{mount, FuseConfig};
//!
//! let config = FuseConfig::default();
//! let session = mount(config, db_pool, rt_handle)?;
//! // Filesystem is mounted until session is dropped
//! ```

// Module declarations
pub mod cache;
pub mod dehydration;
pub mod error;
pub mod filesystem;
pub mod hydration;
pub mod inode;
pub mod inode_entry;
pub mod write_serializer;
pub mod xattr;

// Public re-exports (types will be implemented in subsequent tasks)
// ---------------------------------------------------------------------------
// T040: mount() function
// T041: unmount() function
// ---------------------------------------------------------------------------
use std::{path::PathBuf, sync::Arc};

pub use cache::ContentCache;
pub use dehydration::{DehydrationManager, DehydrationPolicy, DehydrationReport};
pub use error::FuseError;
pub use filesystem::LnxDriveFs;
pub use fuser::BackgroundSession;
use fuser::MountOption;
pub use hydration::{HydrationManager, HydrationPriority, HydrationRequest};
use lnxdrive_cache::pool::DatabasePool;
use lnxdrive_core::config::FuseConfig;
use tokio::runtime::Handle;
use tracing::{debug, info};

/// Expands a tilde (~) prefix in a path to the user's home directory.
///
/// If the path starts with `~/`, it is expanded to the user's home directory.
/// Otherwise, the path is returned unchanged as a `PathBuf`.
///
/// # Arguments
///
/// * `path` - A string slice representing the path to expand
///
/// # Returns
///
/// A `PathBuf` with the tilde expanded (if present) or the original path.
///
/// # Example
///
/// ```ignore
/// let expanded = expand_tilde("~/OneDrive");
/// // Returns: /home/user/OneDrive (on Linux)
/// ```
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Mounts the LNXDrive FUSE filesystem at the configured mount point.
///
/// This function creates and mounts the FUSE filesystem in a background thread,
/// returning immediately with a handle that keeps the filesystem mounted.
///
/// # Arguments
///
/// * `config` - FUSE filesystem configuration containing mount point, cache settings, etc.
/// * `db_pool` - Database connection pool for state persistence
/// * `rt_handle` - Handle to a tokio runtime for spawning async tasks
///
/// # Returns
///
/// * `Ok(BackgroundSession)` - A handle to the mounted filesystem. The filesystem
///   remains mounted as long as this handle is kept alive. Dropping it will unmount.
/// * `Err(FuseError)` - If mounting fails (e.g., mount point doesn't exist, isn't empty,
///   or FUSE mount operation fails)
///
/// # Mount Options
///
/// The filesystem is mounted with the following options:
/// - `AutoUnmount` - Automatically unmount when the mounting process exits
/// - `FSName("lnxdrive")` - Set the filesystem name in mtab
/// - `Subtype("onedrive")` - Set the filesystem subtype in mtab
/// - `DefaultPermissions` - Let the kernel handle permission checks
/// - `NoAtime` - Don't update access time on every read (performance optimization)
/// - `Async` - Use asynchronous I/O
///
/// # Example
///
/// ```ignore
/// use lnxdrive_fuse::{mount, FuseConfig};
/// use lnxdrive_cache::pool::DatabasePool;
/// use tokio::runtime::Handle;
///
/// let config = FuseConfig::default();
/// let db_pool = DatabasePool::in_memory().await?;
/// let session = mount(config, db_pool, Handle::current())?;
/// // Filesystem is now mounted and will remain so until session is dropped
/// ```
///
/// # Errors
///
/// Returns `FuseError::NotFound` if the mount point doesn't exist.
/// Returns `FuseError::NotEmpty` if the mount point directory is not empty.
/// Returns `FuseError::IoError` if the FUSE mount operation fails.
pub fn mount(
    config: FuseConfig,
    db_pool: DatabasePool,
    rt_handle: Handle,
) -> Result<BackgroundSession, FuseError> {
    // Expand tilde in mount point path
    let mount_point = expand_tilde(&config.mount_point);

    info!(
        mount_point = %mount_point.display(),
        "Preparing to mount LNXDrive FUSE filesystem"
    );

    // Validate mount point exists
    if !mount_point.exists() {
        return Err(FuseError::NotFound(format!(
            "Mount point does not exist: {}",
            mount_point.display()
        )));
    }

    // Validate mount point is a directory
    if !mount_point.is_dir() {
        return Err(FuseError::NotADirectory(format!(
            "Mount point is not a directory: {}",
            mount_point.display()
        )));
    }

    // Validate mount point is empty
    let entries = std::fs::read_dir(&mount_point)?;
    if entries.count() > 0 {
        return Err(FuseError::NotEmpty(format!(
            "Mount point is not empty: {}",
            mount_point.display()
        )));
    }

    // Create ContentCache from cache_dir
    let cache_dir = expand_tilde(&config.cache_dir);
    debug!(cache_dir = %cache_dir.display(), "Creating content cache");

    let cache = ContentCache::new(cache_dir)?;
    let cache = Arc::new(cache);

    // Create LnxDriveFs instance
    // Note: HydrationManager is None here because mount() does not have a
    // GraphCloudProvider. The daemon should call LnxDriveFs::set_hydration_manager()
    // after mounting, or pass it via the constructor when using the full daemon setup.
    let filesystem = LnxDriveFs::new(rt_handle, db_pool, config, cache, None);

    // Configure mount options
    let mount_options = [
        MountOption::AutoUnmount,
        MountOption::FSName("lnxdrive".to_string()),
        MountOption::Subtype("onedrive".to_string()),
        MountOption::DefaultPermissions,
        MountOption::NoAtime,
        MountOption::Async,
    ];

    debug!(
        options = ?mount_options,
        "Mounting FUSE filesystem"
    );

    // Spawn the FUSE filesystem in a background thread
    let session = fuser::spawn_mount2(filesystem, &mount_point, &mount_options).map_err(|e| {
        FuseError::IoError(format!(
            "Failed to mount FUSE filesystem at {}: {}",
            mount_point.display(),
            e
        ))
    })?;

    info!(
        mount_point = %mount_point.display(),
        "LNXDrive FUSE filesystem mounted successfully"
    );

    Ok(session)
}

/// Unmounts the LNXDrive FUSE filesystem.
///
/// This function unmounts the FUSE filesystem by dropping the background session
/// handle. When the handle is dropped, it triggers the filesystem's `destroy()`
/// method and signals the kernel to unmount the filesystem.
///
/// # Arguments
///
/// * `session` - The `BackgroundSession` handle returned by [`mount()`]
///
/// # Behavior
///
/// 1. The session handle is dropped (consumed by this function)
/// 2. This triggers the filesystem's `destroy()` callback
/// 3. The kernel unmounts the filesystem
/// 4. The background thread handling FUSE operations terminates
///
/// # Example
///
/// ```ignore
/// use lnxdrive_fuse::{mount, unmount, FuseConfig};
///
/// let session = mount(config, db_pool, rt_handle)?;
/// // ... use the filesystem ...
/// unmount(session); // Filesystem is now unmounted
/// ```
///
/// # Note
///
/// This function is effectively a no-op since dropping the session does all the
/// work. It exists primarily for API clarity and to make the intent explicit.
pub fn unmount(session: BackgroundSession) {
    info!("Unmounting LNXDrive FUSE filesystem");

    // Dropping the session triggers:
    // 1. The filesystem's destroy() callback
    // 2. Kernel unmount of the filesystem
    // 3. Background thread termination
    drop(session);

    info!("LNXDrive FUSE filesystem unmounted");
}
