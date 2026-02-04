//! LNXDrive IPC - D-Bus communication library
//!
//! Provides high-level async API for UI clients to communicate
//! with the LNXDrive daemon via D-Bus session bus.
//!
//! # Interfaces
//! - `com.enigmora.LNXDrive.SyncController` - Sync control
//! - `com.enigmora.LNXDrive.Account` - Account management
//! - `com.enigmora.LNXDrive.Conflicts` - Conflict resolution
//!
//! # Usage
//!
//! The [`DbusService`] type is the main entry point. It manages the
//! D-Bus connection lifecycle and registers all interface implementations.
//!
//! ```rust,no_run
//! use lnxdrive_ipc::service::DbusService;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let service = DbusService::with_default_state();
//! let _connection = service.start().await?;
//! // Service is now active on the session bus
//! # Ok(())
//! # }
//! ```

pub mod service;

pub use service::{
    AccountInterface, ConflictsInterface, DaemonState, DaemonSyncState, DbusService,
    SyncControllerInterface, DBUS_NAME, DBUS_PATH,
};
