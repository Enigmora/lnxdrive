//! Database connection pool management
//!
//! Provides a wrapper around SQLx's SqlitePool with:
//! - Automatic directory creation for database files
//! - WAL journal mode for concurrent reads
//! - Automatic schema migration on first connection
//! - In-memory mode for testing

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::CacheError;

/// Manages a pool of SQLite connections for LNXDrive state persistence
///
/// The pool is configured with:
/// - WAL journal mode for concurrent read access
/// - 5 max connections for file-based databases
/// - 1 connection for in-memory databases (required for data persistence)
/// - 5-second busy timeout to handle write contention
pub struct DatabasePool {
    pool: SqlitePool,
}

impl DatabasePool {
    /// Creates a new database pool connected to the specified file
    ///
    /// This will:
    /// 1. Create parent directories if they don't exist
    /// 2. Create the database file if it doesn't exist
    /// 3. Enable WAL journal mode
    /// 4. Run schema migrations
    ///
    /// # Errors
    ///
    /// Returns `CacheError::ConnectionFailed` if the connection cannot be established,
    /// or `CacheError::MigrationFailed` if schema migrations fail.
    pub async fn new(db_path: &Path) -> Result<Self, CacheError> {
        // Create parent directory if needed
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CacheError::ConnectionFailed(format!(
                    "Failed to create database directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| {
                CacheError::ConnectionFailed(format!(
                    "Failed to connect to database at {}: {}",
                    db_path.display(),
                    e
                ))
            })?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        tracing::info!(
            path = %db_path.display(),
            "Database pool initialized"
        );

        Ok(Self { pool })
    }

    /// Creates an in-memory database pool for testing
    ///
    /// Uses a single connection to ensure data persistence across queries
    /// (SQLite in-memory databases are per-connection).
    ///
    /// # Errors
    ///
    /// Returns `CacheError::ConnectionFailed` if the connection cannot be established,
    /// or `CacheError::MigrationFailed` if schema migrations fail.
    pub async fn in_memory() -> Result<Self, CacheError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| {
                CacheError::ConnectionFailed(format!("Failed to create in-memory database: {}", e))
            })?;

        // Enable foreign keys for in-memory databases
        sqlx::raw_sql("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .map_err(|e| {
                CacheError::MigrationFailed(format!("Failed to enable foreign keys: {}", e))
            })?;

        Self::run_migrations(&pool).await?;

        tracing::debug!("In-memory database pool initialized");

        Ok(Self { pool })
    }

    /// Returns a reference to the underlying SQLite connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Runs the initial schema migration
    async fn run_migrations(pool: &SqlitePool) -> Result<(), CacheError> {
        let migration_sql = include_str!("migrations/20260203_initial.sql");
        sqlx::raw_sql(migration_sql)
            .execute(pool)
            .await
            .map_err(|e| {
                CacheError::MigrationFailed(format!("Failed to run initial migration: {}", e))
            })?;

        tracing::debug!("Database migrations completed");
        Ok(())
    }
}
