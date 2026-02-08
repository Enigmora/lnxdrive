//! LNXDrive Audit - Structured logging and audit trail
//!
//! Provides:
//! - `AuditLogger`: High-level service for recording audit entries
//! - `ReasonCode`: Structured reason codes for failures and conflicts
//! - Integration with `IStateRepository` for persistent audit storage

pub mod logger;
pub mod reason;

pub use logger::AuditLogger;
pub use reason::ReasonCode;
