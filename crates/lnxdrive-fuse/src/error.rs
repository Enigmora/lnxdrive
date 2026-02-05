//! Error types for the FUSE filesystem.
//!
//! Defines `FuseError` and conversions to libc errno values.

use thiserror::Error;

/// Errors that can occur in the FUSE filesystem.
#[derive(Error, Debug)]
pub enum FuseError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("directory not empty: {0}")]
    NotEmpty(String),

    #[error("I/O error: {0}")]
    IoError(String),

    #[error("not a directory: {0}")]
    NotADirectory(String),

    #[error("is a directory: {0}")]
    IsADirectory(String),

    #[error("disk full: {0}")]
    DiskFull(String),

    #[error("extended attribute not found: {0}")]
    XattrNotFound(String),

    #[error("buffer too small for extended attribute")]
    XattrBufferTooSmall,

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("name too long: {0}")]
    NameTooLong(String),

    #[error("hydration failed: {0}")]
    HydrationFailed(String),

    #[error("cache error: {0}")]
    CacheError(String),

    #[error("database error: {0}")]
    DatabaseError(String),
}

impl From<FuseError> for libc::c_int {
    fn from(err: FuseError) -> libc::c_int {
        match err {
            FuseError::NotFound(_) => libc::ENOENT,
            FuseError::PermissionDenied(_) => libc::EACCES,
            FuseError::AlreadyExists(_) => libc::EEXIST,
            FuseError::NotEmpty(_) => libc::ENOTEMPTY,
            FuseError::IoError(_) => libc::EIO,
            FuseError::NotADirectory(_) => libc::ENOTDIR,
            FuseError::IsADirectory(_) => libc::EISDIR,
            FuseError::DiskFull(_) => libc::ENOSPC,
            FuseError::XattrNotFound(_) => libc::ENODATA,
            FuseError::XattrBufferTooSmall => libc::ERANGE,
            FuseError::InvalidArgument(_) => libc::EINVAL,
            FuseError::NameTooLong(_) => libc::ENAMETOOLONG,
            FuseError::HydrationFailed(_) => libc::EIO,
            FuseError::CacheError(_) => libc::EIO,
            FuseError::DatabaseError(_) => libc::EIO,
        }
    }
}

impl From<std::io::Error> for FuseError {
    fn from(err: std::io::Error) -> Self {
        FuseError::IoError(err.to_string())
    }
}

impl From<anyhow::Error> for FuseError {
    fn from(err: anyhow::Error) -> Self {
        FuseError::IoError(err.to_string())
    }
}
