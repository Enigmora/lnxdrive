//! LNXDrive IPC - D-Bus communication library
//!
//! Provides high-level async API for UI clients to communicate
//! with the LNXDrive daemon via D-Bus session bus.
//!
//! # Interfaces
//! - `org.enigmora.LNXDrive.Manager` - Daemon control
//! - `org.enigmora.LNXDrive.Sync` - Synchronization operations
//! - `org.enigmora.LNXDrive.Files` - File operations
//! - `org.enigmora.LNXDrive.Status` - Status information
//! - `org.enigmora.LNXDrive.Settings` - Configuration
//! - `org.enigmora.LNXDrive.Accounts` - Multi-account management

pub mod client;
pub mod types;
