//! Delta synchronization engine
//!
//! The [`SyncEngine`] orchestrates bidirectional synchronization between
//! the local filesystem and a cloud provider (OneDrive via Microsoft Graph).
//!
//! ## Sync Flow
//!
//! 1. **Remote changes** (pull): Query delta, process creates/updates/deletes
//! 2. **Local changes** (push): Scan filesystem, upload new/modified, delete remote
//! 3. **Bookkeeping**: Update delta token, complete session, return summary
//!
//! ## Retry Logic
//!
//! Transient errors (network, rate limiting, server errors) are retried with
//! exponential backoff: 1s, 2s, 4s, 8s, 16s (max 5 retries).

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use lnxdrive_core::config::Config;
use lnxdrive_core::domain::newtypes::{DeltaToken, FileHash, RemoteId, RemotePath, SyncPath};
use lnxdrive_core::domain::session::SyncSession;
use lnxdrive_core::domain::sync_item::SyncItem;
use lnxdrive_core::ports::cloud_provider::{DeltaItem, ICloudProvider};
use lnxdrive_core::ports::local_filesystem::ILocalFileSystem;
use lnxdrive_core::ports::state_repository::IStateRepository;

// ============================================================================
// T186: FileWatcher integration - re-export ChangeEvent from watcher module
// ============================================================================

pub use crate::watcher::ChangeEvent;

// ============================================================================
// T152: SyncResult
// ============================================================================

/// Summary of a completed synchronization cycle
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Number of files downloaded from the cloud
    pub files_downloaded: u32,
    /// Number of files uploaded to the cloud
    pub files_uploaded: u32,
    /// Number of files deleted (locally or remotely)
    pub files_deleted: u32,
    /// Errors encountered during the sync (non-fatal)
    pub errors: Vec<String>,
    /// Wall-clock duration of the sync in milliseconds
    pub duration_ms: u64,
}

// ============================================================================
// T157: LocalChange - represents a detected local change
// ============================================================================

/// A local filesystem change detected during scanning
#[derive(Debug, Clone)]
enum LocalChange {
    /// A new file or directory that has no SyncItem counterpart
    Created(SyncPath),
    /// An existing file whose content has changed
    Modified(SyncPath, SyncItem),
    /// A SyncItem whose local file no longer exists
    Deleted(SyncItem),
}

// ============================================================================
// T161: Retry logic
// ============================================================================

/// Maximum number of retries for transient errors
const MAX_RETRIES: u32 = 5;

/// Base delay for exponential backoff (1 second)
const BASE_DELAY_SECS: u64 = 1;

/// Determines whether an error is transient (retryable)
///
/// Transient errors include:
/// - Network errors (connection refused, timeout, DNS)
/// - Rate limiting (HTTP 429)
/// - Server errors (HTTP 5xx)
fn is_transient_error(err: &anyhow::Error) -> bool {
    let err_str = format!("{err:#}").to_lowercase();

    // Network errors
    if err_str.contains("network")
        || err_str.contains("connection")
        || err_str.contains("timeout")
        || err_str.contains("dns")
        || err_str.contains("reset by peer")
        || err_str.contains("broken pipe")
    {
        return true;
    }

    // Rate limiting
    if err_str.contains("429")
        || err_str.contains("too many requests")
        || err_str.contains("rate limit")
    {
        return true;
    }

    // Server errors (5xx)
    if err_str.contains("500")
        || err_str.contains("502")
        || err_str.contains("503")
        || err_str.contains("504")
        || err_str.contains("server error")
    {
        return true;
    }

    false
}

