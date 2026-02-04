//! SyncSession domain entity
//!
//! This module defines the SyncSession entity which tracks the state
//! and progress of a synchronization operation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::newtypes::{AccountId, DeltaToken, SessionId, UniqueId};

/// T034: Status of a sync session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is currently running
    Running,
    /// Session completed successfully
    Completed,
    /// Session failed with an error message
    Failed(String),
    /// Session was cancelled by user or system
    Cancelled,
}

impl SessionStatus {
    /// Returns true if the session is still in progress
    pub fn is_running(&self) -> bool {
        matches!(self, SessionStatus::Running)
    }

    /// Returns true if the session has finished (successfully or not)
    pub fn is_finished(&self) -> bool {
        !self.is_running()
    }

    /// Returns true if the session completed successfully
    pub fn is_success(&self) -> bool {
        matches!(self, SessionStatus::Completed)
    }

    /// Returns true if the session failed
    pub fn is_failed(&self) -> bool {
        matches!(self, SessionStatus::Failed(_))
    }
}

impl Default for SessionStatus {
    fn default() -> Self {
        SessionStatus::Running
    }
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Running => write!(f, "running"),
            SessionStatus::Completed => write!(f, "completed"),
            SessionStatus::Failed(msg) => write!(f, "failed: {}", msg),
            SessionStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// T035: Error that occurred during a sync session for a specific item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionError {
    /// ID of the item that failed
    item_id: UniqueId,
    /// Error code for categorization
    error_code: String,
    /// Human-readable error message
    message: String,
    /// When the error occurred
    timestamp: DateTime<Utc>,
}

impl SessionError {
    /// Creates a new SessionError
    pub fn new(
        item_id: UniqueId,
        error_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            item_id,
            error_code: error_code.into(),
            message: message.into(),
            timestamp: Utc::now(),
        }
    }

    /// Creates a SessionError with a specific timestamp
    pub fn with_timestamp(
        item_id: UniqueId,
        error_code: impl Into<String>,
        message: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            item_id,
            error_code: error_code.into(),
            message: message.into(),
            timestamp,
        }
    }

    /// Returns the item ID that failed
    pub fn item_id(&self) -> &UniqueId {
        &self.item_id
    }

    /// Returns the error code
    pub fn error_code(&self) -> &str {
        &self.error_code
    }

    /// Returns the error message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns when the error occurred
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] Item {}: {} - {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.item_id,
            self.error_code,
            self.message
        )
    }
}

/// T036: Represents a synchronization session
///
/// A SyncSession tracks the progress and outcome of a synchronization
/// operation for an account. It records items processed, success/failure
/// counts, and any errors encountered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncSession {
    /// Unique identifier for this session
    id: SessionId,
    /// Account being synchronized
    account_id: AccountId,
    /// When the session started
    started_at: DateTime<Utc>,
    /// When the session completed (None if still running)
    completed_at: Option<DateTime<Utc>>,
    /// Current status of the session
    status: SessionStatus,
    /// Total items to process
    items_total: u64,
    /// Items processed so far
    items_processed: u64,
    /// Items that succeeded
    items_succeeded: u64,
    /// Items that failed
    items_failed: u64,
    /// Bytes uploaded during this session
    bytes_uploaded: u64,
    /// Bytes downloaded during this session
    bytes_downloaded: u64,
    /// Delta token at session start (for resumability)
    delta_token_start: Option<DeltaToken>,
    /// Delta token at session end (for next sync)
    delta_token_end: Option<DeltaToken>,
    /// T171: Total items received from the delta query
    items_checked: u64,
    /// T171: Items that actually required action (not skipped)
    items_synced: u64,
    /// Errors encountered during the session
    errors: Vec<SessionError>,
}

impl SyncSession {
    /// T037: Creates a new SyncSession for an account
    ///
    /// # Arguments
    /// * `account_id` - The account being synchronized
    ///
    /// # Returns
    /// A new SyncSession in Running state with zero counters
    pub fn new(account_id: AccountId) -> Self {
        Self {
            id: SessionId::new(),
            account_id,
            started_at: Utc::now(),
            completed_at: None,
            status: SessionStatus::Running,
            items_total: 0,
            items_processed: 0,
            items_succeeded: 0,
            items_failed: 0,
            bytes_uploaded: 0,
            bytes_downloaded: 0,
            delta_token_start: None,
            delta_token_end: None,
            items_checked: 0,
            items_synced: 0,
            errors: Vec::new(),
        }
    }

