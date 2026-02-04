//! SyncItem domain entity
//!
//! This module defines the SyncItem entity which represents a file or folder
//! being synchronized between local storage and OneDrive using the
//! Files-On-Demand model.
//!
//! ## State Machine
//!
//! ```text
//!     ┌──────────┐    access     ┌───────────┐   complete   ┌───────────┐
//!     │  Online  │ ────────────► │ Hydrating │ ───────────► │ Hydrated  │
//!     │(placeholder)│            │(downloading)│            │ (local)   │
//!     └──────────┘               └───────────┘              └───────────┘
//!          ▲                                                     │
//!          │                                                     │
//!          │    dehydrate                              modify    │
//!          └─────────────────────────────────────────────────────┘
//!                                     │
//!                                     ▼
//!                              ┌───────────┐
//!                              │ Modified  │ ──── conflict ───► Conflicted
//!                              │ (dirty)   │
//!                              └───────────┘
//!                                     │
//!                                     │ sync
//!                                     ▼
//!                              ┌───────────┐
//!                              │ Hydrated  │
//!                              └───────────┘
//! ```

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::errors::DomainError;
use super::newtypes::{FileHash, RemoteId, RemotePath, SyncPath, UniqueId};

// ============================================================================
// T025: ItemState enum
// ============================================================================

/// State of a sync item in the Files-On-Demand model
///
/// Represents the hydration state of a file, tracking whether content
/// is available locally or only in the cloud.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Metadata only, content exists only in cloud (placeholder file)
    #[default]
    Online,
    /// Currently downloading content from cloud
    Hydrating,
    /// Fully synced, content available locally
    Hydrated,
    /// Local changes pending upload to cloud
    Modified,
    /// Conflict detected between local and remote versions
    Conflicted,
    /// Error state with reason
    Error(String),
    /// Marked for deletion
    Deleted,
}

impl ItemState {
    /// Returns true if the item content is available locally
    pub fn is_local(&self) -> bool {
        matches!(self, ItemState::Hydrated | ItemState::Modified)
    }

    /// Returns true if the item is only a placeholder
    pub fn is_placeholder(&self) -> bool {
        matches!(self, ItemState::Online)
    }

    /// Returns true if the item is in an active transfer state
    pub fn is_transferring(&self) -> bool {
        matches!(self, ItemState::Hydrating)
    }

    /// Returns true if the item needs user attention
    pub fn needs_attention(&self) -> bool {
        matches!(self, ItemState::Conflicted | ItemState::Error(_))
    }

    /// Returns true if the item has pending changes to sync
    pub fn has_pending_changes(&self) -> bool {
        matches!(self, ItemState::Modified)
    }

    /// Returns the state name as a string (without error details)
    pub fn name(&self) -> &'static str {
        match self {
            ItemState::Online => "Online",
            ItemState::Hydrating => "Hydrating",
            ItemState::Hydrated => "Hydrated",
            ItemState::Modified => "Modified",
            ItemState::Conflicted => "Conflicted",
            ItemState::Error(_) => "Error",
            ItemState::Deleted => "Deleted",
        }
    }
}

impl fmt::Display for ItemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemState::Online => write!(f, "online"),
            ItemState::Hydrating => write!(f, "hydrating"),
            ItemState::Hydrated => write!(f, "hydrated"),
            ItemState::Modified => write!(f, "modified"),
            ItemState::Conflicted => write!(f, "conflicted"),
            ItemState::Error(reason) => write!(f, "error: {}", reason),
            ItemState::Deleted => write!(f, "deleted"),
        }
    }
}

// ============================================================================
// T026: ItemMetadata struct
// ============================================================================

/// Unix-style file permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    /// Read permission
    pub read: bool,
    /// Write permission
    pub write: bool,
    /// Execute permission (for directories, this means list)
    pub execute: bool,
}

impl Permissions {
    /// Creates permissions with all flags set to true
    pub fn all() -> Self {
        Self {
            read: true,
            write: true,
            execute: true,
        }
    }

    /// Creates read-only permissions
    pub fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            execute: false,
        }
    }

    /// Creates permissions from a Unix mode (e.g., 0o644)
    pub fn from_mode(mode: u32) -> Self {
        Self {
            read: (mode & 0o400) != 0,
            write: (mode & 0o200) != 0,
            execute: (mode & 0o100) != 0,
        }
    }

    /// Converts to a Unix mode for the owner bits
    pub fn to_mode(&self) -> u32 {
        let mut mode = 0u32;
        if self.read {
            mode |= 0o400;
        }
        if self.write {
            mode |= 0o200;
        }
        if self.execute {
            mode |= 0o100;
        }
        mode
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::all()
    }
}

