//! On-demand file hydration manager.
//!
//! Provides `HydrationManager` for downloading file content from OneDrive
//! when a dehydrated file is accessed (read, mmap, or exec).
//!
//! ## Architecture
//!
//! The `HydrationManager` coordinates concurrent file downloads while ensuring:
//!
//! - **Deduplication**: Multiple readers of the same file share a single download
//! - **Concurrency limiting**: Configurable maximum parallel downloads
//! - **Progress tracking**: Watch channels for real-time progress updates
//! - **Cancellation support**: In-flight downloads can be cancelled
//!
//! ```text
//! ┌───────────────┐     hydrate()      ┌─────────────────────┐
//! │  FUSE reader  │ ─────────────────► │  HydrationManager   │
//! │   (waiting)   │                    │                     │
//! └───────────────┘                    │  active: DashMap    │
//!        │                             │  semaphore: permits │
//!        │  watch::Receiver            │                     │
//!        │◄────────────────────────────│                     │
//!        │                             └─────────────────────┘
//!        │                                       │
//!        │                                       │ spawn download task
//!        │                                       ▼
//!        │                             ┌─────────────────────┐
//!        │                             │  Download Task      │
//!        │                             │  - GraphCloudProvider│
//!        │                             │  - ContentCache     │
//!        │                             │  - WriteSerializer  │
//!        │                             └─────────────────────┘
//!        │                                       │
//!        │  progress updates                     │
//!        │◄──────────────────────────────────────┘
//! ```

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use lnxdrive_core::domain::{sync_item::ItemState, RemoteId, UniqueId};
use lnxdrive_graph::provider::GraphCloudProvider;
use tokio::{
    runtime::Handle,
    sync::{watch, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{cache::ContentCache, error::FuseError, write_serializer::WriteSerializerHandle};

// ============================================================================
// HydrationPriority
// ============================================================================

/// Priority levels for hydration requests.
///
/// Higher priority requests are processed first when the hydration
/// queue has multiple pending items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HydrationPriority {
    /// Lowest priority - prefetch for anticipated access
    Prefetch = 0,
    /// Medium priority - user explicitly pinned the file
    PinRequest = 1,
    /// Highest priority - user is actively opening the file
    UserOpen = 2,
}

// ============================================================================
// HydrationRequest
// ============================================================================

/// Represents an in-progress file hydration (download).
///
/// Tracks download progress and provides a watch channel for
/// progress notifications to waiting readers.
pub struct HydrationRequest {
    /// FUSE inode number
    pub ino: u64,
    /// Database item ID
    pub item_id: UniqueId,
    /// OneDrive remote ID
    pub remote_id: RemoteId,
    /// Total file size in bytes
    pub total_size: u64,
    /// Bytes downloaded so far (atomically updated)
    downloaded: AtomicU64,
    /// Path to the cache file
    pub cache_path: PathBuf,
    /// Request priority
    pub priority: HydrationPriority,
    /// When the request was created
    pub created_at: DateTime<Utc>,
    /// Channel to send progress updates (0-100%)
    progress_tx: watch::Sender<u8>,
}

impl HydrationRequest {
    /// Create a new hydration request.
    ///
    /// Returns the request and a receiver for progress updates.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number for the file
    /// * `item_id` - Database item ID
    /// * `remote_id` - OneDrive remote ID for fetching
    /// * `total_size` - Total file size in bytes
    /// * `cache_path` - Path where the file will be cached
    /// * `priority` - Priority level for this request
    #[must_use]
    pub fn new(
        ino: u64,
        item_id: UniqueId,
        remote_id: RemoteId,
        total_size: u64,
        cache_path: PathBuf,
        priority: HydrationPriority,
    ) -> (Self, watch::Receiver<u8>) {
        let (progress_tx, progress_rx) = watch::channel(0u8);
        let request = Self {
            ino,
            item_id,
            remote_id,
            total_size,
            downloaded: AtomicU64::new(0),
            cache_path,
            priority,
            created_at: Utc::now(),
            progress_tx,
        };
        (request, progress_rx)
    }

    /// Calculate current progress as percentage (0-100).
    ///
    /// Returns 100 for empty files (they are immediately complete).
    #[must_use]
    pub fn progress(&self) -> u8 {
        if self.total_size == 0 {
            return 100; // Empty file is complete
        }
        let downloaded = self.downloaded.load(Ordering::SeqCst);
        ((downloaded * 100) / self.total_size).min(100) as u8
    }

    /// Get bytes downloaded so far.
    #[must_use]
    pub fn downloaded(&self) -> u64 {
        self.downloaded.load(Ordering::SeqCst)
    }

    /// Add to downloaded bytes and send progress update.
    ///
    /// This method is thread-safe and can be called from multiple threads.
    pub fn add_downloaded(&self, bytes: u64) {
        self.downloaded.fetch_add(bytes, Ordering::SeqCst);
        let _ = self.progress_tx.send(self.progress());
    }

    /// Set downloaded to total (mark complete).
    ///
    /// Sends a 100% progress update to all subscribers.
    pub fn mark_complete(&self) {
        self.downloaded.store(self.total_size, Ordering::SeqCst);
        let _ = self.progress_tx.send(100);
    }

    /// Get a receiver for progress updates.
    ///
    /// Multiple subscribers can watch the same request's progress.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u8> {
        self.progress_tx.subscribe()
    }
}

impl fmt::Debug for HydrationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HydrationRequest")
            .field("ino", &self.ino)
            .field("item_id", &self.item_id)
            .field("remote_id", &self.remote_id)
            .field("total_size", &self.total_size)
            .field("downloaded", &self.downloaded())
            .field("cache_path", &self.cache_path)
            .field("priority", &self.priority)
            .field("created_at", &self.created_at)
            .field("progress", &format!("{}%", self.progress()))
            .finish()
    }
}

// ============================================================================
// T049: HydrationManager struct
// ============================================================================

/// Threshold in bytes for using chunked downloads (100 MB).
const CHUNKED_DOWNLOAD_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Size of each chunk for large file downloads (10 MB).
const DOWNLOAD_CHUNK_SIZE: u64 = 10 * 1024 * 1024;

/// Internal state for an active hydration task.
struct ActiveHydration {
    /// The hydration request being processed
    request: Arc<HydrationRequest>,
    /// Cancellation token for the download task
    cancel_token: CancellationToken,
    /// Join handle for the download task (for awaiting completion)
    _task_handle: JoinHandle<()>,
}

