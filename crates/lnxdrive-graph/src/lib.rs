//! LNXDrive Graph - Microsoft Graph API client
//!
//! Provides async client for:
//! - OAuth2 authentication (Authorization Code with PKCE)
//! - OneDrive file operations via Microsoft Graph API
//! - Delta queries for efficient sync
//! - Chunked upload/download for large files
//!
//! ## Modules
//!
//! - [`auth`] - OAuth2 PKCE authentication flow components
//! - [`client`] - Microsoft Graph API HTTP client
//! - [`delta`] - Delta queries for incremental synchronization
//! - [`upload`] - File upload operations (small and large/chunked)

pub mod auth;
pub mod client;
pub mod delta;
pub mod provider;
pub mod rate_limit;
pub mod upload;

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur when communicating with the Microsoft Graph API
#[derive(Debug, Error)]
pub enum GraphError {
    /// Authentication credentials are invalid or expired
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Insufficient permissions for the requested operation
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// The requested resource does not exist
    #[error("Not found: {0}")]
    NotFound(String),

    /// A conflict was detected (e.g., concurrent modification)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Rate limit exceeded; retry after the specified duration
    #[error("Too many requests, retry after {retry_after:?}")]
    TooManyRequests {
        /// Duration to wait before retrying
        retry_after: Duration,
    },

    /// A server-side error occurred (5xx)
    #[error("Server error: {0}")]
    ServerError(String),

    /// A network-level error occurred
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// The OAuth2 token has expired and must be refreshed
    #[error("Token expired")]
    TokenExpired,

    /// The API response could not be parsed or was malformed
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
