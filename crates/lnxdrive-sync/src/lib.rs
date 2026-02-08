//! LNXDrive Sync - Delta synchronization engine
//!
//! Provides:
//! - Incremental delta sync with Microsoft Graph
//! - Adaptive rate limiting
//! - Conflict detection
//! - Bidirectional synchronization
//!
//! ## Modules
//!
//! - [`engine`] - Bidirectional sync engine orchestrating pull/push cycles
//! - [`filesystem`] - Local filesystem adapter (atomic writes, quickXorHash)

pub mod engine;
pub mod filesystem;
pub mod scheduler;
pub mod watcher;

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur during synchronization operations
#[derive(Debug, Error)]
pub enum SyncError {
    /// An I/O error occurred during file operations
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// A file is currently locked by another process
    #[error("File locked: {0}")]
    FileLocked(PathBuf),

    /// No available disk space to complete the operation
    #[error("Disk full")]
    DiskFull,

    /// Insufficient filesystem permissions
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    /// The specified path does not exist
    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    /// A domain-level error propagated from lnxdrive-core
    #[error("Domain error: {0}")]
    DomainError(#[from] lnxdrive_core::domain::errors::DomainError),
}
