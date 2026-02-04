//! State repository port (driven/secondary port)
//!
//! This module defines the interface for persisting and querying
//! synchronization state, including sync items, accounts, sessions,
//! audit entries, and conflicts.
//!
//! ## Design Notes
//!
//! - Uses `anyhow::Result` because storage errors are adapter-specific
//!   (SQLite, filesystem, etc.) and don't need domain-level classification.
//! - The `ItemFilter` struct provides a composable query mechanism
//!   without exposing storage implementation details.
//! - All write operations take references to domain entities, allowing
//!   the caller to retain ownership.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::domain::{
    newtypes::{AccountId, RemoteId, SessionId, SyncPath, UniqueId},
    sync_item::ItemState,
    Account, AuditEntry, Conflict, SyncItem, SyncSession,
};

// ============================================================================
// T053: ItemFilter struct
// ============================================================================

/// Filter criteria for querying sync items
///
/// All fields are optional; when `None`, no filtering is applied for that field.
/// Multiple filters are combined with AND logic.
///
/// # Example
///
/// ```
/// use lnxdrive_core::ports::ItemFilter;
/// use lnxdrive_core::domain::sync_item::ItemState;
///
/// // Query all modified items for a specific account
/// let filter = ItemFilter {
///     account_id: None, // could be set to filter by account
///     state: Some(ItemState::Modified),
///     path_prefix: None,
///     modified_since: None,
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct ItemFilter {
    /// Filter by account ID
    pub account_id: Option<AccountId>,
    /// Filter by item state
    pub state: Option<ItemState>,
    /// Filter by path prefix (items whose local path starts with this prefix)
    pub path_prefix: Option<SyncPath>,
    /// Filter by modification time (items modified after this timestamp)
    pub modified_since: Option<DateTime<Utc>>,
}

impl ItemFilter {
    /// Creates a new empty filter (matches all items)
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the account ID filter
    pub fn with_account_id(mut self, account_id: AccountId) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Sets the item state filter
    pub fn with_state(mut self, state: ItemState) -> Self {
        self.state = Some(state);
        self
    }

    /// Sets the path prefix filter
    pub fn with_path_prefix(mut self, path_prefix: SyncPath) -> Self {
        self.path_prefix = Some(path_prefix);
        self
    }

    /// Sets the modified since filter
    pub fn with_modified_since(mut self, since: DateTime<Utc>) -> Self {
        self.modified_since = Some(since);
        self
    }

    /// Returns true if no filters are set
    pub fn is_empty(&self) -> bool {
        self.account_id.is_none()
            && self.state.is_none()
            && self.path_prefix.is_none()
            && self.modified_since.is_none()
    }
}

// ============================================================================
// T054: IStateRepository trait
// ============================================================================

/// Port trait for persistent state storage
///
/// This is the primary interface for all persistence operations in LNXDrive.
/// It covers CRUD operations for all domain entities: sync items, accounts,
/// sessions, audit entries, and conflicts.
///
/// ## Implementation Notes
///
/// - Implementations should ensure atomicity for individual operations.
/// - For batch operations, implementations may use transactions internally.
/// - The `count_items_by_state` method returns a map where keys are state
///   names (as returned by `ItemState::name()`) and values are counts.
/// - Audit and conflict operations are included here to avoid proliferating
///   small repository traits; implementations may delegate to sub-repositories.
#[async_trait::async_trait]
pub trait IStateRepository: Send + Sync {
    // --- SyncItem operations ---

    /// Saves a sync item (insert or update)
    ///
    /// If an item with the same ID already exists, it is updated.
    async fn save_item(&self, item: &SyncItem) -> anyhow::Result<()>;

    /// Retrieves a sync item by its unique ID
    async fn get_item(&self, id: &UniqueId) -> anyhow::Result<Option<SyncItem>>;

    /// Retrieves a sync item by its local path
    async fn get_item_by_path(&self, path: &SyncPath) -> anyhow::Result<Option<SyncItem>>;

    /// Retrieves a sync item by its remote ID
    async fn get_item_by_remote_id(&self, remote_id: &RemoteId)
        -> anyhow::Result<Option<SyncItem>>;

    /// Queries sync items matching the given filter criteria
    async fn query_items(&self, filter: &ItemFilter) -> anyhow::Result<Vec<SyncItem>>;

    /// Deletes a sync item by its unique ID
    async fn delete_item(&self, id: &UniqueId) -> anyhow::Result<()>;

    /// Counts sync items grouped by state for a given account
    ///
    /// Returns a map where keys are state names (e.g., "Online", "Hydrated")
    /// and values are the number of items in each state.
    async fn count_items_by_state(
        &self,
        account_id: &AccountId,
    ) -> anyhow::Result<HashMap<String, u64>>;

    // --- Account operations ---

    /// Saves an account (insert or update)
    async fn save_account(&self, account: &Account) -> anyhow::Result<()>;

    /// Retrieves an account by its ID
    async fn get_account(&self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// Retrieves the default (primary) account
    ///
    /// Returns `None` if no accounts are configured.
    async fn get_default_account(&self) -> anyhow::Result<Option<Account>>;

    // --- Session operations ---

    /// Saves a sync session (insert or update)
    async fn save_session(&self, session: &SyncSession) -> anyhow::Result<()>;

    /// Retrieves a sync session by its ID
    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<SyncSession>>;

    // --- Audit operations ---

    /// Saves an audit entry
    async fn save_audit(&self, entry: &AuditEntry) -> anyhow::Result<()>;

    /// Retrieves all audit entries for a specific sync item
    ///
    /// Returns entries ordered by timestamp (oldest first).
    async fn get_audit_trail(&self, item_id: &UniqueId) -> anyhow::Result<Vec<AuditEntry>>;

    /// Retrieves audit entries since a given timestamp, up to a limit
    ///
    /// Returns entries ordered by timestamp (newest first).
    async fn get_audit_since(
        &self,
        since: DateTime<Utc>,
        limit: u32,
    ) -> anyhow::Result<Vec<AuditEntry>>;

    // --- Conflict operations ---

    /// Saves a conflict record (insert or update)
    async fn save_conflict(&self, conflict: &Conflict) -> anyhow::Result<()>;

    /// Retrieves all unresolved conflicts
    ///
    /// Returns conflicts ordered by detection time (newest first).
    async fn get_unresolved_conflicts(&self) -> anyhow::Result<Vec<Conflict>>;
}
