//! Delta query use case
//!
//! Orchestrates incremental synchronization by querying the Microsoft Graph
//! delta API for changes since the last sync. Handles delta token management,
//! converting cloud-side delta items into domain SyncItems.

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;

use crate::{
    domain::{
        Account, AccountId, AuditAction, AuditEntry, AuditResult, DeltaToken, FileHash, RemoteId,
        RemotePath, SyncItem, SyncPath,
    },
    ports::{DeltaItem, ICloudProvider, IStateRepository},
};

/// Use case for querying incremental changes from the cloud provider
///
/// Coordinates delta queries between the cloud provider and state repository,
/// handling delta token lifecycle.
pub struct QueryDeltaUseCase {
    cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
    state_repository: Arc<dyn IStateRepository + Send + Sync>,
}

impl QueryDeltaUseCase {
    /// Creates a new QueryDeltaUseCase with the required dependencies
    ///
    /// # Arguments
    ///
    /// * `cloud_provider` - Cloud storage provider for delta API queries
    /// * `state_repository` - Persistent storage for sync state and delta tokens
    pub fn new(
        cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
        state_repository: Arc<dyn IStateRepository + Send + Sync>,
    ) -> Self {
        Self {
            cloud_provider,
            state_repository,
        }
    }

    /// Executes a delta query to get all changes since the last sync
    ///
    /// This method:
    /// 1. Retrieves the account's current delta token (None triggers full sync)
    /// 2. Queries the cloud provider's delta API via `get_delta`
    /// 3. Saves the new delta token to the account for the next sync
    /// 4. Returns the complete list of delta items
    ///
    /// # Arguments
    ///
    /// * `account` - The account to query changes for
    ///
    /// # Returns
    ///
    /// A vector of all DeltaItems representing changes since the last sync
    ///
    /// # Errors
    ///
    /// Returns an error if the delta query or token persistence fails
    pub async fn execute(&self, account: &Account) -> Result<Vec<DeltaItem>> {
        // Step 1: Get current delta token from account (None = initial full sync)
        let delta_token = account.delta_token().cloned();

        // Step 2: Query the delta API
        let response = self
            .cloud_provider
            .get_delta(delta_token.as_ref())
            .await
            .context("Failed to query delta API")?;

        let all_items = response.items;

        // Step 3: Save the new delta token to the account
        // The delta_link field contains the token for the next delta query
        if let Some(ref delta_link) = response.delta_link {
            let new_token = DeltaToken::try_from(delta_link.clone())
                .context("Failed to parse delta link as DeltaToken")?;
            let mut updated_account = account.clone();
            updated_account.update_delta_token(new_token);
            self.state_repository
                .save_account(&updated_account)
                .await
                .context("Failed to persist updated delta token")?;
        }

        // Step 4: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success())
            .with_details(json!({
                "account_id": account.id().to_string(),
                "items_received": all_items.len(),
                "had_delta_token": delta_token.is_some(),
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record delta query audit entry")?;

        Ok(all_items)
    }

    /// Processes a single delta item into a SyncItem
    ///
    /// Based on the delta item type, this method:
    /// - **Created/Modified**: Creates a new SyncItem or updates an existing one
    /// - **Deleted**: Marks the corresponding SyncItem as Deleted
    ///
    /// # Arguments
    ///
    /// * `item` - The delta item to process
    /// * `account_id` - The account that owns the sync item
    /// * `account` - The account, used to derive the local sync root path
    ///
    /// # Returns
    ///
    /// The created or updated SyncItem
    ///
    /// # Errors
    ///
    /// Returns an error if SyncItem creation or state update fails
    pub async fn handle_delta_item(
        &self,
        item: &DeltaItem,
        _account_id: &AccountId,
        account: &Account,
    ) -> Result<SyncItem> {
        // Convert the string ID to a RemoteId newtype
        let remote_id = RemoteId::try_from(item.id.clone())
            .context("Failed to parse delta item ID as RemoteId")?;

        // Try to find an existing sync item by remote ID
        let existing = self
            .state_repository
            .get_item_by_remote_id(&remote_id)
            .await
            .context("Failed to look up existing sync item by remote ID")?;

        let sync_item = if item.is_deleted {
            // Handle deletion
            match existing {
                Some(mut existing_item) => {
                    existing_item
                        .mark_deleted()
                        .context("Invalid state transition to Deleted for delta item")?;
                    existing_item
                }
                None => {
                    // Item was already deleted or never tracked locally - create a placeholder
                    let (local_path, remote_path) = self.derive_paths(item, account)?;
                    let modified = item.modified.unwrap_or_else(Utc::now);
                    let content_hash = self.parse_content_hash(item)?;

                    let mut new_item = SyncItem::from_remote(
                        local_path,
                        remote_path,
                        remote_id.clone(),
                        item.is_directory,
                        item.size.unwrap_or(0),
                        content_hash,
                        modified,
                    )
                    .context("Failed to create SyncItem from deleted delta item")?;

                    new_item
                        .mark_deleted()
                        .context("Failed to mark new item as Deleted")?;
                    new_item
                }
            }
        } else {
            // Handle creation or modification
            match existing {
                Some(mut existing_item) => {
                    // Update existing item with new remote metadata
                    if let Some(modified) = item.modified {
                        existing_item.set_last_modified_remote(modified);
                    }
                    existing_item.set_size_bytes(item.size.unwrap_or(0));
                    if let Some(hash) = self.parse_content_hash(item)? {
                        existing_item.set_content_hash(hash);
                    }
                    existing_item
                }
                None => {
                    // Create new SyncItem from delta data
                    let (local_path, remote_path) = self.derive_paths(item, account)?;
                    let modified = item.modified.unwrap_or_else(Utc::now);
                    let content_hash = self.parse_content_hash(item)?;

                    SyncItem::from_remote(
                        local_path,
                        remote_path,
                        remote_id.clone(),
                        item.is_directory,
                        item.size.unwrap_or(0),
                        content_hash,
                        modified,
                    )
                    .context("Failed to create SyncItem from delta item")?
                }
            }
        };

        // Persist the sync item
        self.state_repository
            .save_item(&sync_item)
            .await
            .context("Failed to persist sync item from delta")?;

        Ok(sync_item)
    }

    /// Derives local and remote paths from a DeltaItem
    ///
    /// Maps the cloud-relative path to a local absolute path under the
    /// account's sync root, and to a `RemotePath` newtype.
    fn derive_paths(&self, item: &DeltaItem, account: &Account) -> Result<(SyncPath, RemotePath)> {
        // Use the item's cloud path, falling back to just the name
        let cloud_path = item.path.as_deref().unwrap_or(&item.name);

        // Build the remote path (must start with /)
        let remote_path_str = if cloud_path.starts_with('/') {
            cloud_path.to_owned()
        } else {
            format!("/{cloud_path}")
        };
        let remote_path = RemotePath::try_from(remote_path_str)
            .context("Failed to parse remote path from delta item")?;

        // Build the local path under the account's sync root
        // Strip leading '/' from cloud path to make it relative for joining
        let relative = cloud_path.trim_start_matches('/');
        let local_pathbuf: PathBuf = account.sync_root().as_path().join(relative);
        let local_path = SyncPath::try_from(local_pathbuf)
            .context("Failed to construct local SyncPath from delta item")?;

        Ok((local_path, remote_path))
    }

    /// Parses the optional hash string from a DeltaItem into an optional FileHash
    fn parse_content_hash(&self, item: &DeltaItem) -> Result<Option<FileHash>> {
        match item.hash {
            Some(ref h) => {
                let file_hash = FileHash::try_from(h.clone())
                    .context("Failed to parse content hash from delta item")?;
                Ok(Some(file_hash))
            }
            None => Ok(None),
        }
    }
}
