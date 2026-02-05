//! Audit entry domain entities
//!
//! This module defines the core audit types for tracking all significant
//! operations in LNXDrive, enabling transparency and debugging capabilities.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::newtypes::{AuditId, SessionId, UniqueId};

/// Actions that can be recorded in the audit log
///
/// Each action represents a significant operation that should be tracked
/// for transparency, debugging, and user explanation purposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// User authenticated with OneDrive
    AuthLogin,
    /// User logged out
    AuthLogout,
    /// OAuth token was refreshed
    AuthRefresh,
    /// Synchronization cycle started
    SyncStart,
    /// Synchronization cycle completed
    SyncComplete,
    /// File was uploaded to OneDrive
    FileUpload,
    /// File was downloaded from OneDrive
    FileDownload,
    /// File was deleted (locally or remotely)
    FileDelete,
    /// A sync conflict was detected
    ConflictDetected,
    /// A conflict was resolved
    ConflictResolved,
    /// An error occurred
    Error,
    /// Configuration was changed
    ConfigChange,
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AuditAction::AuthLogin => "auth_login",
            AuditAction::AuthLogout => "auth_logout",
            AuditAction::AuthRefresh => "auth_refresh",
            AuditAction::SyncStart => "sync_start",
            AuditAction::SyncComplete => "sync_complete",
            AuditAction::FileUpload => "file_upload",
            AuditAction::FileDownload => "file_download",
            AuditAction::FileDelete => "file_delete",
            AuditAction::ConflictDetected => "conflict_detected",
            AuditAction::ConflictResolved => "conflict_resolved",
            AuditAction::Error => "error",
            AuditAction::ConfigChange => "config_change",
        };
        write!(f, "{}", s)
    }
}

/// Result of an audited action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    /// The action completed successfully
    Success,
    /// The action failed with an error code and message
    Failed {
        /// Error code for categorization
        code: String,
        /// Human-readable error message
        message: String,
    },
}

impl AuditResult {
    /// Creates a successful result
    pub fn success() -> Self {
        AuditResult::Success
    }

    /// Creates a failed result with the given code and message
    pub fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        AuditResult::Failed {
            code: code.into(),
            message: message.into(),
        }
    }

    /// Returns true if the result is a success
    pub fn is_success(&self) -> bool {
        matches!(self, AuditResult::Success)
    }

    /// Returns true if the result is a failure
    pub fn is_failed(&self) -> bool {
        matches!(self, AuditResult::Failed { .. })
    }
}

/// An audit log entry recording a significant operation
///
/// AuditEntry captures all relevant information about an operation
/// for later querying, analysis, and user explanation (via `lnxdrive explain`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for this audit entry (assigned by database)
    id: Option<AuditId>,
    /// When the action occurred
    timestamp: DateTime<Utc>,
    /// Session ID if action is associated with a user session
    session_id: Option<SessionId>,
    /// Item ID if action is associated with a specific file/folder
    item_id: Option<UniqueId>,
    /// The type of action that was performed
    action: AuditAction,
    /// The result of the action
    result: AuditResult,
    /// Additional structured details about the action
    details: Value,
    /// How long the action took in milliseconds
    duration_ms: Option<u64>,
}

impl AuditEntry {
    /// Creates a new audit entry with the required fields
    ///
    /// The `id` field is set to `None` and will be assigned by the database
    /// when the entry is persisted.
    ///
    /// # Arguments
    ///
    /// * `action` - The type of action being recorded
    /// * `result` - The outcome of the action
    ///
    /// # Example
    ///
    /// ```
    /// use lnxdrive_core::domain::audit::{AuditEntry, AuditAction, AuditResult};
    ///
    /// let entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success());
    /// assert!(entry.result().is_success());
    /// assert!(entry.id().is_none()); // ID assigned on persist
    /// ```
    pub fn new(action: AuditAction, result: AuditResult) -> Self {
        Self {
            id: None,
            timestamp: Utc::now(),
            session_id: None,
            item_id: None,
            action,
            result,
            details: Value::Null,
            duration_ms: None,
        }
    }

    /// Returns the audit entry ID (None if not yet persisted)
    pub fn id(&self) -> Option<AuditId> {
        self.id
    }

    /// Sets the ID for this audit entry (typically called after database insert)
    pub fn with_id(mut self, id: AuditId) -> Self {
        self.id = Some(id);
        self
    }

