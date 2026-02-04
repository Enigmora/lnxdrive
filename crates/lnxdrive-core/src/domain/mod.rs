//! Domain entities and business logic
//!
//! This module contains the core domain types for LNXDrive:
//! - Newtypes for type-safe identifiers and validated domain types
//! - Account management types
//! - Audit entries for tracking operations
//! - Conflict detection and resolution types
//! - Session management types
//! - Sync item types
//! - Domain-specific error types

pub mod account;
pub mod audit;
pub mod conflict;
pub mod errors;
pub mod newtypes;
pub mod session;
pub mod sync_item;

// Re-export commonly used types
pub use account::{Account, AccountState};
pub use audit::{AuditAction, AuditEntry, AuditResult};
pub use conflict::{Conflict, Resolution, ResolutionSource, VersionInfo};
pub use errors::DomainError;
pub use newtypes::*;
pub use session::{SessionError, SessionStatus, SyncSession};
pub use sync_item::{ErrorInfo, ItemMetadata, ItemState, Permissions, SyncItem};