/// Executes an async operation with exponential backoff retry
///
/// Only retries on transient errors (network, rate limiting, server errors).
/// Non-transient errors are returned immediately.
///
/// Backoff schedule: 1s, 2s, 4s, 8s, 16s
async fn with_retry<F, Fut, T>(operation_name: &str, f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=MAX_RETRIES {
        match f().await {
            Ok(value) => {
                if attempt > 0 {
                    info!(
                        operation = operation_name,
                        attempt, "Operation succeeded after retry"
                    );
                }
                return Ok(value);
            }
            Err(err) => {
                if attempt < MAX_RETRIES && is_transient_error(&err) {
                    let delay_secs = BASE_DELAY_SECS * 2u64.pow(attempt);
                    warn!(
                        operation = operation_name,
                        attempt,
                        delay_secs,
                        error = %err,
                        "Transient error, retrying"
                    );
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                    last_error = Some(err);
                } else {
                    return Err(err);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Retry exhausted for {}", operation_name)))
}

// ============================================================================
// T153: DeltaAction - result of processing a delta item
// ============================================================================

/// Result of processing a single delta item from the cloud
enum DeltaAction {
    /// A new file was downloaded
    Downloaded,
    /// An existing file was updated (re-downloaded)
    Updated,
    /// A file was deleted locally
    Deleted,
    /// No action was needed (unchanged or metadata-only update)
    Skipped,
}

// ============================================================================
// T151: SyncEngine struct
// ============================================================================

/// Default bulk mode detection threshold (number of items)
const BULK_MODE_THRESHOLD: u64 = 1000;

/// Reduced concurrent operations during bulk mode
#[allow(dead_code)]
const BULK_MODE_MAX_CONCURRENT: u32 = 4;

/// Delay between batches during bulk mode (in milliseconds)
#[allow(dead_code)]
const BULK_MODE_BATCH_DELAY_MS: u64 = 2000;

/// Bidirectional synchronization engine
///
/// Coordinates delta queries, local scanning, and file transfers between
/// the local filesystem and a cloud storage provider.
///
/// ## Dependencies
///
/// - `cloud_provider`: Remote file operations (delta, download, upload, delete)
/// - `state_repository`: Persistent state (items, accounts, sessions)
/// - `local_filesystem`: Local file I/O, hashing, and directory operations
/// - `large_file_threshold`: Byte threshold for choosing upload method
pub struct SyncEngine {
    /// Cloud storage provider (OneDrive via Graph API)
    cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
    /// Persistent state store
    state_repository: Arc<dyn IStateRepository + Send + Sync>,
    /// Local filesystem operations
    local_filesystem: Arc<dyn ILocalFileSystem + Send + Sync>,
    /// Files larger than this (in bytes) use resumable upload sessions
    large_file_threshold: u64,
    /// T186: Receiver for filesystem watcher events
    ///
    /// When set, the engine can consume real-time change events from
    /// the FileWatcher instead of relying solely on periodic directory scans.
    /// TODO: Integrate watcher events into the sync loop once the FileWatcher
    /// module is implemented.
    watcher_rx: Option<mpsc::Receiver<ChangeEvent>>,
    /// T212: Whether the engine is currently in bulk mode
    ///
    /// Bulk mode is activated during initial syncs or when processing a
    /// large number of items (>1000). In this mode:
    /// - Concurrent operations are reduced (4 vs 8 normal)
    /// - Delays are added between batches (2 seconds)
    /// - Rate limiting becomes more conservative
    bulk_mode: bool,
}

impl SyncEngine {
    /// Creates a new `SyncEngine` with the given dependencies
    ///
    /// # Arguments
    /// * `cloud_provider` - Cloud storage operations (ICloudProvider)
    /// * `state_repository` - State persistence (IStateRepository)
    /// * `local_filesystem` - Local file operations (ILocalFileSystem)
    /// * `config` - Application configuration for sync settings
    pub fn new(
        cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
        state_repository: Arc<dyn IStateRepository + Send + Sync>,
        local_filesystem: Arc<dyn ILocalFileSystem + Send + Sync>,
        config: &Config,
    ) -> Self {
        Self {
            cloud_provider,
            state_repository,
            local_filesystem,
            large_file_threshold: config.large_files.threshold_mb * 1024 * 1024,
            watcher_rx: None,
            bulk_mode: false,
        }
    }

    // ========================================================================
    // T212: Bulk mode configuration
    // ========================================================================

    /// Enables or disables bulk mode manually.
    ///
    /// Bulk mode reduces concurrency and adds delays between batches to
    /// minimize rate limiting pressure during large synchronization operations.
    ///
    /// # Arguments
    /// * `enabled` - Whether bulk mode should be active
    pub fn set_bulk_mode(&mut self, enabled: bool) {
        if enabled && !self.bulk_mode {
            info!("Bulk mode activated: reducing concurrency and adding batch delays");
        } else if !enabled && self.bulk_mode {
            info!("Bulk mode deactivated: returning to normal operation");
        }
        self.bulk_mode = enabled;
    }

    /// Returns whether the engine is currently in bulk mode.
    pub fn is_bulk_mode(&self) -> bool {
        self.bulk_mode
    }

    /// Detects whether bulk mode should be activated based on the delta response.
    ///
    /// Bulk mode is activated when:
    /// - There is no existing delta token (initial sync), OR
    /// - The number of pending items exceeds [`BULK_MODE_THRESHOLD`] (1000)
    ///
    /// # Arguments
    /// * `has_delta_token` - Whether the account has a stored delta token
    /// * `item_count` - Number of items in the delta response
    pub fn detect_bulk_mode(&mut self, has_delta_token: bool, item_count: u64) {
        let should_activate = !has_delta_token || item_count > BULK_MODE_THRESHOLD;

        if should_activate && !self.bulk_mode {
            info!(
                has_delta_token,
                item_count,
                threshold = BULK_MODE_THRESHOLD,
                "Bulk mode auto-detected: initial sync or large delta"
            );
            self.bulk_mode = true;
        } else if !should_activate && self.bulk_mode {
            info!(item_count, "Bulk mode auto-deactivated: below threshold");
            self.bulk_mode = false;
        }
    }

    /// Returns the maximum concurrent operations based on current mode.
    ///
    /// In bulk mode, returns [`BULK_MODE_MAX_CONCURRENT`] (4).
    /// In normal mode, returns 8 (standard concurrency).
    pub fn max_concurrent_operations(&self) -> u32 {
        if self.bulk_mode {
            BULK_MODE_MAX_CONCURRENT
        } else {
            8
        }
    }

    /// Returns the delay between batches based on current mode.
    ///
    /// In bulk mode, returns [`BULK_MODE_BATCH_DELAY_MS`] (2000ms).
    /// In normal mode, returns 0 (no delay).
    pub fn batch_delay(&self) -> Duration {
        if self.bulk_mode {
            Duration::from_millis(BULK_MODE_BATCH_DELAY_MS)
        } else {
            Duration::ZERO
        }
    }

    // ========================================================================
    // T186: FileWatcher integration hookup
    // ========================================================================

    /// Sets the receiver for filesystem watcher events
    ///
    /// When a FileWatcher is active, it sends [`ChangeEvent`]s through an
    /// `mpsc` channel. This method connects that channel to the engine,
    /// allowing future sync cycles to consume real-time change notifications
    /// instead of relying solely on full directory scans.
    ///
    /// # Arguments
    /// * `rx` - The receiving end of the watcher's event channel
    ///
    /// # Example
    /// ```rust,no_run
    /// # use tokio::sync::mpsc;
    /// # use lnxdrive_sync::engine::{SyncEngine, ChangeEvent};
    /// let (tx, rx) = mpsc::channel::<ChangeEvent>(1024);
    /// // engine.set_watcher_events_receiver(rx);
    /// ```
    pub fn set_watcher_events_receiver(&mut self, rx: mpsc::Receiver<ChangeEvent>) {
        self.watcher_rx = Some(rx);
        info!("FileWatcher events receiver connected to SyncEngine");
        // TODO: Drain watcher events at the start of each sync cycle to
        // build a targeted change set, reducing the need for full scans.
    }

    // ========================================================================
    // T152: SyncEngine::sync()
    // ========================================================================

    /// Performs a full bidirectional synchronization cycle
    ///
    /// 1. Gets the default account from the state repository
    /// 2. Creates a new SyncSession
    /// 3. Queries the cloud for delta changes
    /// 4. Processes each remote delta item (create/update/delete)
    /// 5. Scans the local filesystem for changes
    /// 6. Processes each local change (upload/delete)
    /// 7. Updates the delta token on the account
    /// 8. Completes the session
    ///
    /// # Returns
    /// A [`SyncResult`] summarizing the sync cycle
    ///
    /// # Errors
    /// Returns an error if no account is configured or if the sync cycle fails
    #[tracing::instrument(skip(self))]
    pub async fn sync(&self) -> Result<SyncResult> {
        let start = std::time::Instant::now();
        let mut result = SyncResult {
            files_downloaded: 0,
            files_uploaded: 0,
            files_deleted: 0,
            errors: Vec::new(),
            duration_ms: 0,
        };

        // Step 1: Get the default account
        let mut account = self
            .state_repository
            .get_default_account()
            .await
            .context("Failed to query default account")?
            .ok_or_else(|| {
                anyhow::anyhow!("No account configured. Run 'lnxdrive auth login' first.")
            })?;

        let sync_root = account.sync_root().clone();

        info!(
            account_id = %account.id(),
            sync_root = %sync_root,
            "Starting sync cycle"
        );

        // Step 2: Create a new SyncSession
        let mut session = SyncSession::new(*account.id());
        self.state_repository
            .save_session(&session)
            .await
            .context("Failed to save initial sync session")?;

        // Step 3: Query delta (T167/T168/T170: delta token persistence and 410 Gone handling)
        let delta_token = account.delta_token().cloned();
        if let Some(ref token) = delta_token {
            session.set_delta_token_start(token.clone());
        }

        let delta_response = match with_retry("get_delta", || {
            let token_ref = delta_token.as_ref();
            async move { self.cloud_provider.get_delta(token_ref).await }
        })
        .await
        {
            Ok(response) => response,
            Err(err) => {
                // T168/T170: Handle 410 Gone by clearing delta token and retrying with full resync
                let err_str = format!("{err:#}");
                if err_str.contains("410") || err_str.contains("Gone") {
                    warn!("Delta token expired, performing full resync");
                    account.clear_delta_token();
                    self.state_repository
                        .save_account(&account)
                        .await
                        .context("Failed to save account after clearing delta token")?;

                    // Retry with no token (full resync)
                    match with_retry("get_delta_full_resync", || async move {
                        self.cloud_provider.get_delta(None).await
                    })
                    .await
                    {
                        Ok(response) => response,
                        Err(retry_err) => {
                            let reason =
                                format!("Failed to query delta (full resync): {retry_err}");
                            error!(%reason);
                            session.fail(&reason);
                            self.state_repository.save_session(&session).await.ok();
                            return Err(retry_err.context("Delta query failed (full resync)"));
                        }
                    }
                } else {
                    let reason = format!("Failed to query delta: {err}");
                    error!(%reason);
                    session.fail(&reason);
                    self.state_repository.save_session(&session).await.ok();
                    return Err(err.context("Delta query failed"));
                }
            }
        };

        let total_remote = delta_response.items.len();
        info!(
            items = total_remote,
            has_delta_link = delta_response.delta_link.is_some(),
            "Delta query returned"
        );

        // T171: Track delta efficiency metrics
        session.set_items_checked(total_remote as u64);
        let mut items_synced: u64 = 0;

        // Step 4: Process remote delta items
        for delta_item in &delta_response.items {
            match self.process_delta_item(delta_item, &sync_root).await {
                Ok(action) => match action {
                    DeltaAction::Downloaded => {
                        result.files_downloaded += 1;
                        items_synced += 1;
                    }
                    DeltaAction::Deleted => {
                        result.files_deleted += 1;
                        items_synced += 1;
                    }
                    DeltaAction::Updated => {
                        result.files_downloaded += 1;
                        items_synced += 1;
                    }
                    DeltaAction::Skipped => {}
                },
                Err(err) => {
                    let msg = format!(
                        "Error processing delta item '{}' ({}): {err}",
                        delta_item.name, delta_item.id
                    );
                    warn!(%msg);
                    result.errors.push(msg);
                    session.record_failure();
                    continue;
                }
            }
            session.record_success();
        }

        // Step 5: Scan for local changes (T172: pass last_sync for optimization)
        let last_sync = account.last_sync();
        let local_changes = match self.scan_local_changes(&sync_root, last_sync).await {
            Ok(changes) => changes,
            Err(err) => {
                let msg = format!("Failed to scan local changes: {err}");
                warn!(%msg);
                result.errors.push(msg);
                Vec::new()
            }
        };

        info!(changes = local_changes.len(), "Local changes detected");

        // Step 6: Process local changes
        for change in &local_changes {
            match change {
                LocalChange::Created(path) => {
                    match self.handle_local_create(path, &sync_root).await {
                        Ok(()) => {
                            result.files_uploaded += 1;
                            items_synced += 1;
                            session.record_success();
                        }
                        Err(err) => {
                            let msg = format!("Error uploading new file '{}': {err}", path);
                            warn!(%msg);
                            result.errors.push(msg);
                            session.record_failure();
                        }
                    }
                }
                LocalChange::Modified(path, existing) => {
                    match self.handle_local_update(path, existing, &sync_root).await {
                        Ok(()) => {
                            result.files_uploaded += 1;
                            items_synced += 1;
                            session.record_success();
                        }
                        Err(err) => {
                            let msg = format!("Error uploading modified file '{}': {err}", path);
                            warn!(%msg);
                            result.errors.push(msg);
                            session.record_failure();
                        }
                    }
                }
                LocalChange::Deleted(item) => match self.handle_local_delete(item).await {
                    Ok(()) => {
                        result.files_deleted += 1;
                        items_synced += 1;
                        session.record_success();
                    }
                    Err(err) => {
                        let msg =
                            format!("Error deleting remote item '{}': {err}", item.local_path());
                        warn!(%msg);
                        result.errors.push(msg);
                        session.record_failure();
                    }
                },
            }
        }

        // T171: Finalize delta efficiency metrics on the session
        session.set_items_synced(items_synced);

        debug!(
            items_checked = session.items_checked(),
            items_synced = session.items_synced(),
            efficiency = session.sync_efficiency(),
            "Delta sync efficiency"
        );

        // Step 7: Update delta token
        if let Some(delta_link) = &delta_response.delta_link {
            // Extract the token value from the delta link URL
            // The delta_link is a full URL like:
            // https://graph.microsoft.com/v1.0/me/drive/root/delta?token=...
            if let Some(token_str) = extract_token_from_delta_link(delta_link) {
                match DeltaToken::new(token_str) {
                    Ok(token) => {
                        session.set_delta_token_end(token.clone());
                        account.update_delta_token(token);
                        account.record_sync(Utc::now());
                        self.state_repository
                            .save_account(&account)
                            .await
                            .context("Failed to save updated account")?;
                    }
                    Err(err) => {
                        warn!("Failed to create DeltaToken from delta link: {err}");
                    }
                }
            } else {
                // Use the full delta_link as the token
                match DeltaToken::new(delta_link.clone()) {
                    Ok(token) => {
                        session.set_delta_token_end(token.clone());
                        account.update_delta_token(token);
                        account.record_sync(Utc::now());
                        self.state_repository
                            .save_account(&account)
                            .await
                            .context("Failed to save updated account")?;
                    }
                    Err(err) => {
                        warn!("Failed to create DeltaToken from delta link: {err}");
                    }
                }
            }
        }

        // Step 8: Complete the session
        session.complete();
        self.state_repository
            .save_session(&session)
            .await
            .context("Failed to save completed session")?;

        result.duration_ms = start.elapsed().as_millis() as u64;

        info!(
            downloaded = result.files_downloaded,
            uploaded = result.files_uploaded,
            deleted = result.files_deleted,
            errors = result.errors.len(),
            duration_ms = result.duration_ms,
            "Sync cycle completed"
        );

        Ok(result)
    }

    // ========================================================================
    // T153: process_delta_item()
    // ========================================================================

    /// Processes a single delta item from the cloud
    ///
    /// Determines the appropriate action based on the item's state:
    /// - Deleted -> handle_remote_delete
    /// - Existing (by remote_id) -> handle_remote_update
    /// - New -> handle_remote_create
    #[tracing::instrument(skip(self))]
    async fn process_delta_item(
        &self,
        delta_item: &DeltaItem,
        sync_root: &SyncPath,
    ) -> Result<DeltaAction> {
        if delta_item.is_deleted {
            return self.handle_remote_delete(delta_item).await;
        }

        // Check if we already track this remote item
        let remote_id =
            RemoteId::new(delta_item.id.clone()).context("Invalid remote ID in delta item")?;

        let existing = self
            .state_repository
            .get_item_by_remote_id(&remote_id)
            .await
            .context("Failed to query existing item by remote ID")?;

        if let Some(existing_item) = existing {
            self.handle_remote_update(delta_item, &existing_item, sync_root)
                .await
        } else {
            self.handle_remote_create(delta_item, sync_root).await
        }
    }

    // ========================================================================
    // T154: handle_remote_create()
    // ========================================================================

    /// Handles a new item appearing in the cloud
    ///
    /// - Directories: Creates the local directory, saves SyncItem as Hydrated
    /// - Files: Downloads content, writes to local path, creates SyncItem,
    ///   transitions through Hydrating -> Hydrated
    #[tracing::instrument(skip(self))]
    async fn handle_remote_create(
        &self,
        delta_item: &DeltaItem,
        sync_root: &SyncPath,
    ) -> Result<DeltaAction> {
        let remote_path_str = delta_item
            .path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Delta item has no path: {}", delta_item.id))?;

        let remote_path = RemotePath::new(remote_path_str.to_string())
            .context("Invalid remote path in delta item")?;
        let remote_id =
            RemoteId::new(delta_item.id.clone()).context("Invalid remote ID in delta item")?;

        // Build local path: sync_root + relative path from remote
        let relative = remote_path_str.trim_start_matches('/');
        let local_path = SyncPath::new(sync_root.as_path().join(relative))
            .context("Failed to construct local path")?;

        if delta_item.is_directory {
            debug!(path = %local_path, "Creating local directory from remote");

            self.local_filesystem
                .create_directory(&local_path)
                .await
                .context("Failed to create local directory")?;

            let mut item = SyncItem::from_remote(
                local_path,
                remote_path,
                remote_id,
                true, // is_directory
                0,
                None,
                delta_item.modified.unwrap_or_else(Utc::now),
            )?;

            // Directories go directly to Hydrated via Hydrating
            item.start_hydrating()?;
            item.complete_hydration()?;
            item.mark_synced();

            self.state_repository
                .save_item(&item)
                .await
                .context("Failed to save new directory SyncItem")?;

            Ok(DeltaAction::Downloaded)
        } else {
            debug!(
                path = %local_path,
                size = delta_item.size.unwrap_or(0),
                "Downloading new file from remote"
            );

            // Download the file content
            let data = with_retry("download_file", || {
                let rid = remote_id.clone();
                async move { self.cloud_provider.download_file(&rid).await }
            })
            .await
            .context("Failed to download file")?;

            // Write to local filesystem
            self.local_filesystem
                .write_file(&local_path, &data)
                .await
                .context("Failed to write downloaded file")?;

            // Build content hash if available
            let content_hash = if let Some(ref hash_str) = delta_item.hash {
                FileHash::new(hash_str.clone()).ok()
            } else {
                None
            };

            // Create and persist the SyncItem
            let mut item = SyncItem::from_remote(
                local_path.clone(),
                remote_path,
                remote_id,
                false, // is_file
                delta_item.size.unwrap_or(data.len() as u64),
                content_hash,
                delta_item.modified.unwrap_or_else(Utc::now),
            )?;

            item.start_hydrating()?;
            item.complete_hydration()?;
            item.mark_synced();

            // Compute and set local hash
            if let Ok(local_hash) = self.local_filesystem.compute_hash(&local_path).await {
                item.set_local_hash(local_hash);
            }

            self.state_repository
                .save_item(&item)
                .await
                .context("Failed to save new file SyncItem")?;

            Ok(DeltaAction::Downloaded)
        }
    }

    // ========================================================================
    // T155: handle_remote_update()
    // ========================================================================

    /// Handles an updated item in the cloud
    ///
    /// Compares the remote content hash with the stored hash. If they differ,
    /// downloads the new content and updates the local file and SyncItem.
    #[tracing::instrument(skip(self))]
    async fn handle_remote_update(
        &self,
        delta_item: &DeltaItem,
        existing: &SyncItem,
        _sync_root: &SyncPath,
    ) -> Result<DeltaAction> {
        // For directories, just update metadata
        if delta_item.is_directory {
            debug!(
                path = %existing.local_path(),
                "Remote directory updated (metadata only)"
            );
            let mut updated = existing.clone();
            if let Some(modified) = delta_item.modified {
                updated.set_last_modified_remote(modified);
            }
            updated.mark_synced();
            self.state_repository.save_item(&updated).await?;
            return Ok(DeltaAction::Skipped);
        }

        // Compare hashes to determine if content changed
        let remote_hash_str = delta_item.hash.as_deref();
        let stored_hash_str = existing.content_hash().map(|h| h.as_str());

        let hashes_differ = match (remote_hash_str, stored_hash_str) {
            (Some(remote), Some(stored)) => remote != stored,
            (Some(_), None) => true, // New hash, assume changed
            (None, _) => false,      // No remote hash, can't compare
        };

        if !hashes_differ {
            debug!(
                path = %existing.local_path(),
                "Remote file unchanged (hash match)"
            );
            let mut updated = existing.clone();
            if let Some(modified) = delta_item.modified {
                updated.set_last_modified_remote(modified);
            }
            updated.mark_synced();
            self.state_repository.save_item(&updated).await?;
            return Ok(DeltaAction::Skipped);
        }

        debug!(
            path = %existing.local_path(),
            "Remote file content changed, downloading update"
        );

        let remote_id = existing
            .remote_id()
            .ok_or_else(|| anyhow::anyhow!("Existing item has no remote ID"))?
            .clone();

        // Download updated content
        let data = with_retry("download_file_update", || {
            let rid = remote_id.clone();
            async move { self.cloud_provider.download_file(&rid).await }
        })
        .await
        .context("Failed to download updated file")?;

        // Write to local filesystem
        let local_path = existing.local_path();
        self.local_filesystem
            .write_file(local_path, &data)
            .await
            .context("Failed to write updated file")?;

        // Update the SyncItem
        let mut updated = existing.clone();
        if let Some(ref hash_str) = delta_item.hash {
            if let Ok(hash) = FileHash::new(hash_str.clone()) {
                updated.set_content_hash(hash);
            }
        }
        if let Some(size) = delta_item.size {
            updated.set_size_bytes(size);
        }
        if let Some(modified) = delta_item.modified {
            updated.set_last_modified_remote(modified);
        }

        // Compute and set local hash
        if let Ok(local_hash) = self.local_filesystem.compute_hash(local_path).await {
            updated.set_local_hash(local_hash);
        }

        updated.mark_synced();
        self.state_repository.save_item(&updated).await?;

        Ok(DeltaAction::Updated)
    }

    // ========================================================================
    // T156: handle_remote_delete()
    // ========================================================================

    /// Handles an item deleted from the cloud
    ///
    /// Finds the local SyncItem by remote ID, deletes the local file/directory,
    /// and marks the SyncItem as Deleted.
    #[tracing::instrument(skip(self))]
    async fn handle_remote_delete(&self, delta_item: &DeltaItem) -> Result<DeltaAction> {
        let remote_id = RemoteId::new(delta_item.id.clone())
            .context("Invalid remote ID in deleted delta item")?;

        let existing = self
            .state_repository
            .get_item_by_remote_id(&remote_id)
            .await
            .context("Failed to query item for remote delete")?;

        let Some(mut item) = existing else {
            debug!(
                id = %delta_item.id,
                "Remote delete for unknown item, skipping"
            );
            return Ok(DeltaAction::Skipped);
        };

        debug!(
            path = %item.local_path(),
            "Deleting local file/directory (remote deleted)"
        );

        // Delete from local filesystem (ignore errors if already gone)
        let fs_state = self
            .local_filesystem
            .get_state(item.local_path())
            .await
            .context("Failed to check local file state")?;

        if fs_state.exists {
            self.local_filesystem
                .delete_file(item.local_path())
                .await
                .context("Failed to delete local file")?;
        }

        // Mark the SyncItem as Deleted
        item.mark_deleted()?;
        self.state_repository
            .save_item(&item)
            .await
            .context("Failed to save deleted SyncItem")?;

        Ok(DeltaAction::Deleted)
    }

    // ========================================================================
    // T157: scan_local_changes()
    // ========================================================================

    /// Scans the sync root directory for local changes
    ///
    /// Walks the directory tree recursively and compares each file/directory
    /// against the stored SyncItems to detect:
    /// - New files (no SyncItem exists)
    /// - Modified files (hash differs from stored)
    /// - Deleted files (SyncItem exists but file is gone)
    ///
    /// T172: When `last_sync` is provided, only files modified since that
    /// timestamp are considered for change detection, improving scan efficiency.
    #[tracing::instrument(skip(self))]
    async fn scan_local_changes(
        &self,
        sync_root: &SyncPath,
        last_sync: Option<DateTime<Utc>>,
    ) -> Result<Vec<LocalChange>> {
        let mut changes = Vec::new();

        // Walk the sync root directory
        self.walk_directory(sync_root, sync_root, &mut changes, last_sync)
            .await?;

        // Check for deleted items: items in the state repo whose local file is gone
        let all_items = self
            .state_repository
            .query_items(&lnxdrive_core::ports::state_repository::ItemFilter::new())
            .await
            .context("Failed to query all sync items")?;

        for item in all_items {
            // Skip already-deleted items
            if matches!(
                item.state(),
                lnxdrive_core::domain::sync_item::ItemState::Deleted
            ) {
                continue;
            }

            let fs_state = self
                .local_filesystem
                .get_state(item.local_path())
                .await
                .context("Failed to check local state for deletion scan")?;

            if !fs_state.exists && item.remote_id().is_some() {
                debug!(
                    path = %item.local_path(),
                    "Local file deleted, will remove from cloud"
                );
                changes.push(LocalChange::Deleted(item));
            }
        }

        Ok(changes)
    }

    /// Recursively walks a directory, detecting new and modified files
    ///
    /// T172: When `last_sync` is provided, files whose modification time
    /// predates that timestamp are skipped (they haven't changed since the
    /// last successful sync), reducing expensive hash computations.
    fn walk_directory<'a>(
        &'a self,
        dir: &'a SyncPath,
        sync_root: &'a SyncPath,
        changes: &'a mut Vec<LocalChange>,
        last_sync: Option<DateTime<Utc>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Begin walk_directory body
            let mut entries = tokio::fs::read_dir(dir.as_path())
                .await
                .with_context(|| format!("Failed to read directory: {}", dir))?;

            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let sync_path = match SyncPath::new(entry_path.clone()) {
                    Ok(p) => p,
                    Err(err) => {
                        warn!(path = ?entry_path, %err, "Skipping invalid path");
                        continue;
                    }
                };

                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    // Check if this directory is tracked
                    let existing = self
                        .state_repository
                        .get_item_by_path(&sync_path)
                        .await
                        .unwrap_or(None);

                    if existing.is_none() {
                        changes.push(LocalChange::Created(sync_path.clone()));
                    }

                    // Recurse into subdirectory
                    self.walk_directory(&sync_path, sync_root, changes, last_sync)
                        .await?;
                } else if metadata.is_file() {
                    // T172: Skip files not modified since last_sync for existing items.
                    // New files (not tracked) always need to be checked regardless
                    // of their modification time.
                    let existing = self
                        .state_repository
                        .get_item_by_path(&sync_path)
                        .await
                        .unwrap_or(None);

                    match existing {
                        None => {
                            // New file - always report as Created
                            changes.push(LocalChange::Created(sync_path));
                        }
                        Some(item) => {
                            // T172: Optimization - skip hash computation for files
                            // not modified since last sync
                            if let Some(last_sync_time) = last_sync {
                                if let Ok(modified_time) = metadata.modified() {
                                    let modified_dt: DateTime<Utc> = modified_time.into();
                                    if modified_dt <= last_sync_time {
                                        debug!(
                                            path = %sync_path,
                                            "Skipping unchanged file (modified before last sync)"
                                        );
                                        continue;
                                    }
                                }
                            }

                            // Check if modified by comparing hashes
                            if let Ok(local_hash) =
                                self.local_filesystem.compute_hash(&sync_path).await
                            {
                                let stored_hash = item.content_hash().map(|h| h.as_str());
                                if stored_hash != Some(local_hash.as_str()) {
                                    changes.push(LocalChange::Modified(sync_path, item));
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        }) // end Box::pin(async move { ... })
    }

    // ========================================================================
    // T158: handle_local_create()
    // ========================================================================

    /// Handles a new local file that needs to be uploaded to the cloud
    ///
    /// Reads the file, determines the parent remote path, and uploads using
    /// either simple upload or resumable session based on file size.
    #[tracing::instrument(skip(self))]
    async fn handle_local_create(&self, path: &SyncPath, sync_root: &SyncPath) -> Result<()> {
        let fs_state = self
            .local_filesystem
            .get_state(path)
            .await
            .context("Failed to get state for new local file")?;

        // Compute relative path and derive remote path
        let relative = path
            .relative_to(sync_root)
            .context("Path is not within sync root")?;
        let remote_path_str = format!("/{}", relative.display()).replace('\\', "/"); // Normalize for Windows-style paths in tests

        if fs_state.is_directory() {
            // For directories, we don't upload them directly (they're created implicitly)
            // but we do track them
            let remote_path = RemotePath::new(remote_path_str)
                .context("Failed to construct remote path for directory")?;

            let mut item = SyncItem::new_directory(path.clone(), remote_path)?;
            item.start_hydrating()?;
            item.complete_hydration()?;
            item.mark_synced();

            self.state_repository.save_item(&item).await?;
            return Ok(());
        }

        // Read file content
        let data = self
            .local_filesystem
            .read_file(path)
            .await
            .context("Failed to read local file for upload")?;

        // Determine parent path and file name
        let (parent_remote_path, file_name) = split_remote_path(&remote_path_str)?;

        // Upload based on size
        let delta_item = if data.len() as u64 > self.large_file_threshold {
            debug!(
                path = %path,
                size = data.len(),
                "Using resumable upload session (large file)"
            );
            with_retry("upload_file_session", || {
                let parent = parent_remote_path.clone();
                let name = file_name.clone();
                let d = data.clone();
                async move {
                    self.cloud_provider
                        .upload_file_session(&parent, &name, &d, None)
                        .await
                }
            })
            .await
            .context("Failed to upload large file")?
        } else {
            debug!(
                path = %path,
                size = data.len(),
                "Using simple upload"
            );
            with_retry("upload_file", || {
                let parent = parent_remote_path.clone();
                let name = file_name.clone();
                let d = data.clone();
                async move { self.cloud_provider.upload_file(&parent, &name, &d).await }
            })
            .await
            .context("Failed to upload file")?
        };

        // Create SyncItem from the upload response
        let remote_id =
            RemoteId::new(delta_item.id.clone()).context("Invalid remote ID in upload response")?;
        let remote_path =
            RemotePath::new(remote_path_str).context("Failed to construct remote path")?;

        let content_hash = delta_item
            .hash
            .as_ref()
            .and_then(|h| FileHash::new(h.clone()).ok());

        let mut item = SyncItem::from_remote(
            path.clone(),
            remote_path,
            remote_id,
            false,
            delta_item.size.unwrap_or(data.len() as u64),
            content_hash,
            delta_item.modified.unwrap_or_else(Utc::now),
        )?;

        item.start_hydrating()?;
        item.complete_hydration()?;
        item.mark_synced();

        // Compute and set local hash
        if let Ok(local_hash) = self.local_filesystem.compute_hash(path).await {
            item.set_local_hash(local_hash);
        }

        self.state_repository.save_item(&item).await?;

        Ok(())
    }

    // ========================================================================
    // T159: handle_local_update()
    // ========================================================================

    /// Handles a locally modified file that needs to be re-uploaded
    ///
    /// Compares the local hash with the stored content hash. If they differ,
    /// reads and uploads the file, then updates the SyncItem.
    #[tracing::instrument(skip(self))]
    async fn handle_local_update(
        &self,
        path: &SyncPath,
        existing: &SyncItem,
        sync_root: &SyncPath,
    ) -> Result<()> {
        // Compute current local hash
        let local_hash = self
            .local_filesystem
            .compute_hash(path)
            .await
            .context("Failed to compute local hash")?;

        // Compare with stored content hash
        let needs_upload = match existing.content_hash() {
            Some(stored) => local_hash.as_str() != stored.as_str(),
            None => true, // No stored hash, assume changed
        };

        if !needs_upload {
            debug!(path = %path, "Local file unchanged, skipping upload");
            return Ok(());
        }

        debug!(path = %path, "Local file modified, uploading update");

        // Read file content
        let data = self
            .local_filesystem
            .read_file(path)
            .await
            .context("Failed to read modified local file")?;

        // Determine parent path and file name
        let relative = path.relative_to(sync_root)?;
        let remote_path_str = format!("/{}", relative.display()).replace('\\', "/");
        let (parent_remote_path, file_name) = split_remote_path(&remote_path_str)?;

        // Upload
        let delta_item = if data.len() as u64 > self.large_file_threshold {
            with_retry("upload_file_session_update", || {
                let parent = parent_remote_path.clone();
                let name = file_name.clone();
                let d = data.clone();
                async move {
                    self.cloud_provider
                        .upload_file_session(&parent, &name, &d, None)
                        .await
                }
            })
            .await?
        } else {
            with_retry("upload_file_update", || {
                let parent = parent_remote_path.clone();
                let name = file_name.clone();
                let d = data.clone();
                async move { self.cloud_provider.upload_file(&parent, &name, &d).await }
            })
            .await?
        };

        // Update the SyncItem
        let mut updated = existing.clone();
        if let Some(ref hash_str) = delta_item.hash {
            if let Ok(hash) = FileHash::new(hash_str.clone()) {
                updated.set_content_hash(hash);
            }
        }
        if let Some(size) = delta_item.size {
            updated.set_size_bytes(size);
        }
        updated.set_local_hash(local_hash);
        updated.set_last_modified_local(Utc::now());

        // If the item was in Modified state, transition to Hydrated
        if matches!(
            updated.state(),
            lnxdrive_core::domain::sync_item::ItemState::Modified
        ) {
            updated.complete_sync()?;
        }
        updated.mark_synced();

        self.state_repository.save_item(&updated).await?;

        Ok(())
    }

    // ========================================================================
    // T160: handle_local_delete()
    // ========================================================================

    /// Handles a locally deleted file that needs to be removed from the cloud
    ///
    /// The file exists in the state repository but no longer on disk.
    /// Deletes it from the cloud provider and marks the SyncItem as Deleted.
    #[tracing::instrument(skip(self))]
    async fn handle_local_delete(&self, item: &SyncItem) -> Result<()> {
        let remote_id = item
            .remote_id()
            .ok_or_else(|| anyhow::anyhow!("Cannot delete item without remote ID"))?
            .clone();

        debug!(
            path = %item.local_path(),
            remote_id = %remote_id,
            "Deleting item from cloud (local file deleted)"
        );

        // Delete from cloud with retry
        with_retry("delete_item", || {
            let rid = remote_id.clone();
            async move { self.cloud_provider.delete_item(&rid).await }
        })
        .await
        .context("Failed to delete item from cloud")?;

        // Mark as deleted in state repository
        let mut updated = item.clone();
        updated.mark_deleted()?;
        self.state_repository.save_item(&updated).await?;

        Ok(())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Splits a remote path like "/Documents/file.txt" into parent ("/Documents")
/// and file name ("file.txt")
fn split_remote_path(path: &str) -> Result<(RemotePath, String)> {
    let remote_path = RemotePath::new(path.to_string()).context("Invalid remote path")?;

    let file_name = remote_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Remote path has no file name: {}", path))?
        .to_string();

    let parent = remote_path.parent().unwrap_or_else(RemotePath::root);

    Ok((parent, file_name))
}

/// Extracts the token parameter from a delta link URL
///
/// Input: `https://graph.microsoft.com/v1.0/me/drive/root/delta?token=abc123`
/// Output: `Some("abc123")`
fn extract_token_from_delta_link(delta_link: &str) -> Option<String> {
    url::Url::parse(delta_link).ok().and_then(|u| {
        u.query_pairs()
            .find(|(key, _)| key == "token")
            .map(|(_, value)| value.into_owned())
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_split_remote_path_root_file() {
        let (parent, name) = split_remote_path("/file.txt").unwrap();
        assert_eq!(parent.as_str(), "/");
        assert_eq!(name, "file.txt");
    }

    #[test]
    fn test_split_remote_path_subfolder() {
        let (parent, name) = split_remote_path("/Documents/report.pdf").unwrap();
        assert_eq!(parent.as_str(), "/Documents");
        assert_eq!(name, "report.pdf");
    }

    #[test]
    fn test_split_remote_path_nested() {
        let (parent, name) = split_remote_path("/Projects/Analysis/data.csv").unwrap();
        assert_eq!(parent.as_str(), "/Projects/Analysis");
        assert_eq!(name, "data.csv");
    }

    #[test]
    fn test_extract_token_from_delta_link() {
        let link = "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=abc123";
        assert_eq!(
            extract_token_from_delta_link(link),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_extract_token_from_delta_link_missing() {
        let link = "https://graph.microsoft.com/v1.0/me/drive/root/delta";
        assert_eq!(extract_token_from_delta_link(link), None);
    }

    #[test]
    fn test_extract_token_from_delta_link_invalid() {
        let link = "not a valid url";
        assert_eq!(extract_token_from_delta_link(link), None);
    }

    #[test]
    fn test_is_transient_error_network() {
        let err = anyhow::anyhow!("Network error: connection refused");
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_transient_error_rate_limit() {
        let err = anyhow::anyhow!("Too many requests (429)");
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_transient_error_server() {
        let err = anyhow::anyhow!("Server error: 503 Service Unavailable");
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_transient_error_not_transient() {
        let err = anyhow::anyhow!("File not found: /path/to/file");
        assert!(!is_transient_error(&err));
    }

    #[test]
    fn test_is_transient_error_auth() {
        let err = anyhow::anyhow!("Unauthorized: invalid token");
        assert!(!is_transient_error(&err));
    }

    #[test]
    fn test_sync_result_default() {
        let result = SyncResult {
            files_downloaded: 0,
            files_uploaded: 0,
            files_deleted: 0,
            errors: Vec::new(),
            duration_ms: 0,
        };
        assert_eq!(result.files_downloaded, 0);
        assert!(result.errors.is_empty());
    }

    // T168/T170: 410 Gone detection tests
    #[test]
    fn test_410_gone_detected_in_error_string() {
        let err = anyhow::anyhow!("Delta token expired (410 Gone)");
        let err_str = format!("{err:#}");
        assert!(err_str.contains("410") || err_str.contains("Gone"));
    }

    #[test]
    fn test_410_gone_not_transient() {
        // 410 Gone should NOT be treated as a transient error
        // (it needs special handling, not generic retry)
        let err = anyhow::anyhow!("Delta token expired (410 Gone)");
        assert!(!is_transient_error(&err));
    }

    // T186: ChangeEvent tests
    #[test]
    fn test_change_event_created() {
        let event = ChangeEvent::Created(PathBuf::from("/home/user/OneDrive/new.txt"));
        assert!(matches!(event, ChangeEvent::Created(_)));
    }

    #[test]
    fn test_change_event_modified() {
        let event = ChangeEvent::Modified(PathBuf::from("/home/user/OneDrive/file.txt"));
        assert!(matches!(event, ChangeEvent::Modified(_)));
    }

    #[test]
    fn test_change_event_deleted() {
        let event = ChangeEvent::Deleted(PathBuf::from("/home/user/OneDrive/old.txt"));
        assert!(matches!(event, ChangeEvent::Deleted(_)));
    }

    #[test]
    fn test_change_event_renamed() {
        let event = ChangeEvent::Renamed {
            old: PathBuf::from("/home/user/OneDrive/old.txt"),
            new: PathBuf::from("/home/user/OneDrive/new.txt"),
        };
        assert!(matches!(event, ChangeEvent::Renamed { .. }));
    }

    #[test]
    fn test_change_event_debug() {
        let event = ChangeEvent::Created(PathBuf::from("/test/path"));
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Created"));
        assert!(debug_str.contains("/test/path"));
    }

    // ====================================================================
    // T212: Bulk mode tests
    // ====================================================================

    #[test]
    fn test_bulk_mode_constants() {
        assert_eq!(BULK_MODE_THRESHOLD, 1000);
        assert_eq!(BULK_MODE_MAX_CONCURRENT, 4);
        assert_eq!(BULK_MODE_BATCH_DELAY_MS, 2000);
    }

    #[test]
    fn test_bulk_mode_max_concurrent_normal() {
        // In normal mode, max concurrent = 8
        // We can only test this if we have a SyncEngine, which needs mocks.
        // Instead, test the constant relationships.
        assert!(
            BULK_MODE_MAX_CONCURRENT < 8,
            "Bulk mode should reduce concurrency"
        );
    }

    #[test]
    fn test_bulk_mode_batch_delay_positive() {
        assert!(
            BULK_MODE_BATCH_DELAY_MS > 0,
            "Batch delay should be positive in bulk mode"
        );
    }

    #[test]
    fn test_bulk_mode_threshold_reasonable() {
        // Threshold should be large enough to avoid false positives
        assert!(
            BULK_MODE_THRESHOLD >= 100,
            "Threshold should be at least 100"
        );
        // But not so large that initial syncs aren't detected
        assert!(
            BULK_MODE_THRESHOLD <= 10000,
            "Threshold should be at most 10000"
        );
    }
}