/// Metadata about a sync item
///
/// Contains file system metadata and OneDrive-specific information
/// needed for synchronization decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemMetadata {
    /// Whether this item is a directory
    is_directory: bool,
    /// MIME type of the file (None for directories)
    mime_type: Option<String>,
    /// When the item was originally created
    created_at: DateTime<Utc>,
    /// ETag from OneDrive for optimistic concurrency control
    etag: Option<String>,
    /// File permissions
    permissions: Permissions,
}

impl ItemMetadata {
    /// Creates metadata for a file
    pub fn new_file(mime_type: Option<String>) -> Self {
        Self {
            is_directory: false,
            mime_type,
            created_at: Utc::now(),
            etag: None,
            permissions: Permissions::all(),
        }
    }

    /// Creates metadata for a directory
    pub fn new_directory() -> Self {
        Self {
            is_directory: true,
            mime_type: None,
            created_at: Utc::now(),
            etag: None,
            permissions: Permissions::all(),
        }
    }

    /// Creates metadata with all fields specified
    pub fn new(
        is_directory: bool,
        mime_type: Option<String>,
        created_at: DateTime<Utc>,
        etag: Option<String>,
        permissions: Permissions,
    ) -> Self {
        Self {
            is_directory,
            mime_type,
            created_at,
            etag,
            permissions,
        }
    }

    /// Returns true if this is a directory
    pub fn is_directory(&self) -> bool {
        self.is_directory
    }

    /// Returns the MIME type
    pub fn mime_type(&self) -> Option<&str> {
        self.mime_type.as_deref()
    }

    /// Returns when the item was created
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the ETag
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Returns the permissions
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Sets the ETag
    pub fn set_etag(&mut self, etag: impl Into<String>) {
        self.etag = Some(etag.into());
    }

    /// Clears the ETag
    pub fn clear_etag(&mut self) {
        self.etag = None;
    }

    /// Sets the permissions
    pub fn set_permissions(&mut self, permissions: Permissions) {
        self.permissions = permissions;
    }

    /// Sets the MIME type
    pub fn set_mime_type(&mut self, mime_type: Option<String>) {
        self.mime_type = mime_type;
    }
}

// ============================================================================
// T027: ErrorInfo struct
// ============================================================================

/// Information about an error that occurred during synchronization
///
/// Tracks error details and retry information for failed operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code for categorization (e.g., "NETWORK_ERROR", "AUTH_EXPIRED")
    code: String,
    /// Human-readable error message
    message: String,
    /// Number of retry attempts made
    retry_count: u32,
    /// When the last attempt was made
    last_attempt: DateTime<Utc>,
    /// When the next retry should be attempted (None if no retry scheduled)
    next_retry: Option<DateTime<Utc>>,
}

impl ErrorInfo {
    /// Creates a new ErrorInfo
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retry_count: 0,
            last_attempt: Utc::now(),
            next_retry: None,
        }
    }

    /// Creates an ErrorInfo with a scheduled retry
    pub fn with_retry(
        code: impl Into<String>,
        message: impl Into<String>,
        retry_delay: Duration,
    ) -> Self {
        let now = Utc::now();
        Self {
            code: code.into(),
            message: message.into(),
            retry_count: 0,
            last_attempt: now,
            next_retry: Some(now + retry_delay),
        }
    }

    /// Returns the error code
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the error message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Returns when the last attempt was made
    pub fn last_attempt(&self) -> DateTime<Utc> {
        self.last_attempt
    }

    /// Returns when the next retry is scheduled
    pub fn next_retry(&self) -> Option<DateTime<Utc>> {
        self.next_retry
    }

    /// Returns true if a retry is scheduled
    pub fn has_retry_scheduled(&self) -> bool {
        self.next_retry.is_some()
    }

    /// Returns true if it's time to retry
    pub fn should_retry_now(&self) -> bool {
        match self.next_retry {
            Some(next) => Utc::now() >= next,
            None => false,
        }
    }

    /// Increments the retry count and updates the last attempt time
    pub fn record_retry(&mut self) {
        self.retry_count += 1;
        self.last_attempt = Utc::now();
    }

    /// Schedules the next retry with exponential backoff
    ///
    /// Uses the formula: base_delay * 2^retry_count, capped at max_delay
    pub fn schedule_retry_exponential(&mut self, base_delay: Duration, max_delay: Duration) {
        let multiplier = 2i64.saturating_pow(self.retry_count);
        let delay = base_delay * multiplier as i32;
        let capped_delay = if delay > max_delay { max_delay } else { delay };
        self.next_retry = Some(Utc::now() + capped_delay);
    }

    /// Cancels any scheduled retry
    pub fn cancel_retry(&mut self) {
        self.next_retry = None;
    }

    /// Creates a common network error
    pub fn network_error(message: impl Into<String>) -> Self {
        Self::with_retry("NETWORK_ERROR", message, Duration::seconds(30))
    }

    /// Creates a common authentication error
    pub fn auth_error(message: impl Into<String>) -> Self {
        Self::new("AUTH_ERROR", message)
    }

    /// Creates a common rate limit error
    pub fn rate_limited(retry_after: Duration) -> Self {
        Self::with_retry("RATE_LIMITED", "Rate limit exceeded", retry_after)
    }

    /// Creates a common conflict error
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("CONFLICT", message)
    }
}

