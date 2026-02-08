//! LNXDrive IPC - D-Bus communication library
//!
//! Provides high-level async API for UI clients to communicate
//! with the LNXDrive daemon via D-Bus session bus.
//!
//! # Interfaces
//! - `com.enigmora.LNXDrive.SyncController` - Sync control (legacy)
//! - `com.enigmora.LNXDrive.Account` - Account management (legacy)
//! - `com.enigmora.LNXDrive.Conflicts` - Conflict resolution
//! - `com.enigmora.LNXDrive.Files` - File status queries, pin/unpin, sync-by-path
//! - `com.enigmora.LNXDrive.Sync` - Global sync control with properties
//! - `com.enigmora.LNXDrive.Status` - Account and quota information
//! - `com.enigmora.LNXDrive.Auth` - OAuth2 authentication
//! - `com.enigmora.LNXDrive.Settings` - Configuration management
//! - `com.enigmora.LNXDrive.Manager` - Daemon lifecycle
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
    AccountInterface, AuthInterface, ConflictsInterface, DaemonState, DaemonSyncState,
    DbusService, FilesInterface, ManagerInterface, SettingsInterface, StatusInterface,
    SyncControllerInterface, SyncInterface, DBUS_NAME, DBUS_PATH,
};