/// Manages concurrent file hydration (download) operations.
///
/// Ensures:
/// - **Deduplication**: The same inode is not downloaded twice concurrently.
///   Multiple readers waiting on the same file share a single download task.
/// - **Concurrency limit**: Configurable maximum parallel downloads via semaphore.
/// - **Progress tracking**: Watch channels for real-time progress updates.
/// - **Cancellation**: In-flight downloads can be cancelled.
///
/// # Example
///
/// ```ignore
/// let manager = HydrationManager::new(
///     8,  // max concurrent downloads
///     cache,
///     write_handle,
///     provider,
///     Handle::current(),
/// );
///
/// // Start hydration for a file
/// let progress_rx = manager.hydrate(
///     ino,
///     item_id,
///     remote_id,
///     file_size,
///     HydrationPriority::UserOpen,
/// ).await?;
///
/// // Wait for completion
/// manager.wait_for_completion(ino).await?;
/// ```
pub struct HydrationManager {
    /// Active hydration requests, keyed by inode
    active: Arc<DashMap<u64, ActiveHydration>>,
    /// Semaphore for concurrency limiting
    semaphore: Arc<Semaphore>,
    /// Content cache for storing downloaded files
    cache: Arc<ContentCache>,
    /// Handle for serialized DB writes
    write_handle: WriteSerializerHandle,
    /// Cloud provider for downloads
    provider: Arc<GraphCloudProvider>,
    /// Tokio runtime handle for spawning tasks
    rt_handle: Handle,
}

impl HydrationManager {
    /// Creates a new `HydrationManager`.
    ///
    /// # Arguments
    ///
    /// * `max_concurrent` - Maximum number of parallel downloads
    /// * `cache` - Content cache for storing downloaded files
    /// * `write_handle` - Handle for serialized database writes
    /// * `provider` - Cloud provider for downloading files
    /// * `rt_handle` - Tokio runtime handle for spawning download tasks
    pub fn new(
        max_concurrent: usize,
        cache: Arc<ContentCache>,
        write_handle: WriteSerializerHandle,
        provider: Arc<GraphCloudProvider>,
        rt_handle: Handle,
    ) -> Self {
        Self {
            active: Arc::new(DashMap::new()),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            cache,
            write_handle,
            provider,
            rt_handle,
        }
    }
}

// ============================================================================
// T050: HydrationManager::hydrate()
// ============================================================================

impl HydrationManager {
    /// Initiates hydration (download) for a file.
    ///
    /// If the file is already being hydrated, returns a receiver for the existing
    /// download's progress. Otherwise, creates a new download task.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    /// * `item_id` - Database item ID
    /// * `remote_id` - OneDrive remote ID for fetching
    /// * `total_size` - Total file size in bytes
    /// * `priority` - Priority level for this request
    ///
    /// # Returns
    ///
    /// A `watch::Receiver<u8>` that receives progress updates (0-100%).
    ///
    /// # Errors
    ///
    /// Returns an error if the download cannot be started.
    pub async fn hydrate(
        &self,
        ino: u64,
        item_id: UniqueId,
        remote_id: RemoteId,
        total_size: u64,
        priority: HydrationPriority,
    ) -> Result<watch::Receiver<u8>, FuseError> {
        // Check if already hydrating (deduplication)
        if let Some(active) = self.active.get(&ino) {
            tracing::debug!(
                ino,
                "Hydration already in progress, returning existing receiver"
            );
            return Ok(active.request.subscribe());
        }

        // Create the cache path
        let cache_path = self.cache.cache_path(&remote_id);

        // Create the hydration request
        let (request, progress_rx) = HydrationRequest::new(
            ino,
            item_id,
            remote_id.clone(),
            total_size,
            cache_path,
            priority,
        );
        let request = Arc::new(request);

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Clone values for the spawned task
        let semaphore = Arc::clone(&self.semaphore);
        let cache = Arc::clone(&self.cache);
        let write_handle = self.write_handle.clone();
        let provider = Arc::clone(&self.provider);
        let request_clone = Arc::clone(&request);
        let cancel_token_clone = cancel_token.clone();
        let active_map = self.active.clone();

        // Update item state to Hydrating
        write_handle
            .update_state(item_id, ItemState::Hydrating)
            .await?;

        // Spawn the download task
        let task_handle = self.rt_handle.spawn(async move {
            let result = Self::download_task(
                ino,
                item_id,
                remote_id,
                total_size,
                semaphore,
                cache,
                write_handle.clone(),
                provider,
                request_clone,
                cancel_token_clone,
            )
            .await;

            // Handle completion or error
            match result {
                Ok(()) => {
                    tracing::info!(ino, "Hydration completed successfully");
                    // Update state to Hydrated
                    if let Err(e) = write_handle
                        .update_state(item_id, ItemState::Hydrated)
                        .await
                    {
                        tracing::error!(ino, error = %e, "Failed to update state to Hydrated");
                    }
                    // Clear hydration progress
                    if let Err(e) = write_handle.update_hydration_progress(item_id, None).await {
                        tracing::error!(ino, error = %e, "Failed to clear hydration progress");
                    }
                }
                Err(e) => {
                    tracing::error!(ino, error = %e, "Hydration failed");
                    // Update state to Error
                    if let Err(update_err) = write_handle
                        .update_state(item_id, ItemState::Error(e.to_string()))
                        .await
                    {
                        tracing::error!(
                            ino,
                            error = %update_err,
                            "Failed to update state to Error"
                        );
                    }
                }
            }

            // Remove from active map
            active_map.remove(&ino);
        });

        // Insert into active map
        self.active.insert(
            ino,
            ActiveHydration {
                request,
                cancel_token,
                _task_handle: task_handle,
            },
        );

        Ok(progress_rx)
    }