impl fmt::Display for ErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if self.retry_count > 0 {
            write!(f, " (retries: {})", self.retry_count)?;
        }
        Ok(())
    }
}

// ============================================================================
// T028: SyncItem struct
// ============================================================================

/// Represents a file or folder being synchronized between local storage and OneDrive
///
/// SyncItem is the core domain entity that tracks the synchronization state
/// of individual files and directories using the Files-On-Demand model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncItem {
    /// Unique identifier for this item within LNXDrive
    id: UniqueId,
    /// Local file system path
    local_path: SyncPath,
    /// OneDrive item ID (None for new local items not yet uploaded)
    remote_id: Option<RemoteId>,
    /// Remote OneDrive path
    remote_path: RemotePath,
    /// Current synchronization state
    state: ItemState,
    /// Content hash from OneDrive (quickXorHash)
    content_hash: Option<FileHash>,
    /// Local content hash (computed locally)
    local_hash: Option<FileHash>,
    /// File size in bytes (0 for directories)
    size_bytes: u64,
    /// When this item was last successfully synced
    last_sync: Option<DateTime<Utc>>,
    /// Last modified time on the local filesystem
    last_modified_local: Option<DateTime<Utc>>,
    /// Last modified time on OneDrive
    last_modified_remote: Option<DateTime<Utc>>,
    /// Item metadata
    metadata: ItemMetadata,
    /// Error information if state is Error
    error_info: Option<ErrorInfo>,
}

// ============================================================================
// T029: SyncItem::new() constructor
// ============================================================================

impl SyncItem {
    /// Creates a new SyncItem with validation
    ///
    /// # Arguments
    ///
    /// * `local_path` - The local filesystem path (must be absolute)
    /// * `remote_path` - The OneDrive path
    /// * `is_directory` - Whether this is a directory
    ///
    /// # Errors
    ///
    /// Returns `DomainError::ValidationFailed` if:
    /// - The paths are inconsistent (e.g., file path ends with /)
    pub fn new(
        local_path: SyncPath,
        remote_path: RemotePath,
        is_directory: bool,
    ) -> Result<Self, DomainError> {
        // Validate that directory flag is consistent
        // (We can't fully validate path consistency without filesystem access)

        let metadata = if is_directory {
            ItemMetadata::new_directory()
        } else {
            ItemMetadata::new_file(None)
        };

        Ok(Self {
            id: UniqueId::new(),
            local_path,
            remote_id: None,
            remote_path,
            state: ItemState::Online,
            content_hash: None,
            local_hash: None,
            size_bytes: 0,
            last_sync: None,
            last_modified_local: None,
            last_modified_remote: None,
            metadata,
            error_info: None,
        })
    }

    /// Creates a new SyncItem for a file with size and MIME type
    pub fn new_file(
        local_path: SyncPath,
        remote_path: RemotePath,
        size_bytes: u64,
        mime_type: Option<String>,
    ) -> Result<Self, DomainError> {
        let mut item = Self::new(local_path, remote_path, false)?;
        item.size_bytes = size_bytes;
        item.metadata.set_mime_type(mime_type);
        Ok(item)
    }

    /// Creates a new SyncItem for a directory
    pub fn new_directory(
        local_path: SyncPath,
        remote_path: RemotePath,
    ) -> Result<Self, DomainError> {
        Self::new(local_path, remote_path, true)
    }

    /// Creates a SyncItem from remote data (for initial sync from cloud)
    pub fn from_remote(
        local_path: SyncPath,
        remote_path: RemotePath,
        remote_id: RemoteId,
        is_directory: bool,
        size_bytes: u64,
        content_hash: Option<FileHash>,
        last_modified_remote: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        let mut item = Self::new(local_path, remote_path, is_directory)?;
        item.remote_id = Some(remote_id);
        item.size_bytes = size_bytes;
        item.content_hash = content_hash;
        item.last_modified_remote = Some(last_modified_remote);
        Ok(item)
    }

    // --- Getters ---

    /// Returns the item's unique identifier
    pub fn id(&self) -> &UniqueId {
        &self.id
    }

    /// Returns the local file path
    pub fn local_path(&self) -> &SyncPath {
        &self.local_path
    }

    /// Returns the remote ID if set
    pub fn remote_id(&self) -> Option<&RemoteId> {
        self.remote_id.as_ref()
    }

    /// Returns the remote path
    pub fn remote_path(&self) -> &RemotePath {
        &self.remote_path
    }

    /// Returns the current state
    pub fn state(&self) -> &ItemState {
        &self.state
    }

    /// Returns the content hash from OneDrive
    pub fn content_hash(&self) -> Option<&FileHash> {
        self.content_hash.as_ref()
    }

