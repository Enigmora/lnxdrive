//! Non-fatal error report generation
//!
//! Captures recurring or unexpected errors for optional submission.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A structured error report (non-fatal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    pub id: String,
    pub timestamp: String,
    pub version: String,
    pub error_type: String,
    pub message: String,
    pub context: String,
    pub chain: Vec<String>,
}

impl ErrorReport {
    /// Create a new error report.
    pub fn new(error_type: &str, message: &str, context: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            error_type: error_type.to_string(),
            message: message.to_string(),
            context: context.to_string(),
            chain: Vec::new(),
        }
    }

    /// Add an error chain entry.
    pub fn with_chain(mut self, chain: Vec<String>) -> Self {
        self.chain = chain;
        self
    }
}

/// Filters which errors are worth reporting.
pub struct ErrorReporter;

impl ErrorReporter {
    /// Determines whether an error should be captured as a report.
    ///
    /// Excludes transient/expected errors like timeouts, connection refused,
    /// and rate limiting.
    pub fn should_report(error_msg: &str) -> bool {
        let lower = error_msg.to_lowercase();

        // Exclude transient/expected errors
        if lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("connection refused")
            || lower.contains("rate limit")
            || lower.contains("too many requests")
            || lower.contains("429")
        {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_report_creation() {
        let report = ErrorReport::new("IOError", "file not found", "sync_engine");
        assert!(!report.id.is_empty());
        assert_eq!(report.error_type, "IOError");
        assert_eq!(report.message, "file not found");
    }

    #[test]
    fn test_error_report_with_chain() {
        let report = ErrorReport::new("IOError", "read failed", "download")
            .with_chain(vec!["caused by: permission denied".to_string()]);
        assert_eq!(report.chain.len(), 1);
    }

    #[test]
    fn test_should_report_filters_transient() {
        assert!(!ErrorReporter::should_report("Connection timed out"));
        assert!(!ErrorReporter::should_report("connection refused"));
        assert!(!ErrorReporter::should_report("Rate limit exceeded"));
        assert!(!ErrorReporter::should_report("HTTP 429 Too Many Requests"));
    }

    #[test]
    fn test_should_report_accepts_real_errors() {
        assert!(ErrorReporter::should_report("database corrupted"));
        assert!(ErrorReporter::should_report("unexpected null pointer"));
        assert!(ErrorReporter::should_report("schema migration failed"));
    }
}