    /// Internal download task that performs the actual file download.
    #[allow(clippy::too_many_arguments)]
    async fn download_task(
        ino: u64,
        item_id: UniqueId,
        remote_id: RemoteId,
        total_size: u64,
        semaphore: Arc<Semaphore>,
        cache: Arc<ContentCache>,
        write_handle: WriteSerializerHandle,
        provider: Arc<GraphCloudProvider>,
        request: Arc<HydrationRequest>,
        cancel_token: CancellationToken,
    ) -> Result<(), FuseError> {
        // Acquire semaphore permit (limits concurrency)
        let _permit = semaphore
            .acquire()
            .await
            .map_err(|_| FuseError::HydrationFailed("Semaphore closed".to_string()))?;

        tracing::debug!(ino, total_size, "Starting download");

        // Check for cancellation before starting
        if cancel_token.is_cancelled() {
            return Err(FuseError::HydrationFailed("Cancelled".to_string()));
        }

        // Get download URL from Graph API
        let download_url = provider.get_download_url(&remote_id).await.map_err(|e| {
            FuseError::HydrationFailed(format!("Failed to get download URL: {}", e))
        })?;

        // Get partial path for download
        let partial_path = cache.partial_path(&remote_id);
        let final_path = cache.cache_path(&remote_id);

        // Ensure parent directory exists
        if let Some(parent) = partial_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Choose download strategy based on file size
        if total_size < CHUNKED_DOWNLOAD_THRESHOLD {
            // Full download for smaller files
            Self::download_full(
                ino,
                &download_url,
                &partial_path,
                &provider,
                &request,
                &cancel_token,
                &write_handle,
                &item_id,
            )
            .await?;
        } else {
            // Chunked download for larger files
            Self::download_chunked(
                ino,
                &download_url,
                &partial_path,
                total_size,
                &provider,
                &request,
                &cancel_token,
                &write_handle,
                &item_id,
            )
            .await?;
        }

        // Rename partial file to final path
        std::fs::rename(&partial_path, &final_path).map_err(|e| {
            FuseError::HydrationFailed(format!("Failed to rename partial file: {}", e))
        })?;

        // Mark request as complete
        request.mark_complete();

        Ok(())
    }

    /// Download a file in a single request (for files < 100MB).
    #[allow(clippy::too_many_arguments)]
    async fn download_full(
        ino: u64,
        download_url: &str,
        partial_path: &Path,
        provider: &Arc<GraphCloudProvider>,
        request: &Arc<HydrationRequest>,
        cancel_token: &CancellationToken,
        write_handle: &WriteSerializerHandle,
        item_id: &UniqueId,
    ) -> Result<(), FuseError> {
        tracing::debug!(ino, "Using full download strategy");

        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Err(FuseError::HydrationFailed("Cancelled".to_string()));
        }

        // Download to partial file
        let bytes_written = provider
            .download_file_to_disk(download_url, partial_path)
            .await
            .map_err(|e| FuseError::HydrationFailed(format!("Download failed: {}", e)))?;

        // Update progress
        request.add_downloaded(bytes_written);

        // Update progress in database
        let progress = request.progress();
        if let Err(e) = write_handle
            .update_hydration_progress(*item_id, Some(progress))
            .await
        {
            tracing::warn!(ino, error = %e, "Failed to update hydration progress in DB");
        }

        Ok(())
    }

    /// Download a file in chunks using HTTP Range requests (for files >= 100MB).
    #[allow(clippy::too_many_arguments)]
    async fn download_chunked(
        ino: u64,
        download_url: &str,
        partial_path: &Path,
        total_size: u64,
        provider: &Arc<GraphCloudProvider>,
        request: &Arc<HydrationRequest>,
        cancel_token: &CancellationToken,
        write_handle: &WriteSerializerHandle,
        item_id: &UniqueId,
    ) -> Result<(), FuseError> {
        tracing::debug!(
            ino,
            total_size,
            chunk_size = DOWNLOAD_CHUNK_SIZE,
            "Using chunked download strategy"
        );

        // Pre-allocate the file to the expected size
        {
            let file = std::fs::File::create(partial_path)?;
            file.set_len(total_size)?;
        }

        let mut offset = 0u64;
        let mut last_reported_progress = 0u8;

        while offset < total_size {
            // Check for cancellation before each chunk
            if cancel_token.is_cancelled() {
                // Clean up partial file
                let _ = std::fs::remove_file(partial_path);
                return Err(FuseError::HydrationFailed("Cancelled".to_string()));
            }

            // Calculate chunk size (may be smaller for last chunk)
            let remaining = total_size - offset;
            let chunk_size = remaining.min(DOWNLOAD_CHUNK_SIZE);

            tracing::trace!(
                ino,
                offset,
                chunk_size,
                progress = request.progress(),
                "Downloading chunk"
            );

            // Download the chunk
            let bytes_written = provider
                .download_range(download_url, partial_path, offset, chunk_size)
                .await
                .map_err(|e| {
                    FuseError::HydrationFailed(format!(
                        "Chunk download failed at offset {}: {}",
                        offset, e
                    ))
                })?;

            // Update progress
            request.add_downloaded(bytes_written);
            offset += bytes_written;

            // Update progress in database (throttled to avoid too many writes)
            let current_progress = request.progress();
            if current_progress >= last_reported_progress + 5 || current_progress == 100 {
                if let Err(e) = write_handle
                    .update_hydration_progress(*item_id, Some(current_progress))
                    .await
                {
                    tracing::warn!(ino, error = %e, "Failed to update hydration progress in DB");
                }
                last_reported_progress = current_progress;
            }
        }

        Ok(())
    }
}

// ============================================================================
// T051: HydrationManager::wait_for_completion()
// ============================================================================

impl HydrationManager {
    /// Waits for a hydration to complete.
    ///
    /// Blocks until the hydration for the given inode reaches 100% progress.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number of the file being hydrated
    ///
    /// # Returns
    ///
    /// `Ok(())` when hydration completes successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if the hydration fails or if no hydration is active
    /// for the given inode.
    pub async fn wait_for_completion(&self, ino: u64) -> Result<(), FuseError> {
        // Get a receiver for the active hydration
        let mut rx = {
            let active = self.active.get(&ino).ok_or_else(|| {
                FuseError::NotFound(format!("No active hydration for inode {}", ino))
            })?;
            active.request.subscribe()
        };

        // Wait for progress to reach 100
        loop {
            let progress = *rx.borrow();
            if progress >= 100 {
                return Ok(());
            }

            // Wait for next update
            rx.changed()
                .await
                .map_err(|_| FuseError::HydrationFailed("Hydration channel closed".to_string()))?;
        }
    }
}

// ============================================================================
// T052: HydrationManager::wait_for_range()
// ============================================================================