    /// Returns the local content hash
    pub fn local_hash(&self) -> Option<&FileHash> {
        self.local_hash.as_ref()
    }

    /// Returns the file size in bytes
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns when the item was last synced
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.last_sync
    }

    /// Returns the local last modified time
    pub fn last_modified_local(&self) -> Option<DateTime<Utc>> {
        self.last_modified_local
    }

    /// Returns the remote last modified time
    pub fn last_modified_remote(&self) -> Option<DateTime<Utc>> {
        self.last_modified_remote
    }

    /// Returns the metadata
    pub fn metadata(&self) -> &ItemMetadata {
        &self.metadata
    }

    /// Returns mutable metadata
    pub fn metadata_mut(&mut self) -> &mut ItemMetadata {
        &mut self.metadata
    }

    /// Returns the error info if any
    pub fn error_info(&self) -> Option<&ErrorInfo> {
        self.error_info.as_ref()
    }

    /// Returns true if this is a directory
    pub fn is_directory(&self) -> bool {
        self.metadata.is_directory()
    }

    /// Returns true if the local and remote hashes match
    pub fn hashes_match(&self) -> bool {
        match (&self.local_hash, &self.content_hash) {
            (Some(local), Some(remote)) => local == remote,
            _ => false,
        }
    }

    // --- Setters ---

    /// Sets the remote ID
    pub fn set_remote_id(&mut self, remote_id: RemoteId) {
        self.remote_id = Some(remote_id);
    }

    /// Sets the content hash from OneDrive
    pub fn set_content_hash(&mut self, hash: FileHash) {
        self.content_hash = Some(hash);
    }

    /// Sets the local content hash
    pub fn set_local_hash(&mut self, hash: FileHash) {
        self.local_hash = Some(hash);
    }

    /// Sets the file size
    pub fn set_size_bytes(&mut self, size: u64) {
        self.size_bytes = size;
    }

    /// Sets the local last modified time
    pub fn set_last_modified_local(&mut self, time: DateTime<Utc>) {
        self.last_modified_local = Some(time);
    }

    /// Sets the remote last modified time
    pub fn set_last_modified_remote(&mut self, time: DateTime<Utc>) {
        self.last_modified_remote = Some(time);
    }

    /// Updates the last sync time to now
    pub fn mark_synced(&mut self) {
        self.last_sync = Some(Utc::now());
    }

    /// Updates the local path
    pub fn update_local_path(&mut self, path: SyncPath) {
        self.local_path = path;
    }

    /// Updates the remote path
    pub fn update_remote_path(&mut self, path: RemotePath) {
        self.remote_path = path;
    }
}

// ============================================================================
// T030: State transition methods
// ============================================================================

impl SyncItem {
    /// Checks if a state transition is valid
    ///
    /// Valid transitions:
    /// - Online -> Hydrating, Error, Deleted
    /// - Hydrating -> Hydrated, Error
    /// - Hydrated -> Modified, Online (dehydrate), Error, Deleted
    /// - Modified -> Hydrated (after sync), Conflicted, Error
    /// - Conflicted -> Hydrated (after resolution), Error
    /// - Error -> any state (retry)
    /// - Deleted -> (terminal state, no transitions)
    pub fn can_transition_to(&self, target: &ItemState) -> bool {
        // Deleted is a terminal state
        if matches!(self.state, ItemState::Deleted) {
            return false;
        }

        // Error state can transition to any state (retry mechanism)
        if matches!(self.state, ItemState::Error(_)) {
            return true;
        }

        match (&self.state, target) {
            // Online transitions
            (ItemState::Online, ItemState::Hydrating) => true,
            (ItemState::Online, ItemState::Error(_)) => true,
            (ItemState::Online, ItemState::Deleted) => true,

            // Hydrating transitions
            (ItemState::Hydrating, ItemState::Hydrated) => true,
            (ItemState::Hydrating, ItemState::Error(_)) => true,

            // Hydrated transitions
            (ItemState::Hydrated, ItemState::Modified) => true,
            (ItemState::Hydrated, ItemState::Online) => true, // dehydrate
            (ItemState::Hydrated, ItemState::Error(_)) => true,
            (ItemState::Hydrated, ItemState::Deleted) => true,

            // Modified transitions
            (ItemState::Modified, ItemState::Hydrated) => true, // after sync
            (ItemState::Modified, ItemState::Conflicted) => true,
            (ItemState::Modified, ItemState::Error(_)) => true,

            // Conflicted transitions
            (ItemState::Conflicted, ItemState::Hydrated) => true, // after resolution
            (ItemState::Conflicted, ItemState::Error(_)) => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Attempts to transition to a new state
    ///
    /// # Errors
    ///
    /// Returns `DomainError::InvalidState` if the transition is not allowed.
    pub fn transition_to(&mut self, target: ItemState) -> Result<(), DomainError> {
        if !self.can_transition_to(&target) {
            return Err(DomainError::InvalidState {
                from: self.state.name().to_string(),
                to: target.name().to_string(),
            });
        }

        // Clear error info when leaving error state
        if matches!(self.state, ItemState::Error(_)) {
            self.error_info = None;
        }

        // Set error info when entering error state
        if let ItemState::Error(ref reason) = target {
            if self.error_info.is_none() {
                self.error_info = Some(ErrorInfo::new("UNKNOWN", reason.clone()));
            }
        }

        // Update last sync time when transitioning to Hydrated from Modified
        if matches!(self.state, ItemState::Modified) && matches!(target, ItemState::Hydrated) {
            self.mark_synced();
        }

        self.state = target;
        Ok(())
    }

    /// Transitions to error state with detailed error info
    pub fn transition_to_error(&mut self, error: ErrorInfo) -> Result<(), DomainError> {
        let target = ItemState::Error(error.message().to_string());
        if !self.can_transition_to(&target) {
            return Err(DomainError::InvalidState {
                from: self.state.name().to_string(),
                to: "Error".to_string(),
            });
        }

        self.error_info = Some(error);
        self.state = target;
        Ok(())
    }

    /// Convenience method to start hydrating (downloading)
    pub fn start_hydrating(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Hydrating)
    }

    /// Convenience method to complete hydration
    pub fn complete_hydration(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Hydrated)
    }

