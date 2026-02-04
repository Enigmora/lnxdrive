//! Domain error types
//!
//! This module defines error types specific to domain operations,
//! including validation failures, invalid state transitions, and path errors.

use thiserror::Error;

/// Errors that can occur in domain operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Invalid path format or content
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Invalid email address format
    #[error("Invalid email format: {0}")]
    InvalidEmail(String),

    /// Invalid hash format (expected quickXorHash Base64)
    #[error("Invalid hash format: {0}")]
    InvalidHash(String),

    /// Invalid state transition attempt
    #[error("Invalid state transition from {from} to {to}")]
    InvalidState {
        /// The current state
        from: String,
        /// The attempted target state
        to: String,
    },

    /// Generic validation failure
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Path is not within the configured sync root
    #[error("Path not within sync root: {0}")]
    PathNotInSyncRoot(String),

    /// Invalid remote path format
    #[error("Invalid remote path: {0}")]
    InvalidRemotePath(String),

    /// Invalid remote ID format
    #[error("Invalid remote ID: {0}")]
    InvalidRemoteId(String),

    /// Invalid delta token
    #[error("Invalid delta token: {0}")]
    InvalidDeltaToken(String),

    /// ID parsing error
    #[error("Invalid ID format: {0}")]
    InvalidId(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DomainError::InvalidPath("/bad/path".to_string());
        assert_eq!(err.to_string(), "Invalid path: /bad/path");

        let err = DomainError::InvalidEmail("notanemail".to_string());
        assert_eq!(err.to_string(), "Invalid email format: notanemail");

        let err = DomainError::InvalidState {
            from: "Pending".to_string(),
            to: "Completed".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid state transition from Pending to Completed"
        );
    }

    #[test]
    fn test_error_equality() {
        let err1 = DomainError::InvalidPath("/path".to_string());
        let err2 = DomainError::InvalidPath("/path".to_string());
        let err3 = DomainError::InvalidPath("/other".to_string());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_error_clone() {
        let err = DomainError::ValidationFailed("test".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }
}
