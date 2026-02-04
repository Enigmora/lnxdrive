//! LNXDrive Cache - Local state persistence
//!
//! SQLite-based cache for:
//! - File metadata and sync state
//! - Delta tokens
//! - Account information
//! - Audit trail
//!
//! ## Architecture
//!
//! This crate implements the `IStateRepository` port from `lnxdrive-core`
//! using SQLite as the storage backend. It is a driven (secondary) adapter
//! in the hexagonal architecture.
//!
//! ## Key Components
//!
//! - [`DatabasePool`] - Connection pool with migration support
//! - [`SqliteStateRepository`] - Full `IStateRepository` implementation
//! - [`CacheError`] - Error types for cache operations
//!
//! ## Usage
//!
//! ```no_run
//! use std::path::Path;
//! use lnxdrive_cache::{DatabasePool, SqliteStateRepository};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let pool = DatabasePool::new(Path::new("/home/user/.local/share/lnxdrive/state.db")).await?;
//! let repo = SqliteStateRepository::new(pool.pool().clone());
//! // Use repo as IStateRepository...
//! # Ok(())
//! # }
//! ```

pub mod pool;
pub mod repository;

pub use pool::DatabasePool;
pub use repository::SqliteStateRepository;

/// Errors that can occur during cache operations
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// Failed to establish a database connection
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// A database query failed
    #[error("Query failed: {0}")]
    QueryFailed(String),

    /// Schema migration failed
    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    /// Serialization or deserialization of domain types failed
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<sqlx::Error> for CacheError {
    fn from(e: sqlx::Error) -> Self {
        CacheError::QueryFailed(e.to_string())
    }
}
