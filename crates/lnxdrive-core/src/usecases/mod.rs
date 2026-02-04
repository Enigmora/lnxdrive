//! Use cases (interactors) for LNXDrive
//!
//! This module contains the application use cases that orchestrate
//! domain entities and port interfaces. Use cases are thin coordinators
//! that delegate business rules to domain methods and I/O to ports.
//!
//! ## Use Cases
//!
//! - [`AuthenticateUseCase`] - OAuth2 authentication flow, token refresh, logout
//! - [`SyncFileUseCase`] - Single file upload/download synchronization
//! - [`QueryDeltaUseCase`] - Incremental delta queries from OneDrive
//! - [`ExplainFailureUseCase`] - Human-readable failure explanations

pub mod authenticate;
pub mod explain_failure;
pub mod query_delta;
pub mod sync_file;

pub use authenticate::AuthenticateUseCase;
pub use explain_failure::ExplainFailureUseCase;
pub use query_delta::QueryDeltaUseCase;
pub use sync_file::SyncFileUseCase;