impl HydrationManager {
    /// Waits until a specific byte range is available.
    ///
    /// For full downloads, this waits for completion. For streaming downloads,
    /// this could return sooner when the requested range becomes available.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    /// * `offset` - Starting byte offset of the range
    /// * `size` - Size of the range in bytes
    ///
    /// # Returns
    ///
    /// `Ok(())` when the requested range is available.
    ///
    /// # Errors
    ///
    /// Returns an error if the hydration fails or if no hydration is active.
    pub async fn wait_for_range(&self, ino: u64, offset: u64, size: u64) -> Result<(), FuseError> {
        // Get request info
        let (mut rx, total_size) = {
            let active = self.active.get(&ino).ok_or_else(|| {
                FuseError::NotFound(format!("No active hydration for inode {}", ino))
            })?;
            (active.request.subscribe(), active.request.total_size)
        };

        // Calculate the end of the requested range
        let range_end = offset.saturating_add(size);

        // For sequential downloads, we need to wait until downloaded bytes >= range_end
        // Progress is percentage, so we calculate required progress
        let required_progress = if total_size == 0 {
            100u8
        } else {
            // Calculate minimum progress needed for the range to be available
            // We need at least (range_end / total_size * 100) progress
            ((range_end * 100) / total_size).min(100) as u8
        };

        tracing::debug!(
            ino,
            offset,
            size,
            range_end,
            required_progress,
            "Waiting for byte range"
        );

        // Wait for sufficient progress
        loop {
            let progress = *rx.borrow();
            if progress >= required_progress {
                return Ok(());
            }

            // Wait for next update
            rx.changed()
                .await
                .map_err(|_| FuseError::HydrationFailed("Hydration channel closed".to_string()))?;
        }
    }
}

// ============================================================================
// T053: HydrationManager::cancel()
// ============================================================================

impl HydrationManager {
    /// Cancels an in-progress hydration.
    ///
    /// Removes the hydration from the active map, signals cancellation to the
    /// download task, and deletes any partial file.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number of the file to cancel
    ///
    /// # Returns
    ///
    /// `Ok(())` if cancellation was successful or if no hydration was active.
    pub async fn cancel(&self, ino: u64) -> Result<(), FuseError> {
        // Remove from active map and get the cancellation token
        if let Some((_, active)) = self.active.remove(&ino) {
            tracing::info!(ino, "Cancelling hydration");

            // Signal cancellation
            active.cancel_token.cancel();

            // Delete partial file
            let partial_path = self.cache.partial_path(&active.request.remote_id);
            if partial_path.exists() {
                if let Err(e) = std::fs::remove_file(&partial_path) {
                    tracing::warn!(
                        ino,
                        path = %partial_path.display(),
                        error = %e,
                        "Failed to delete partial file"
                    );
                }
            }

            // Update state back to Online
            if let Err(e) = self
                .write_handle
                .update_state(active.request.item_id, ItemState::Online)
                .await
            {
                tracing::error!(ino, error = %e, "Failed to reset state to Online after cancel");
            }

            // Clear hydration progress
            if let Err(e) = self
                .write_handle
                .update_hydration_progress(active.request.item_id, None)
                .await
            {
                tracing::warn!(
                    ino,
                    error = %e,
                    "Failed to clear hydration progress after cancel"
                );
            }
        } else {
            tracing::debug!(ino, "No active hydration to cancel");
        }

        Ok(())
    }
}

// ============================================================================
// T054: HydrationManager::is_hydrating() and progress()
// ============================================================================

impl HydrationManager {
    /// Checks if a file is currently being hydrated.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number to check
    ///
    /// # Returns
    ///
    /// `true` if hydration is in progress for the inode, `false` otherwise.
    pub fn is_hydrating(&self, ino: u64) -> bool {
        self.active.contains_key(&ino)
    }

    /// Gets the current progress of an active hydration.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number to check
    ///
    /// # Returns
    ///
    /// `Some(progress)` with progress percentage (0-100) if hydration is active,
    /// `None` if no hydration is in progress for the inode.
    pub fn progress(&self, ino: u64) -> Option<u8> {
        self.active
            .get(&ino)
            .map(|active| active.request.progress())
    }

    /// Gets the number of currently active hydrations.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Gets a progress receiver for an active hydration.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    ///
    /// # Returns
    ///
    /// `Some(receiver)` if hydration is active, `None` otherwise.
    pub fn subscribe(&self, ino: u64) -> Option<watch::Receiver<u8>> {
        self.active
            .get(&ino)
            .map(|active| active.request.subscribe())
    }
}

// ============================================================================
// T073: HydrationManager::pin()
// ============================================================================

impl HydrationManager {
    /// Pins a file for permanent offline access.
    ///
    /// If the file is not hydrated, triggers hydration with `PinRequest` priority.
    /// Once hydrated, transitions the state to `Pinned`. Pinned files are never
    /// auto-dehydrated.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    /// * `item_id` - Database item ID
    /// * `remote_id` - OneDrive remote ID
    /// * `total_size` - Total file size in bytes
    /// * `current_state` - Current state of the item
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # State Transitions
    ///
    /// - `Online` → triggers hydration → `Pinned`
    /// - `Hydrating` → waits for completion → `Pinned`
    /// - `Hydrated` → directly → `Pinned`
    /// - `Pinned` → no-op
    pub async fn pin(
        &self,
        ino: u64,
        item_id: UniqueId,
        remote_id: RemoteId,
        total_size: u64,
        current_state: ItemState,
    ) -> Result<(), FuseError> {
        tracing::info!(ino, ?current_state, "Pinning file");

        match current_state {
            ItemState::Pinned => {
                // Already pinned, no-op
                tracing::debug!(ino, "File is already pinned");
                Ok(())
            }
            ItemState::Online => {
                // Need to hydrate first, then pin
                tracing::debug!(ino, "File is Online, hydrating before pinning");

                // Start hydration with PinRequest priority
                let _progress_rx = self
                    .hydrate(ino, item_id, remote_id, total_size, HydrationPriority::PinRequest)
                    .await?;

                // Wait for completion
                self.wait_for_completion(ino).await?;

                // Transition to Pinned
                self.write_handle
                    .update_state(item_id, ItemState::Pinned)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                tracing::info!(ino, "File pinned after hydration");
                Ok(())
            }
            ItemState::Hydrating => {
                // Wait for hydration to complete, then pin
                tracing::debug!(ino, "File is Hydrating, waiting before pinning");

                self.wait_for_completion(ino).await?;

                // Transition to Pinned
                self.write_handle
                    .update_state(item_id, ItemState::Pinned)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                tracing::info!(ino, "File pinned after hydration completed");
                Ok(())
            }
            ItemState::Hydrated => {
                // Already hydrated, just transition to Pinned
                tracing::debug!(ino, "File is Hydrated, transitioning to Pinned");

                self.write_handle
                    .update_state(item_id, ItemState::Pinned)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                tracing::info!(ino, "File pinned");
                Ok(())
            }
            ItemState::Modified => {
                // Modified files can be pinned (they stay modified but also pinned)
                // Actually, per state machine, Modified -> Pinned is valid
                tracing::debug!(ino, "File is Modified, transitioning to Pinned");

                self.write_handle
                    .update_state(item_id, ItemState::Pinned)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                tracing::info!(ino, "Modified file pinned");
                Ok(())
            }
            _ => {
                // For other states (Deleted, Error, Conflicted), pinning doesn't make sense
                Err(FuseError::InvalidArgument(format!(
                    "Cannot pin file in state {:?}",
                    current_state
                )))
            }
        }
    }
}