    /// Returns when the action occurred
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Returns the session ID if present
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Returns the item ID if present
    pub fn item_id(&self) -> Option<&UniqueId> {
        self.item_id.as_ref()
    }

    /// Returns the action type
    pub fn action(&self) -> &AuditAction {
        &self.action
    }

    /// Returns the action result
    pub fn result(&self) -> &AuditResult {
        &self.result
    }

    /// Returns the additional details
    pub fn details(&self) -> &Value {
        &self.details
    }

    /// Returns the duration in milliseconds if recorded
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Sets the session ID for this audit entry
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Sets the item ID for this audit entry
    pub fn with_item_id(mut self, item_id: UniqueId) -> Self {
        self.item_id = Some(item_id);
        self
    }

    /// Sets additional details for this audit entry
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    /// Sets the duration in milliseconds for this audit entry
    pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_audit_action_serialization() {
        let action = AuditAction::FileUpload;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"file_upload\"");

        let deserialized: AuditAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::AuthLogin.to_string(), "auth_login");
        assert_eq!(AuditAction::SyncComplete.to_string(), "sync_complete");
        assert_eq!(AuditAction::FileDownload.to_string(), "file_download");
    }

    #[test]
    fn test_audit_result_success() {
        let result = AuditResult::success();
        assert!(result.is_success());
        assert!(!result.is_failed());
    }

    #[test]
    fn test_audit_result_failed() {
        let result = AuditResult::failed("E001", "Network error");
        assert!(!result.is_success());
        assert!(result.is_failed());

        if let AuditResult::Failed { code, message } = result {
            assert_eq!(code, "E001");
            assert_eq!(message, "Network error");
        } else {
            panic!("Expected Failed variant");
        }
    }

    #[test]
    fn test_audit_result_serialization() {
        let success = AuditResult::success();
        let json = serde_json::to_string(&success).unwrap();
        assert_eq!(json, "\"success\"");

        let failed = AuditResult::failed("E001", "Error message");
        let json = serde_json::to_string(&failed).unwrap();
        let deserialized: AuditResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, failed);
    }

    #[test]
    fn test_audit_entry_new() {
        let entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success());

        assert!(entry.id().is_none()); // ID not assigned until persisted
        assert_eq!(*entry.action(), AuditAction::SyncStart);
        assert!(entry.result().is_success());
        assert!(entry.session_id().is_none());
        assert!(entry.item_id().is_none());
        assert_eq!(*entry.details(), Value::Null);
        assert!(entry.duration_ms().is_none());
    }

    #[test]
    fn test_audit_entry_with_id() {
        let entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success())
            .with_id(AuditId::new(42));

        assert_eq!(entry.id(), Some(AuditId::new(42)));
    }

    #[test]
    fn test_audit_entry_builder_pattern() {
        let session_id = SessionId::new();
        let item_id = UniqueId::new();
        let details = json!({"file": "test.txt", "size": 1024});

        let entry = AuditEntry::new(AuditAction::FileUpload, AuditResult::success())
            .with_session_id(session_id)
            .with_item_id(item_id)
            .with_details(details.clone())
            .with_duration_ms(150);

        assert_eq!(entry.session_id(), Some(&session_id));
        assert_eq!(entry.item_id(), Some(&item_id));
        assert_eq!(*entry.details(), details);
        assert_eq!(entry.duration_ms(), Some(150));
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry::new(AuditAction::FileDownload, AuditResult::success())
            .with_details(json!({"path": "/documents/file.pdf"}))
            .with_duration_ms(500);

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.action(), entry.action());
        assert_eq!(deserialized.result(), entry.result());
        assert_eq!(deserialized.details(), entry.details());
        assert_eq!(deserialized.duration_ms(), entry.duration_ms());
    }

    #[test]
    fn test_audit_entry_error_action() {
        let entry = AuditEntry::new(
            AuditAction::Error,
            AuditResult::failed("NETWORK_ERROR", "Connection timed out"),
        )
        .with_details(json!({
            "endpoint": "https://graph.microsoft.com/v1.0/me/drive",
            "retry_count": 3
        }));

        assert_eq!(*entry.action(), AuditAction::Error);
        assert!(entry.result().is_failed());
    }
}