    /// Creates a SyncSession with a specific ID and start time (for reconstitution)
    pub fn with_id(id: SessionId, account_id: AccountId, started_at: DateTime<Utc>) -> Self {
        Self {
            id,
            account_id,
            started_at,
            completed_at: None,
            status: SessionStatus::Running,
            items_total: 0,
            items_processed: 0,
            items_succeeded: 0,
            items_failed: 0,
            bytes_uploaded: 0,
            bytes_downloaded: 0,
            delta_token_start: None,
            delta_token_end: None,
            items_checked: 0,
            items_synced: 0,
            errors: Vec::new(),
        }
    }

    // --- Getters ---

    /// Returns the session's unique identifier
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Returns the account ID being synchronized
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    /// Returns when the session started
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Returns when the session completed, if finished
    pub fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }

    /// Returns the current session status
    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Returns the total number of items to process
    pub fn items_total(&self) -> u64 {
        self.items_total
    }

    /// Returns the number of items processed
    pub fn items_processed(&self) -> u64 {
        self.items_processed
    }

    /// Returns the number of items that succeeded
    pub fn items_succeeded(&self) -> u64 {
        self.items_succeeded
    }

    /// Returns the number of items that failed
    pub fn items_failed(&self) -> u64 {
        self.items_failed
    }

    /// Returns bytes uploaded during this session
    pub fn bytes_uploaded(&self) -> u64 {
        self.bytes_uploaded
    }

    /// Returns bytes downloaded during this session
    pub fn bytes_downloaded(&self) -> u64 {
        self.bytes_downloaded
    }

    /// Returns the delta token at session start
    pub fn delta_token_start(&self) -> Option<&DeltaToken> {
        self.delta_token_start.as_ref()
    }

    /// Returns the delta token at session end
    pub fn delta_token_end(&self) -> Option<&DeltaToken> {
        self.delta_token_end.as_ref()
    }

    /// T171: Returns the total items received from the delta query
    pub fn items_checked(&self) -> u64 {
        self.items_checked
    }

    /// T171: Returns items that actually required action (not skipped)
    pub fn items_synced(&self) -> u64 {
        self.items_synced
    }

    /// Returns all errors encountered during the session
    pub fn errors(&self) -> &[SessionError] {
        &self.errors
    }

    // --- Computed Properties ---

    /// T171: Returns the ratio of items that required action vs total items checked
    ///
    /// A value of 1.0 means every delta item required action (first sync),
    /// while a value close to 0.0 means the delta was very efficient (most
    /// items were unchanged/skipped).
    ///
    /// Returns 0.0 if no items were checked (division by zero guard).
    pub fn sync_efficiency(&self) -> f64 {
        if self.items_checked == 0 {
            return 0.0;
        }
        self.items_synced as f64 / self.items_checked as f64
    }

    /// Returns the progress as a percentage (0.0 to 100.0)
    pub fn progress_percent(&self) -> f64 {
        if self.items_total == 0 {
            return if self.status.is_finished() {
                100.0
            } else {
                0.0
            };
        }
        (self.items_processed as f64 / self.items_total as f64) * 100.0
    }

    /// Returns the duration of the session (so far or total)
    pub fn duration(&self) -> chrono::Duration {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        end - self.started_at
    }

    /// Returns true if the session is still running
    pub fn is_running(&self) -> bool {
        self.status.is_running()
    }

    /// Returns the number of remaining items
    pub fn items_remaining(&self) -> u64 {
        self.items_total.saturating_sub(self.items_processed)
    }

    // --- T037: Methods ---

    /// T037: Marks the session as completed successfully
    pub fn complete(&mut self) {
        self.status = SessionStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// T037: Marks the session as failed with a reason
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.status = SessionStatus::Failed(reason.into());
        self.completed_at = Some(Utc::now());
    }

    /// Marks the session as cancelled
    pub fn cancel(&mut self) {
        self.status = SessionStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    /// T037: Adds an error to the session
    pub fn add_error(&mut self, error: SessionError) {
        self.errors.push(error);
    }

    /// T037: Updates progress counters
    ///
    /// # Arguments
    /// * `processed` - New items_processed value
    /// * `succeeded` - New items_succeeded value
    /// * `failed` - New items_failed value
    pub fn update_progress(&mut self, processed: u64, succeeded: u64, failed: u64) {
        self.items_processed = processed;
        self.items_succeeded = succeeded;
        self.items_failed = failed;
    }

    /// Increments progress counters for a successful item
    pub fn record_success(&mut self) {
        self.items_processed += 1;
        self.items_succeeded += 1;
    }

    /// Increments progress counters for a failed item
    pub fn record_failure(&mut self) {
        self.items_processed += 1;
        self.items_failed += 1;
    }

    /// Sets the total number of items to process
    pub fn set_items_total(&mut self, total: u64) {
        self.items_total = total;
    }

    /// Adds to bytes uploaded
    pub fn add_bytes_uploaded(&mut self, bytes: u64) {
        self.bytes_uploaded += bytes;
    }

    /// Adds to bytes downloaded
    pub fn add_bytes_downloaded(&mut self, bytes: u64) {
        self.bytes_downloaded += bytes;
    }

    /// Sets the starting delta token
    pub fn set_delta_token_start(&mut self, token: DeltaToken) {
        self.delta_token_start = Some(token);
    }

    /// Sets the ending delta token
    pub fn set_delta_token_end(&mut self, token: DeltaToken) {
        self.delta_token_end = Some(token);
    }

    /// T171: Sets the total items received from the delta query
    pub fn set_items_checked(&mut self, count: u64) {
        self.items_checked = count;
    }

    /// T171: Sets items that actually required action
    pub fn set_items_synced(&mut self, count: u64) {
        self.items_synced = count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> SyncSession {
        let account_id = AccountId::new();
        SyncSession::new(account_id)
    }

    mod session_status_tests {
        use super::*;

        #[test]
        fn test_is_running() {
            assert!(SessionStatus::Running.is_running());
            assert!(!SessionStatus::Completed.is_running());
            assert!(!SessionStatus::Failed("error".to_string()).is_running());
            assert!(!SessionStatus::Cancelled.is_running());
        }

        #[test]
        fn test_is_finished() {
            assert!(!SessionStatus::Running.is_finished());
            assert!(SessionStatus::Completed.is_finished());
            assert!(SessionStatus::Failed("error".to_string()).is_finished());
            assert!(SessionStatus::Cancelled.is_finished());
        }

        #[test]
        fn test_is_success() {
            assert!(!SessionStatus::Running.is_success());
            assert!(SessionStatus::Completed.is_success());
            assert!(!SessionStatus::Failed("error".to_string()).is_success());
            assert!(!SessionStatus::Cancelled.is_success());
        }

        #[test]
        fn test_is_failed() {
            assert!(!SessionStatus::Running.is_failed());
            assert!(!SessionStatus::Completed.is_failed());
            assert!(SessionStatus::Failed("error".to_string()).is_failed());
            assert!(!SessionStatus::Cancelled.is_failed());
        }

        #[test]
        fn test_display() {
            assert_eq!(format!("{}", SessionStatus::Running), "running");
            assert_eq!(format!("{}", SessionStatus::Completed), "completed");
            assert_eq!(
                format!("{}", SessionStatus::Failed("network".to_string())),
                "failed: network"
            );
            assert_eq!(format!("{}", SessionStatus::Cancelled), "cancelled");
        }

        #[test]
        fn test_serialization() {
            let running = SessionStatus::Running;
            let json = serde_json::to_string(&running).unwrap();
            assert_eq!(json, "\"running\"");

            let failed = SessionStatus::Failed("test error".to_string());
            let json = serde_json::to_string(&failed).unwrap();
            assert_eq!(json, "{\"failed\":\"test error\"}");
        }
    }

    mod session_error_tests {
        use super::*;

        #[test]
        fn test_new_error() {
            let item_id = UniqueId::new();
            let error = SessionError::new(item_id, "E001", "File not found");

            assert_eq!(error.item_id(), &item_id);
            assert_eq!(error.error_code(), "E001");
            assert_eq!(error.message(), "File not found");
        }

        #[test]
        fn test_with_timestamp() {
            let item_id = UniqueId::new();
            let timestamp = Utc::now();
            let error =
                SessionError::with_timestamp(item_id, "E002", "Permission denied", timestamp);

            assert_eq!(error.item_id(), &item_id);
            assert_eq!(error.timestamp(), timestamp);
        }

        #[test]
        fn test_display() {
            let item_id = UniqueId::new();
            let error = SessionError::new(item_id, "E001", "Test error");
            let display = format!("{}", error);
            assert!(display.contains("E001"));
            assert!(display.contains("Test error"));
        }
    }

    mod sync_session_tests {
        use super::*;

        #[test]
        fn test_new_session() {
            let session = create_test_session();

            assert!(session.is_running());
            assert_eq!(session.items_total(), 0);
            assert_eq!(session.items_processed(), 0);
            assert_eq!(session.items_succeeded(), 0);
            assert_eq!(session.items_failed(), 0);
            assert_eq!(session.bytes_uploaded(), 0);
            assert_eq!(session.bytes_downloaded(), 0);
            assert!(session.delta_token_start().is_none());
            assert!(session.delta_token_end().is_none());
            assert!(session.errors().is_empty());
            assert!(session.completed_at().is_none());
        }

        #[test]
        fn test_complete() {
            let mut session = create_test_session();
            session.complete();

            assert!(!session.is_running());
            assert!(session.status().is_success());
            assert!(session.completed_at().is_some());
        }

        #[test]
        fn test_fail() {
            let mut session = create_test_session();
            session.fail("Network timeout");

            assert!(!session.is_running());
            assert!(session.status().is_failed());
            assert!(session.completed_at().is_some());
            assert!(
                matches!(session.status(), SessionStatus::Failed(msg) if msg == "Network timeout")
            );
        }

        #[test]
        fn test_cancel() {
            let mut session = create_test_session();
            session.cancel();

            assert!(!session.is_running());
            assert_eq!(*session.status(), SessionStatus::Cancelled);
            assert!(session.completed_at().is_some());
        }

        #[test]
        fn test_add_error() {
            let mut session = create_test_session();
            let item_id = UniqueId::new();
            let error = SessionError::new(item_id, "E001", "Test error");

            session.add_error(error);

            assert_eq!(session.errors().len(), 1);
            assert_eq!(session.errors()[0].error_code(), "E001");
        }

        #[test]
        fn test_update_progress() {
            let mut session = create_test_session();
            session.set_items_total(100);
            session.update_progress(50, 45, 5);

            assert_eq!(session.items_processed(), 50);
            assert_eq!(session.items_succeeded(), 45);
            assert_eq!(session.items_failed(), 5);
        }

        #[test]
        fn test_record_success() {
            let mut session = create_test_session();
            session.record_success();
            session.record_success();

            assert_eq!(session.items_processed(), 2);
            assert_eq!(session.items_succeeded(), 2);
            assert_eq!(session.items_failed(), 0);
        }

        #[test]
        fn test_record_failure() {
            let mut session = create_test_session();
            session.record_failure();

            assert_eq!(session.items_processed(), 1);
            assert_eq!(session.items_succeeded(), 0);
            assert_eq!(session.items_failed(), 1);
        }

        #[test]
        fn test_progress_percent() {
            let mut session = create_test_session();
            session.set_items_total(100);
            session.update_progress(50, 50, 0);

            assert!((session.progress_percent() - 50.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_progress_percent_zero_total() {
            let session = create_test_session();
            assert!((session.progress_percent() - 0.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_progress_percent_zero_total_finished() {
            let mut session = create_test_session();
            session.complete();
            assert!((session.progress_percent() - 100.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_items_remaining() {
            let mut session = create_test_session();
            session.set_items_total(100);
            session.update_progress(30, 30, 0);

            assert_eq!(session.items_remaining(), 70);
        }

        #[test]
        fn test_bytes_tracking() {
            let mut session = create_test_session();
            session.add_bytes_uploaded(1024);
            session.add_bytes_uploaded(2048);
            session.add_bytes_downloaded(4096);

            assert_eq!(session.bytes_uploaded(), 3072);
            assert_eq!(session.bytes_downloaded(), 4096);
        }

        #[test]
        fn test_delta_tokens() {
            let mut session = create_test_session();
            let start_token = DeltaToken::new("start_token".to_string()).unwrap();
            let end_token = DeltaToken::new("end_token".to_string()).unwrap();

            session.set_delta_token_start(start_token);
            session.set_delta_token_end(end_token);

            assert_eq!(session.delta_token_start().unwrap().as_str(), "start_token");
            assert_eq!(session.delta_token_end().unwrap().as_str(), "end_token");
        }

        #[test]
        fn test_items_checked_and_synced() {
            let mut session = create_test_session();
            assert_eq!(session.items_checked(), 0);
            assert_eq!(session.items_synced(), 0);

            session.set_items_checked(100);
            session.set_items_synced(25);

            assert_eq!(session.items_checked(), 100);
            assert_eq!(session.items_synced(), 25);
        }

        #[test]
        fn test_sync_efficiency_normal() {
            let mut session = create_test_session();
            session.set_items_checked(100);
            session.set_items_synced(25);

            assert!((session.sync_efficiency() - 0.25).abs() < f64::EPSILON);
        }

        #[test]
        fn test_sync_efficiency_zero_checked() {
            let session = create_test_session();
            assert!((session.sync_efficiency() - 0.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_sync_efficiency_all_synced() {
            let mut session = create_test_session();
            session.set_items_checked(50);
            session.set_items_synced(50);

            assert!((session.sync_efficiency() - 1.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_sync_efficiency_none_synced() {
            let mut session = create_test_session();
            session.set_items_checked(200);
            session.set_items_synced(0);

            assert!((session.sync_efficiency() - 0.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_serialization_roundtrip() {
            let mut session = create_test_session();
            session.set_items_total(100);
            session.update_progress(50, 45, 5);

            let json = serde_json::to_string(&session).unwrap();
            let deserialized: SyncSession = serde_json::from_str(&json).unwrap();

            assert_eq!(session.items_total(), deserialized.items_total());
            assert_eq!(session.items_processed(), deserialized.items_processed());
            assert_eq!(session.items_succeeded(), deserialized.items_succeeded());
            assert_eq!(session.items_failed(), deserialized.items_failed());
        }
    }
}