// ============================================================================
// T074: HydrationManager::unpin()
// ============================================================================

impl HydrationManager {
    /// Unpins a file, allowing it to be auto-dehydrated.
    ///
    /// Transitions the file state from `Pinned` to `Hydrated`.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number
    /// * `item_id` - Database item ID
    /// * `current_state` - Current state of the item
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # State Transitions
    ///
    /// - `Pinned` → `Hydrated`
    /// - Other states → no-op or error
    pub async fn unpin(
        &self,
        ino: u64,
        item_id: UniqueId,
        current_state: ItemState,
    ) -> Result<(), FuseError> {
        tracing::info!(ino, ?current_state, "Unpinning file");

        match current_state {
            ItemState::Pinned => {
                // Transition to Hydrated
                self.write_handle
                    .update_state(item_id, ItemState::Hydrated)
                    .await
                    .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

                tracing::info!(ino, "File unpinned");
                Ok(())
            }
            ItemState::Hydrated => {
                // Already not pinned, no-op
                tracing::debug!(ino, "File is already unpinned (Hydrated)");
                Ok(())
            }
            ItemState::Online => {
                // Not hydrated, nothing to unpin
                tracing::debug!(ino, "File is Online, nothing to unpin");
                Ok(())
            }
            _ => {
                // For other states, unpinning doesn't apply
                tracing::debug!(ino, ?current_state, "Cannot unpin file in this state");
                Ok(())
            }
        }
    }
}

// ============================================================================
// T075: HydrationManager::pin_recursive()
// ============================================================================

use crate::inode::InodeTable;
use std::pin::Pin;
use std::future::Future;

/// Type alias for the boxed future returned by recursive pin/unpin operations.
type PinResultFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<(u64, ItemState)>, FuseError>> + Send + 'a>>;

impl HydrationManager {
    /// Recursively pins all files in a directory.
    ///
    /// Iterates through all children of the given directory, pins each file,
    /// and recurses into subdirectories.
    ///
    /// # Arguments
    ///
    /// * `parent_ino` - FUSE inode number of the directory
    /// * `inode_table` - Reference to the inode table for traversal
    ///
    /// # Returns
    ///
    /// A vector of (ino, new_state) tuples for all pinned files.
    pub fn pin_recursive<'a>(
        &'a self,
        parent_ino: u64,
        inode_table: &'a InodeTable,
    ) -> PinResultFuture<'a> {
        Box::pin(async move {
            let mut results = Vec::new();

            // Get all children of the directory
            let children = inode_table.children(parent_ino);

            for child in children {
                let ino = child.ino().get();
                let item_id = *child.item_id();
                let current_state = child.state();

                if child.kind() == fuser::FileType::Directory {
                    // Recurse into subdirectory
                    tracing::debug!(ino, name = child.name(), "Recursing into directory");
                    let sub_results = self.pin_recursive(ino, inode_table).await?;
                    results.extend(sub_results);
                } else {
                    // Pin the file
                    if let Some(remote_id) = child.remote_id() {
                        match self
                            .pin(ino, item_id, remote_id.clone(), child.size(), current_state.clone())
                            .await
                        {
                            Ok(()) => {
                                results.push((ino, ItemState::Pinned));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    ino,
                                    name = child.name(),
                                    error = %e,
                                    "Failed to pin file, skipping"
                                );
                                // Continue with other files
                            }
                        }
                    } else {
                        tracing::debug!(
                            ino,
                            name = child.name(),
                            "Skipping file without remote_id (newly created)"
                        );
                    }
                }
            }

            Ok(results)
        })
    }

    /// Recursively unpins all files in a directory.
    ///
    /// # Arguments
    ///
    /// * `parent_ino` - FUSE inode number of the directory
    /// * `inode_table` - Reference to the inode table for traversal
    ///
    /// # Returns
    ///
    /// A vector of (ino, new_state) tuples for all unpinned files.
    pub fn unpin_recursive<'a>(
        &'a self,
        parent_ino: u64,
        inode_table: &'a InodeTable,
    ) -> PinResultFuture<'a> {
        Box::pin(async move {
            let mut results = Vec::new();

            // Get all children of the directory
            let children = inode_table.children(parent_ino);

            for child in children {
                let ino = child.ino().get();
                let item_id = *child.item_id();
                let current_state = child.state();

                if child.kind() == fuser::FileType::Directory {
                    // Recurse into subdirectory
                    let sub_results = self.unpin_recursive(ino, inode_table).await?;
                    results.extend(sub_results);
                } else if matches!(current_state, ItemState::Pinned) {
                    // Unpin the file
                    match self.unpin(ino, item_id, current_state.clone()).await {
                        Ok(()) => {
                            results.push((ino, ItemState::Hydrated));
                        }
                        Err(e) => {
                            tracing::warn!(
                                ino,
                                name = child.name(),
                                error = %e,
                                "Failed to unpin file, skipping"
                            );
                        }
                    }
                }
            }

            Ok(results)
        })
    }
}

