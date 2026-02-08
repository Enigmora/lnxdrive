//! Reason codes for audit log entries
//!
//! Provides structured codes for categorizing why an operation failed or
//! why a conflict occurred. Used by `AuditLogger` to enrich audit entries.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Structured reason codes for failures and conflicts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// Remote file was modified while an upload was in progress
    RemoteModifiedDuringUpload,
    /// Local file was modified while a download was in progress
    LocalModifiedDuringDownload,
    /// Network operation timed out
    NetworkTimeout,
    /// API rate limit / throttling was exceeded
    ThrottlingExceeded,
    /// Insufficient permissions to perform the operation
    PermissionDenied,
    /// File exceeds the maximum allowed size
    FileTooLarge,
    /// File path exceeds the maximum allowed length
    PathTooLong,
}

impl fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ReasonCode::RemoteModifiedDuringUpload => "remote_modified_during_upload",
            ReasonCode::LocalModifiedDuringDownload => "local_modified_during_download",
            ReasonCode::NetworkTimeout => "network_timeout",
            ReasonCode::ThrottlingExceeded => "throttling_exceeded",
            ReasonCode::PermissionDenied => "permission_denied",
            ReasonCode::FileTooLarge => "file_too_large",
            ReasonCode::PathTooLong => "path_too_long",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_code_display() {
        assert_eq!(
            ReasonCode::RemoteModifiedDuringUpload.to_string(),
            "remote_modified_during_upload"
        );
        assert_eq!(ReasonCode::NetworkTimeout.to_string(), "network_timeout");
        assert_eq!(ReasonCode::FileTooLarge.to_string(), "file_too_large");
        assert_eq!(ReasonCode::PathTooLong.to_string(), "path_too_long");
    }

    #[test]
    fn reason_code_serialization() {
        let code = ReasonCode::ThrottlingExceeded;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"throttling_exceeded\"");

        let deserialized: ReasonCode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, code);
    }
}
