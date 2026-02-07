//! LNXDrive Conflict - Conflict detection and resolution
//!
//! Provides:
//! - Hash-based conflict detection between local and remote changes
//! - Configurable resolution strategies via YAML policy rules
//! - Automatic resolution for configured file patterns
//! - Manual resolution with keep-local, keep-remote, and keep-both operations
//! - Batch resolution for multiple conflicts at once
//! - Diff tool launching for comparing versions

pub mod detector;
pub mod diff;
pub mod error;
pub mod namer;
pub mod policy;
pub mod resolver;
pub mod use_cases;

// Re-export key types for convenience
pub use detector::{ConflictDetector, DetectionResult};
pub use diff::DiffToolLauncher;
pub use error::ConflictError;
pub use namer::ConflictNamer;
pub use policy::{ConflictRule, PolicyEngine};
pub use resolver::{BatchResult, ConflictResolver};
pub use use_cases::{DetectConflictUseCase, ResolveConflictUseCase};