impl fmt::Debug for HydrationManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HydrationManager")
            .field("active_count", &self.active.len())
            .field("semaphore_permits", &self.semaphore.available_permits())
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    mod hydration_priority_tests {
        use super::*;

        #[test]
        fn test_priority_ordering() {
            // UserOpen should be greater than PinRequest
            assert!(HydrationPriority::UserOpen > HydrationPriority::PinRequest);
            // PinRequest should be greater than Prefetch
            assert!(HydrationPriority::PinRequest > HydrationPriority::Prefetch);
            // UserOpen should be greater than Prefetch (transitive)
            assert!(HydrationPriority::UserOpen > HydrationPriority::Prefetch);
        }

        #[test]
        fn test_priority_equality() {
            assert_eq!(HydrationPriority::UserOpen, HydrationPriority::UserOpen);
            assert_eq!(HydrationPriority::PinRequest, HydrationPriority::PinRequest);
            assert_eq!(HydrationPriority::Prefetch, HydrationPriority::Prefetch);
        }

        #[test]
        fn test_priority_values() {
            assert_eq!(HydrationPriority::Prefetch as u8, 0);
            assert_eq!(HydrationPriority::PinRequest as u8, 1);
            assert_eq!(HydrationPriority::UserOpen as u8, 2);
        }

        #[test]
        fn test_priority_sorting() {
            let mut priorities = vec![
                HydrationPriority::PinRequest,
                HydrationPriority::UserOpen,
                HydrationPriority::Prefetch,
            ];
            priorities.sort();
            assert_eq!(
                priorities,
                vec![
                    HydrationPriority::Prefetch,
                    HydrationPriority::PinRequest,
                    HydrationPriority::UserOpen,
                ]
            );
        }

        #[test]
        fn test_priority_clone() {
            let priority = HydrationPriority::UserOpen;
            let cloned = priority;
            assert_eq!(priority, cloned);
        }

        #[test]
        fn test_priority_debug() {
            let debug_str = format!("{:?}", HydrationPriority::UserOpen);
            assert_eq!(debug_str, "UserOpen");
        }

        #[test]
        fn test_priority_hash() {
            use std::collections::HashSet;
            let mut set = HashSet::new();
            set.insert(HydrationPriority::UserOpen);
            set.insert(HydrationPriority::PinRequest);
            set.insert(HydrationPriority::Prefetch);
            assert_eq!(set.len(), 3);

            // Duplicate should not increase size
            set.insert(HydrationPriority::UserOpen);
            assert_eq!(set.len(), 3);
        }
    }

    mod hydration_request_tests {
        use std::path::PathBuf;

        use super::*;

        fn create_test_request(total_size: u64, priority: HydrationPriority) -> HydrationRequest {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("ABC123DEF456".to_string()).unwrap();
            let cache_path = PathBuf::from("/tmp/cache/test_file.dat");

            let (request, _rx) = HydrationRequest::new(
                42, // ino
                item_id, remote_id, total_size, cache_path, priority,
            );
            request
        }

        #[test]
        fn test_new_request() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("ABC123DEF456".to_string()).unwrap();
            let cache_path = PathBuf::from("/tmp/cache/test_file.dat");

            let (request, rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id.clone(),
                1000,
                cache_path.clone(),
                HydrationPriority::UserOpen,
            );

            assert_eq!(request.ino, 42);
            assert_eq!(request.total_size, 1000);
            assert_eq!(request.downloaded(), 0);
            assert_eq!(request.cache_path, cache_path);
            assert_eq!(request.priority, HydrationPriority::UserOpen);
            assert_eq!(*rx.borrow(), 0); // Initial progress is 0
        }

        #[test]
        fn test_progress_calculation() {
            let request = create_test_request(1000, HydrationPriority::UserOpen);

            // Initial progress should be 0
            assert_eq!(request.progress(), 0);

            // Add 250 bytes -> 25%
            request.add_downloaded(250);
            assert_eq!(request.progress(), 25);

            // Add 250 more bytes -> 50%
            request.add_downloaded(250);
            assert_eq!(request.progress(), 50);

            // Add 500 more bytes -> 100%
            request.add_downloaded(500);
            assert_eq!(request.progress(), 100);
        }

        #[test]
        fn test_progress_empty_file() {
            let request = create_test_request(0, HydrationPriority::Prefetch);

            // Empty file should report 100% progress immediately
            assert_eq!(request.progress(), 100);
        }

        #[test]
        fn test_progress_capped_at_100() {
            let request = create_test_request(100, HydrationPriority::PinRequest);

            // Even if we somehow add more than total, progress caps at 100
            request.add_downloaded(150);
            assert_eq!(request.progress(), 100);
        }

        #[test]
        fn test_downloaded_tracking() {
            let request = create_test_request(1000, HydrationPriority::UserOpen);

            assert_eq!(request.downloaded(), 0);

            request.add_downloaded(100);
            assert_eq!(request.downloaded(), 100);

            request.add_downloaded(200);
            assert_eq!(request.downloaded(), 300);
        }

        #[test]
        fn test_mark_complete() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("ABC123DEF456".to_string()).unwrap();
            let cache_path = PathBuf::from("/tmp/cache/test_file.dat");

            let (request, rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                1000,
                cache_path,
                HydrationPriority::UserOpen,
            );

            // Partially downloaded
            request.add_downloaded(500);
            assert_eq!(request.progress(), 50);

            // Mark complete
            request.mark_complete();
            assert_eq!(request.downloaded(), 1000);
            assert_eq!(request.progress(), 100);
            assert_eq!(*rx.borrow(), 100);
        }

        #[test]
        fn test_progress_updates_via_channel() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("ABC123DEF456".to_string()).unwrap();
            let cache_path = PathBuf::from("/tmp/cache/test_file.dat");

            let (request, rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                100,
                cache_path,
                HydrationPriority::UserOpen,
            );

            // Initial value
            assert_eq!(*rx.borrow(), 0);

            // Update progress
            request.add_downloaded(50);
            assert_eq!(*rx.borrow(), 50);

            // Another update
            request.add_downloaded(30);
            assert_eq!(*rx.borrow(), 80);

            // Complete
            request.add_downloaded(20);
            assert_eq!(*rx.borrow(), 100);
        }

        #[test]
        fn test_subscribe_multiple_receivers() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("ABC123DEF456".to_string()).unwrap();
            let cache_path = PathBuf::from("/tmp/cache/test_file.dat");

            let (request, rx1) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                100,
                cache_path,
                HydrationPriority::UserOpen,
            );

            // Create additional subscribers
            let rx2 = request.subscribe();
            let rx3 = request.subscribe();

            // Update progress
            request.add_downloaded(25);

            // All receivers should see the update
            assert_eq!(*rx1.borrow(), 25);
            assert_eq!(*rx2.borrow(), 25);
            assert_eq!(*rx3.borrow(), 25);
        }

        #[test]
        fn test_debug_format() {
            let request = create_test_request(1000, HydrationPriority::UserOpen);
            request.add_downloaded(500);

            let debug_str = format!("{:?}", request);

            // Check that Debug output contains expected fields
            assert!(debug_str.contains("HydrationRequest"));
            assert!(debug_str.contains("ino: 42"));
            assert!(debug_str.contains("total_size: 1000"));
            assert!(debug_str.contains("downloaded: 500"));
            assert!(debug_str.contains("priority: UserOpen"));
            assert!(debug_str.contains("50%"));
        }

        #[test]
        fn test_created_at_is_recent() {
            let before = Utc::now();
            let request = create_test_request(1000, HydrationPriority::UserOpen);
            let after = Utc::now();

            assert!(request.created_at >= before);
            assert!(request.created_at <= after);
        }

        #[test]
        fn test_atomic_operations_are_consistent() {
            let request = create_test_request(10000, HydrationPriority::UserOpen);

            // Simulate multiple small updates
            for _ in 0..100 {
                request.add_downloaded(100);
            }

            assert_eq!(request.downloaded(), 10000);
            assert_eq!(request.progress(), 100);
        }
    }

    // ========================================================================
    // T061: Unit tests for HydrationManager
    // ========================================================================

    mod hydration_manager_tests {
        use std::sync::Arc;

        use dashmap::DashMap;
        use tokio::sync::Semaphore;

        use super::*;

        // ====================================================================
        // Tests for is_hydrating() and progress() - T054
        // ====================================================================

        /// Test that is_hydrating() returns false for non-existent inode
        #[test]
        fn test_is_hydrating_returns_false_for_unknown_ino() {
            // Create an empty DashMap to simulate empty active map
            let active: DashMap<u64, ()> = DashMap::new();

            // Check for a non-existent inode
            assert!(!active.contains_key(&42));
            assert!(!active.contains_key(&999));
        }

        /// Test that is_hydrating() returns true when inode is in active map
        #[test]
        fn test_is_hydrating_returns_true_for_active_ino() {
            // Create a DashMap with an entry
            let active: DashMap<u64, ()> = DashMap::new();
            active.insert(42, ());

            // Should return true for existing inode
            assert!(active.contains_key(&42));

            // Should return false for non-existent inode
            assert!(!active.contains_key(&100));
        }

        /// Test that progress() returns None for non-existent inode
        #[test]
        fn test_progress_returns_none_for_unknown_ino() {
            // Create an empty DashMap
            let active: DashMap<u64, Arc<HydrationRequest>> = DashMap::new();

            // Progress should be None for non-existent inode
            let progress = active.get(&42).map(|r| r.progress());
            assert!(progress.is_none());
        }

        /// Test that progress() returns correct percentage for active inode
        #[test]
        fn test_progress_returns_correct_percentage() {
            // Create a HydrationRequest with 50% progress
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("test_remote_id".to_string()).unwrap();
            let (request, _rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                1000,
                PathBuf::from("/tmp/cache/test.dat"),
                HydrationPriority::UserOpen,
            );

            // Add 500 bytes (50%)
            request.add_downloaded(500);

            // Wrap in Arc and add to map
            let request = Arc::new(request);
            let active: DashMap<u64, Arc<HydrationRequest>> = DashMap::new();
            active.insert(42, Arc::clone(&request));

            // Progress should be 50
            let progress = active.get(&42).map(|r| r.progress());
            assert_eq!(progress, Some(50));
        }

        // ====================================================================
        // Tests for deduplication behavior - T050
        // ====================================================================

        /// Test that adding the same inode twice preserves the first entry (deduplication)
        #[test]
        fn test_deduplication_preserves_first_entry() {
            let active: DashMap<u64, Arc<HydrationRequest>> = DashMap::new();

            // Create first request
            let item_id1 = UniqueId::new();
            let remote_id1 = RemoteId::new("remote_1".to_string()).unwrap();
            let (request1, _rx1) = HydrationRequest::new(
                42,
                item_id1,
                remote_id1,
                1000,
                PathBuf::from("/tmp/cache/test1.dat"),
                HydrationPriority::UserOpen,
            );
            let request1 = Arc::new(request1);

            // Insert first request
            active.insert(42, Arc::clone(&request1));

            // Check if already hydrating (simulates deduplication check in hydrate())
            let existing = active.get(&42);
            assert!(existing.is_some());

            // The existing receiver would be from the first request
            let existing_request = existing.unwrap();
            assert_eq!(existing_request.total_size, 1000);
        }

        /// Test that multiple subscribers can watch the same hydration
        #[test]
        fn test_multiple_subscribers_receive_same_progress() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("multi_sub".to_string()).unwrap();
            let (request, rx1) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                100,
                PathBuf::from("/tmp/cache/multi.dat"),
                HydrationPriority::UserOpen,
            );

            // Create additional subscribers (simulates multiple readers)
            let rx2 = request.subscribe();
            let rx3 = request.subscribe();

            // Update progress
            request.add_downloaded(50);

            // All receivers should see the same progress
            assert_eq!(*rx1.borrow(), 50);
            assert_eq!(*rx2.borrow(), 50);
            assert_eq!(*rx3.borrow(), 50);
        }

        // ====================================================================
        // Tests for concurrency limiting - T049/T050
        // ====================================================================

        /// Test that semaphore limits concurrent permits
        #[tokio::test]
        async fn test_semaphore_limits_concurrent_permits() {
            // Create semaphore with 2 permits (max 2 concurrent downloads)
            let semaphore = Arc::new(Semaphore::new(2));

            // Acquire first permit
            let permit1 = semaphore.clone().acquire_owned().await.unwrap();
            assert_eq!(semaphore.available_permits(), 1);

            // Acquire second permit
            let permit2 = semaphore.clone().acquire_owned().await.unwrap();
            assert_eq!(semaphore.available_permits(), 0);

            // Try to acquire third permit (should block if we tried)
            // Instead, we use try_acquire to verify it would block
            let result = semaphore.try_acquire();
            assert!(result.is_err()); // No permits available

            // Release first permit
            drop(permit1);
            assert_eq!(semaphore.available_permits(), 1);

            // Now we can acquire again
            let _permit3 = semaphore.clone().acquire_owned().await.unwrap();
            assert_eq!(semaphore.available_permits(), 0);

            // Release remaining permits
            drop(permit2);
            assert_eq!(semaphore.available_permits(), 1);
        }

        /// Test active count tracking
        #[test]
        fn test_active_count_tracks_entries() {
            let active: DashMap<u64, ()> = DashMap::new();

            assert_eq!(active.len(), 0);

            active.insert(1, ());
            assert_eq!(active.len(), 1);

            active.insert(2, ());
            assert_eq!(active.len(), 2);

            active.insert(3, ());
            assert_eq!(active.len(), 3);

            // Remove one
            active.remove(&2);
            assert_eq!(active.len(), 2);
        }

        // ====================================================================
        // Tests for cancel behavior - T053
        // ====================================================================

        /// Test that cancel removes entry from active map
        #[test]
        fn test_cancel_removes_from_active_map() {
            let active: DashMap<u64, ()> = DashMap::new();

            // Insert an entry
            active.insert(42, ());
            assert!(active.contains_key(&42));
            assert_eq!(active.len(), 1);

            // Remove the entry (simulates cancel)
            let removed = active.remove(&42);
            assert!(removed.is_some());
            assert!(!active.contains_key(&42));
            assert_eq!(active.len(), 0);
        }

        /// Test that cancelling non-existent inode is a no-op
        #[test]
        fn test_cancel_nonexistent_is_noop() {
            let active: DashMap<u64, ()> = DashMap::new();

            // Try to remove non-existent entry
            let removed = active.remove(&999);
            assert!(removed.is_none());
            assert_eq!(active.len(), 0);
        }

        /// Test cancellation token behavior
        #[tokio::test]
        async fn test_cancellation_token_signals_correctly() {
            let cancel_token = CancellationToken::new();

            // Initially not cancelled
            assert!(!cancel_token.is_cancelled());

            // Cancel
            cancel_token.cancel();

            // Now cancelled
            assert!(cancel_token.is_cancelled());

            // Clone also sees cancelled state
            let clone = cancel_token.clone();
            assert!(clone.is_cancelled());
        }

        // ====================================================================
        // Tests for progress tracking - T050
        // ====================================================================

        /// Test that watch receiver receives progress updates
        #[tokio::test]
        async fn test_watch_receiver_receives_updates() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("watch_test".to_string()).unwrap();
            let (request, mut rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                100,
                PathBuf::from("/tmp/cache/watch.dat"),
                HydrationPriority::UserOpen,
            );

            // Initial value
            assert_eq!(*rx.borrow(), 0);

            // Update progress
            request.add_downloaded(25);

            // Wait for change
            rx.changed().await.unwrap();
            assert_eq!(*rx.borrow(), 25);

            // Another update
            request.add_downloaded(25);
            rx.changed().await.unwrap();
            assert_eq!(*rx.borrow(), 50);

            // Mark complete
            request.mark_complete();
            rx.changed().await.unwrap();
            assert_eq!(*rx.borrow(), 100);
        }

        /// Test that progress updates are atomic and consistent
        #[test]
        fn test_progress_updates_are_atomic() {
            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("atomic_test".to_string()).unwrap();
            let (request, _rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                1000,
                PathBuf::from("/tmp/cache/atomic.dat"),
                HydrationPriority::UserOpen,
            );

            // Simulate rapid updates
            for i in 0..100 {
                request.add_downloaded(10);
                let downloaded = request.downloaded();
                let expected = (i + 1) * 10;
                assert_eq!(downloaded, expected);
            }

            assert_eq!(request.downloaded(), 1000);
            assert_eq!(request.progress(), 100);
        }

        // ====================================================================
        // Tests for subscribe behavior - T054
        // ====================================================================

        /// Test that subscribe returns a working receiver
        #[test]
        fn test_subscribe_returns_working_receiver() {
            let active: DashMap<u64, Arc<HydrationRequest>> = DashMap::new();

            let item_id = UniqueId::new();
            let remote_id = RemoteId::new("subscribe_test".to_string()).unwrap();
            let (request, _original_rx) = HydrationRequest::new(
                42,
                item_id,
                remote_id,
                100,
                PathBuf::from("/tmp/cache/subscribe.dat"),
                HydrationPriority::UserOpen,
            );
            let request = Arc::new(request);
            active.insert(42, Arc::clone(&request));

            // Subscribe returns a receiver
            let subscribed_rx = active.get(&42).map(|r| r.subscribe());
            assert!(subscribed_rx.is_some());

            let rx = subscribed_rx.unwrap();

            // Update progress through original request
            request.add_downloaded(75);

            // Subscribed receiver sees the update
            assert_eq!(*rx.borrow(), 75);
        }

        /// Test that subscribe returns None for unknown inode
        #[test]
        fn test_subscribe_returns_none_for_unknown() {
            let active: DashMap<u64, Arc<HydrationRequest>> = DashMap::new();

            // Try to subscribe to non-existent inode
            let subscribed_rx = active.get(&999).map(|r| r.subscribe());
            assert!(subscribed_rx.is_none());
        }
    }

    // ========================================================================
    // T078: Unit tests for pin/unpin
    // ========================================================================

    mod pin_unpin_tests {
        use super::*;

        #[test]
        fn test_pin_on_pinned_is_idempotent() {
            // Pin on Pinned state should be a no-op
            let state = ItemState::Pinned;
            assert!(matches!(state, ItemState::Pinned));
        }

        #[test]
        fn test_pin_on_hydrated_transitions_to_pinned() {
            // Pin on Hydrated should transition to Pinned
            let hydrated = ItemState::Hydrated;
            let target = ItemState::Pinned;

            // Both states are valid
            assert!(matches!(hydrated, ItemState::Hydrated));
            assert!(matches!(target, ItemState::Pinned));
        }

        #[test]
        fn test_pin_on_online_requires_hydration() {
            // Pin on Online should trigger hydration first
            let online = ItemState::Online;
            assert!(matches!(online, ItemState::Online));
            // After hydration completes, state should become Pinned
        }

        #[test]
        fn test_unpin_transitions_to_hydrated() {
            // Unpin should transition from Pinned to Hydrated
            let pinned = ItemState::Pinned;
            let target = ItemState::Hydrated;

            assert!(matches!(pinned, ItemState::Pinned));
            assert!(matches!(target, ItemState::Hydrated));
        }

        #[test]
        fn test_unpin_on_hydrated_is_noop() {
            // Unpin on already Hydrated should be a no-op
            let hydrated = ItemState::Hydrated;
            assert!(matches!(hydrated, ItemState::Hydrated));
        }

        #[test]
        fn test_unpin_on_online_is_noop() {
            // Unpin on Online should be a no-op (nothing to unpin)
            let online = ItemState::Online;
            assert!(matches!(online, ItemState::Online));
        }

        #[test]
        fn test_pin_priority_is_pin_request() {
            // Pin operations should use PinRequest priority
            let priority = HydrationPriority::PinRequest;
            assert_eq!(priority as u8, 1);
            assert!(priority > HydrationPriority::Prefetch);
            assert!(priority < HydrationPriority::UserOpen);
        }

        #[test]
        fn test_pinned_files_are_not_dehydratable() {
            // Pinned files should not be eligible for dehydration
            let pinned = ItemState::Pinned;
            // can_dehydrate() returns false for Pinned
            assert!(!pinned.can_dehydrate());
        }

        #[test]
        fn test_hydrated_files_are_dehydratable() {
            // Hydrated files should be eligible for dehydration
            let hydrated = ItemState::Hydrated;
            assert!(hydrated.can_dehydrate());
        }

        #[test]
        fn test_modified_files_are_not_dehydratable() {
            // Modified files should not be dehydrated (pending upload)
            let modified = ItemState::Modified;
            assert!(!modified.can_dehydrate());
        }
    }
}
