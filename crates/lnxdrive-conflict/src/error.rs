//! Error types for the conflict engine

use thiserror::Error;

/// Errors that can occur during conflict detection and resolution
#[derive(Debug, Error)]
pub enum ConflictError {
    /// The remote version changed while attempting to resolve
    #[error("remote version changed during resolution (expected ETag {expected:?}, got {actual:?})")]
    VersionChanged {
        expected: Option<String>,
        actual: Option<String>,
    },

    /// Resolution operation failed (upload, download, or rename)
    #[error("resolution failed: {0}")]
    ResolutionFailed(String),

    /// Diff tool not found on the system
    #[error("diff tool not found: {0}")]
    DiffToolNotFound(String),

    /// Conflict not found in repository
    #[error("conflict not found: {0}")]
    NotFound(String),

    /// Conflict already resolved
    #[error("conflict already resolved: {0}")]
    AlreadyResolved(String),

    /// Invalid glob pattern in conflict rule
    #[error("invalid glob pattern: {pattern}: {reason}")]
    InvalidPattern { pattern: String, reason: String },

    /// Storage error
    #[error("storage error: {0}")]
    Storage(#[from] anyhow::Error),
}