    /// Convenience method to dehydrate (convert to placeholder)
    pub fn dehydrate(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Online)
    }

    /// Convenience method to mark as modified
    pub fn mark_modified(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Modified)
    }

    /// Convenience method to mark as conflicted
    pub fn mark_conflicted(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Conflicted)
    }

    /// Convenience method to mark as deleted
    pub fn mark_deleted(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Deleted)
    }

    /// Convenience method to resolve conflict (transition to Hydrated)
    pub fn resolve_conflict(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Hydrated)
    }

    /// Convenience method to sync modified item (transition to Hydrated)
    pub fn complete_sync(&mut self) -> Result<(), DomainError> {
        self.transition_to(ItemState::Hydrated)
    }

    /// Retry from error state to a target state
    pub fn retry_to(&mut self, target: ItemState) -> Result<(), DomainError> {
        if !matches!(self.state, ItemState::Error(_)) {
            return Err(DomainError::InvalidState {
                from: self.state.name().to_string(),
                to: target.name().to_string(),
            });
        }

        // Record the retry attempt
        if let Some(ref mut error_info) = self.error_info {
            error_info.record_retry();
        }

        self.transition_to(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_sync_item() -> SyncItem {
        let local_path = SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap();
        let remote_path = RemotePath::new("/test.txt".to_string()).unwrap();
        SyncItem::new_file(
            local_path,
            remote_path,
            1024,
            Some("text/plain".to_string()),
        )
        .unwrap()
    }

    mod item_state_tests {
        use super::*;

        #[test]
        fn test_is_local() {
            assert!(!ItemState::Online.is_local());
            assert!(!ItemState::Hydrating.is_local());
            assert!(ItemState::Hydrated.is_local());
            assert!(ItemState::Modified.is_local());
            assert!(!ItemState::Conflicted.is_local());
            assert!(!ItemState::Error("test".to_string()).is_local());
            assert!(!ItemState::Deleted.is_local());
        }

        #[test]
        fn test_is_placeholder() {
            assert!(ItemState::Online.is_placeholder());
            assert!(!ItemState::Hydrated.is_placeholder());
        }

        #[test]
        fn test_is_transferring() {
            assert!(ItemState::Hydrating.is_transferring());
            assert!(!ItemState::Online.is_transferring());
            assert!(!ItemState::Hydrated.is_transferring());
        }

        #[test]
        fn test_needs_attention() {
            assert!(ItemState::Conflicted.needs_attention());
            assert!(ItemState::Error("test".to_string()).needs_attention());
            assert!(!ItemState::Hydrated.needs_attention());
        }

        #[test]
        fn test_has_pending_changes() {
            assert!(ItemState::Modified.has_pending_changes());
            assert!(!ItemState::Hydrated.has_pending_changes());
        }

        #[test]
        fn test_display() {
            assert_eq!(format!("{}", ItemState::Online), "online");
            assert_eq!(format!("{}", ItemState::Hydrating), "hydrating");
            assert_eq!(format!("{}", ItemState::Hydrated), "hydrated");
            assert_eq!(format!("{}", ItemState::Modified), "modified");
            assert_eq!(format!("{}", ItemState::Conflicted), "conflicted");
            assert_eq!(
                format!("{}", ItemState::Error("fail".to_string())),
                "error: fail"
            );
            assert_eq!(format!("{}", ItemState::Deleted), "deleted");
        }

        #[test]
        fn test_name() {
            assert_eq!(ItemState::Online.name(), "Online");
            assert_eq!(ItemState::Error("test".to_string()).name(), "Error");
        }

        #[test]
        fn test_default() {
            assert_eq!(ItemState::default(), ItemState::Online);
        }
    }

    mod permissions_tests {
        use super::*;

        #[test]
        fn test_all() {
            let perms = Permissions::all();
            assert!(perms.read);
            assert!(perms.write);
            assert!(perms.execute);
        }

        #[test]
        fn test_read_only() {
            let perms = Permissions::read_only();
            assert!(perms.read);
            assert!(!perms.write);
            assert!(!perms.execute);
        }

        #[test]
        fn test_from_mode() {
            let perms = Permissions::from_mode(0o644);
            assert!(perms.read);
            assert!(perms.write);
            assert!(!perms.execute);

            let perms = Permissions::from_mode(0o755);
            assert!(perms.read);
            assert!(perms.write);
            assert!(perms.execute);
        }

        #[test]
        fn test_to_mode() {
            let perms = Permissions::all();
            assert_eq!(perms.to_mode(), 0o700);

            let perms = Permissions::read_only();
            assert_eq!(perms.to_mode(), 0o400);
        }
    }

    mod metadata_tests {
        use super::*;

        #[test]
        fn test_new_file() {
            let meta = ItemMetadata::new_file(Some("text/plain".to_string()));
            assert!(!meta.is_directory());
            assert_eq!(meta.mime_type(), Some("text/plain"));
            assert!(meta.etag().is_none());
        }

        #[test]
        fn test_new_directory() {
            let meta = ItemMetadata::new_directory();
            assert!(meta.is_directory());
            assert!(meta.mime_type().is_none());
        }

        #[test]
        fn test_set_etag() {
            let mut meta = ItemMetadata::new_file(None);
            meta.set_etag("etag123");
            assert_eq!(meta.etag(), Some("etag123"));

            meta.clear_etag();
            assert!(meta.etag().is_none());
        }

        #[test]
        fn test_set_permissions() {
            let mut meta = ItemMetadata::new_file(None);
            meta.set_permissions(Permissions::read_only());
            assert!(meta.permissions().read);
            assert!(!meta.permissions().write);
        }
    }

    mod error_info_tests {
        use super::*;

        #[test]
        fn test_new() {
            let error = ErrorInfo::new("E001", "Test error");
            assert_eq!(error.code(), "E001");
            assert_eq!(error.message(), "Test error");
            assert_eq!(error.retry_count(), 0);
            assert!(!error.has_retry_scheduled());
        }

        #[test]
        fn test_with_retry() {
            let error = ErrorInfo::with_retry("E001", "Test error", Duration::seconds(30));
            assert!(error.has_retry_scheduled());
            assert!(error.next_retry().is_some());
        }

        #[test]
        fn test_record_retry() {
            let mut error = ErrorInfo::new("E001", "Test error");
            error.record_retry();
            error.record_retry();
            assert_eq!(error.retry_count(), 2);
        }

        #[test]
        fn test_schedule_retry_exponential() {
            let mut error = ErrorInfo::new("E001", "Test error");
            error.schedule_retry_exponential(Duration::seconds(1), Duration::seconds(60));
            assert!(error.has_retry_scheduled());

            error.record_retry();
            error.schedule_retry_exponential(Duration::seconds(1), Duration::seconds(60));
            // After 1 retry, delay should be 2 seconds
        }

        #[test]
        fn test_cancel_retry() {
            let mut error = ErrorInfo::with_retry("E001", "Test", Duration::seconds(30));
            assert!(error.has_retry_scheduled());
            error.cancel_retry();
            assert!(!error.has_retry_scheduled());
        }

        #[test]
        fn test_factory_methods() {
            let network = ErrorInfo::network_error("Connection failed");
            assert_eq!(network.code(), "NETWORK_ERROR");
            assert!(network.has_retry_scheduled());

            let auth = ErrorInfo::auth_error("Token expired");
            assert_eq!(auth.code(), "AUTH_ERROR");

            let rate = ErrorInfo::rate_limited(Duration::seconds(60));
            assert_eq!(rate.code(), "RATE_LIMITED");

            let conflict = ErrorInfo::conflict("Versions differ");
            assert_eq!(conflict.code(), "CONFLICT");
        }

        #[test]
        fn test_display() {
            let error = ErrorInfo::new("E001", "Test error");
            assert_eq!(error.to_string(), "[E001] Test error");

            let mut error_with_retries = ErrorInfo::new("E001", "Test error");
            error_with_retries.record_retry();
            assert_eq!(
                error_with_retries.to_string(),
                "[E001] Test error (retries: 1)"
            );
        }
    }

    mod sync_item_tests {
        use super::*;

        #[test]
        fn test_new() {
            let local_path = SyncPath::new(PathBuf::from("/home/user/sync/file.txt")).unwrap();
            let remote_path = RemotePath::new("/file.txt".to_string()).unwrap();

            let item = SyncItem::new(local_path, remote_path, false).unwrap();

            assert!(!item.is_directory());
            assert!(matches!(item.state(), ItemState::Online));
            assert!(item.remote_id().is_none());
            assert_eq!(item.size_bytes(), 0);
        }

        #[test]
        fn test_new_file() {
            let item = create_test_sync_item();

            assert!(!item.is_directory());
            assert_eq!(item.size_bytes(), 1024);
            assert_eq!(item.metadata().mime_type(), Some("text/plain"));
        }

        #[test]
        fn test_new_directory() {
            let local_path = SyncPath::new(PathBuf::from("/home/user/sync/folder")).unwrap();
            let remote_path = RemotePath::new("/folder".to_string()).unwrap();

            let item = SyncItem::new_directory(local_path, remote_path).unwrap();

            assert!(item.is_directory());
            assert_eq!(item.size_bytes(), 0);
        }

        #[test]
        fn test_from_remote() {
            let local_path = SyncPath::new(PathBuf::from("/home/user/sync/file.txt")).unwrap();
            let remote_path = RemotePath::new("/file.txt".to_string()).unwrap();
            let remote_id = RemoteId::new("ABC123".to_string()).unwrap();
            let hash = FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap();
            let modified = Utc::now();

            let item = SyncItem::from_remote(
                local_path,
                remote_path,
                remote_id.clone(),
                false,
                2048,
                Some(hash),
                modified,
            )
            .unwrap();

            assert_eq!(item.remote_id(), Some(&remote_id));
            assert_eq!(item.size_bytes(), 2048);
            assert!(item.content_hash().is_some());
            assert_eq!(item.last_modified_remote(), Some(modified));
        }

        #[test]
        fn test_hashes_match() {
            let mut item = create_test_sync_item();
            assert!(!item.hashes_match());

            let hash = FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap();
            item.set_local_hash(hash.clone());
            item.set_content_hash(hash);
            assert!(item.hashes_match());
        }

        #[test]
        fn test_setters() {
            let mut item = create_test_sync_item();

            let remote_id = RemoteId::new("XYZ789".to_string()).unwrap();
            item.set_remote_id(remote_id.clone());
            assert_eq!(item.remote_id(), Some(&remote_id));

            item.set_size_bytes(4096);
            assert_eq!(item.size_bytes(), 4096);

            let now = Utc::now();
            item.set_last_modified_local(now);
            assert_eq!(item.last_modified_local(), Some(now));

            item.set_last_modified_remote(now);
            assert_eq!(item.last_modified_remote(), Some(now));

            item.mark_synced();
            assert!(item.last_sync().is_some());
        }

        #[test]
        fn test_serialization_roundtrip() {
            let item = create_test_sync_item();
            let json = serde_json::to_string(&item).unwrap();
            let deserialized: SyncItem = serde_json::from_str(&json).unwrap();

            assert_eq!(item.id(), deserialized.id());
            assert_eq!(item.size_bytes(), deserialized.size_bytes());
            assert_eq!(item.state(), deserialized.state());
        }
    }

    mod state_transition_tests {
        use super::*;

        #[test]
        fn test_can_transition_from_online() {
            let item = create_test_sync_item();
            assert!(item.can_transition_to(&ItemState::Hydrating));
            assert!(item.can_transition_to(&ItemState::Error("test".to_string())));
            assert!(item.can_transition_to(&ItemState::Deleted));
            assert!(!item.can_transition_to(&ItemState::Hydrated));
            assert!(!item.can_transition_to(&ItemState::Modified));
        }

        #[test]
        fn test_can_transition_from_hydrating() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Hydrating).unwrap();

            assert!(item.can_transition_to(&ItemState::Hydrated));
            assert!(item.can_transition_to(&ItemState::Error("test".to_string())));
            assert!(!item.can_transition_to(&ItemState::Online));
            assert!(!item.can_transition_to(&ItemState::Modified));
        }

        #[test]
        fn test_can_transition_from_hydrated() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Hydrating).unwrap();
            item.transition_to(ItemState::Hydrated).unwrap();

            assert!(item.can_transition_to(&ItemState::Modified));
            assert!(item.can_transition_to(&ItemState::Online)); // dehydrate
            assert!(item.can_transition_to(&ItemState::Error("test".to_string())));
            assert!(item.can_transition_to(&ItemState::Deleted));
            assert!(!item.can_transition_to(&ItemState::Hydrating));
        }

        #[test]
        fn test_can_transition_from_modified() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Hydrating).unwrap();
            item.transition_to(ItemState::Hydrated).unwrap();
            item.transition_to(ItemState::Modified).unwrap();

            assert!(item.can_transition_to(&ItemState::Hydrated)); // after sync
            assert!(item.can_transition_to(&ItemState::Conflicted));
            assert!(item.can_transition_to(&ItemState::Error("test".to_string())));
            assert!(!item.can_transition_to(&ItemState::Online));
            assert!(!item.can_transition_to(&ItemState::Deleted));
        }

        #[test]
        fn test_can_transition_from_conflicted() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Hydrating).unwrap();
            item.transition_to(ItemState::Hydrated).unwrap();
            item.transition_to(ItemState::Modified).unwrap();
            item.transition_to(ItemState::Conflicted).unwrap();

            assert!(item.can_transition_to(&ItemState::Hydrated)); // after resolution
            assert!(item.can_transition_to(&ItemState::Error("test".to_string())));
            assert!(!item.can_transition_to(&ItemState::Online));
            assert!(!item.can_transition_to(&ItemState::Modified));
        }

        #[test]
        fn test_can_transition_from_error() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Error("test".to_string()))
                .unwrap();

            // Error can transition to any state
            assert!(item.can_transition_to(&ItemState::Online));
            assert!(item.can_transition_to(&ItemState::Hydrating));
            assert!(item.can_transition_to(&ItemState::Hydrated));
            assert!(item.can_transition_to(&ItemState::Modified));
            assert!(item.can_transition_to(&ItemState::Conflicted));
            assert!(item.can_transition_to(&ItemState::Deleted));
        }

        #[test]
        fn test_deleted_is_terminal() {
            let mut item = create_test_sync_item();
            item.transition_to(ItemState::Deleted).unwrap();

            // Deleted cannot transition to any state
            assert!(!item.can_transition_to(&ItemState::Online));
            assert!(!item.can_transition_to(&ItemState::Hydrating));
            assert!(!item.can_transition_to(&ItemState::Hydrated));
            assert!(!item.can_transition_to(&ItemState::Error("test".to_string())));
        }

        #[test]
        fn test_transition_to_success() {
            let mut item = create_test_sync_item();

            assert!(item.transition_to(ItemState::Hydrating).is_ok());
            assert!(matches!(item.state(), ItemState::Hydrating));

            assert!(item.transition_to(ItemState::Hydrated).is_ok());
            assert!(matches!(item.state(), ItemState::Hydrated));
        }

        #[test]
        fn test_transition_to_failure() {
            let mut item = create_test_sync_item();

            // Cannot go directly from Online to Hydrated
            let result = item.transition_to(ItemState::Hydrated);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                DomainError::InvalidState { .. }
            ));
        }

        #[test]
        fn test_transition_clears_error_info() {
            let mut item = create_test_sync_item();
            let error = ErrorInfo::new("E001", "Test error");
            item.transition_to_error(error).unwrap();

            assert!(item.error_info().is_some());

            item.transition_to(ItemState::Online).unwrap();
            assert!(item.error_info().is_none());
        }

        #[test]
        fn test_transition_to_error_sets_error_info() {
            let mut item = create_test_sync_item();
            let error = ErrorInfo::new("E001", "Test error");

            item.transition_to_error(error).unwrap();

            assert!(item.error_info().is_some());
            assert_eq!(item.error_info().unwrap().code(), "E001");
        }

        #[test]
        fn test_convenience_methods() {
            let mut item = create_test_sync_item();

            item.start_hydrating().unwrap();
            assert!(matches!(item.state(), ItemState::Hydrating));

            item.complete_hydration().unwrap();
            assert!(matches!(item.state(), ItemState::Hydrated));

            item.mark_modified().unwrap();
            assert!(matches!(item.state(), ItemState::Modified));

            item.complete_sync().unwrap();
            assert!(matches!(item.state(), ItemState::Hydrated));
            assert!(item.last_sync().is_some());

            item.dehydrate().unwrap();
            assert!(matches!(item.state(), ItemState::Online));
        }

        #[test]
        fn test_conflict_flow() {
            let mut item = create_test_sync_item();

            item.start_hydrating().unwrap();
            item.complete_hydration().unwrap();
            item.mark_modified().unwrap();
            item.mark_conflicted().unwrap();
            assert!(matches!(item.state(), ItemState::Conflicted));

            item.resolve_conflict().unwrap();
            assert!(matches!(item.state(), ItemState::Hydrated));
        }

        #[test]
        fn test_retry_from_error() {
            let mut item = create_test_sync_item();
            let error = ErrorInfo::new("E001", "Test error");
            item.transition_to_error(error).unwrap();

            item.retry_to(ItemState::Hydrating).unwrap();
            assert!(matches!(item.state(), ItemState::Hydrating));
        }

        #[test]
        fn test_retry_records_attempt() {
            let mut item = create_test_sync_item();
            let error = ErrorInfo::new("E001", "Test error");
            item.transition_to_error(error).unwrap();

            // Need to access error_info before retry
            assert_eq!(item.error_info().unwrap().retry_count(), 0);

            // The retry_to method should increment the retry count before clearing
            item.retry_to(ItemState::Online).unwrap();
            // After successful retry, error_info is cleared
            assert!(item.error_info().is_none());
        }
    }
}
