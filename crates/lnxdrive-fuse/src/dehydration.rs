//! Space reclamation through file dehydration.
//!
//! Provides `DehydrationManager` for converting fully-hydrated files back
//! to dehydrated placeholders to free local disk space.
//!
//! ## Dehydration Policy
//!
//! Files are candidates for dehydration when:
//! - State is `Hydrated` (not `Pinned`, `Modified`, `Online`, etc.)
//! - `last_accessed` is older than `max_age_days`
//! - No open file handles
//!
//! Dehydration is triggered when cache disk usage exceeds the threshold.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │  DehydrationManager │
//! │                     │
//! │  policy: Policy     │
//! │  cache: ContentCache│
//! │  inode_table: Arc   │
//! │  write_handle       │
//! │  db_pool            │
//! └─────────────────────┘
//!           │
//!           │ run_sweep() every N minutes
//!           ▼
//! ┌─────────────────────┐
//! │  1. Check usage     │ ─── if < threshold, skip
//! │  2. Query DB        │ ─── get_items_for_dehydration()
//! │  3. For each item:  │
//! │     - Check handles │ ─── skip if open
//! │     - Remove cache  │ ─── cache.remove()
//! │     - Update state  │ ─── Hydrated → Online
//! └─────────────────────┘
//! ```

use std::sync::Arc;

use lnxdrive_cache::pool::DatabasePool;
use lnxdrive_core::{config::FuseConfig, domain::sync_item::ItemState};
use tokio::{sync::RwLock, task::JoinHandle, time};
use tracing::{debug, error, info, warn};

use crate::{cache::ContentCache, error::FuseError, inode::InodeTable, write_serializer::WriteSerializerHandle};

// ============================================================================
// T079: DehydrationPolicy struct
// ============================================================================

/// Policy configuration for automatic dehydration.
///
/// Determines when and how dehydration sweeps occur.
#[derive(Debug, Clone)]
pub struct DehydrationPolicy {
    /// Maximum cache size in bytes.
    pub cache_max_bytes: u64,
    /// Percentage of cache_max_bytes that triggers dehydration (0-100).
    pub threshold_percent: u8,
    /// Maximum age in days before a cached file becomes eligible for dehydration.
    pub max_age_days: u32,
    /// Interval in minutes between dehydration background tasks.
    pub interval_minutes: u32,
}

impl DehydrationPolicy {
    /// Create a policy from FUSE configuration.
    ///
    /// Converts `cache_max_size_gb` to bytes and copies other fields.
    pub fn from_config(config: &FuseConfig) -> Self {
        Self {
            cache_max_bytes: (config.cache_max_size_gb as u64) * 1024 * 1024 * 1024,
            threshold_percent: config.dehydration_threshold_percent,
            max_age_days: config.dehydration_max_age_days,
            interval_minutes: config.dehydration_interval_minutes,
        }
    }

    /// Calculate the threshold in bytes that triggers dehydration.
    pub fn threshold_bytes(&self) -> u64 {
        (self.cache_max_bytes * self.threshold_percent as u64) / 100
    }
}

impl Default for DehydrationPolicy {
    fn default() -> Self {
        Self {
            cache_max_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            threshold_percent: 80,
            max_age_days: 30,
            interval_minutes: 60,
        }
    }
}

// ============================================================================
// T080: DehydrationManager struct
// ============================================================================

/// Report of a dehydration operation.
#[derive(Debug, Clone, Default)]
pub struct DehydrationReport {
    /// Number of files successfully dehydrated.
    pub dehydrated_count: usize,
    /// Total bytes freed by dehydration.
    pub bytes_freed: u64,
    /// Number of files skipped (open handles, wrong state, etc.).
    pub skipped_count: usize,
    /// Number of errors encountered.
    pub error_count: usize,
    /// Error messages for failed items.
    pub errors: Vec<String>,
}

impl DehydrationReport {
    /// Merge another report into this one.
    pub fn merge(&mut self, other: DehydrationReport) {
        self.dehydrated_count += other.dehydrated_count;
        self.bytes_freed += other.bytes_freed;
        self.skipped_count += other.skipped_count;
        self.error_count += other.error_count;
        self.errors.extend(other.errors);
    }
}

/// Manages automatic dehydration of cached files to reclaim disk space.
///
/// Periodically sweeps the cache and removes content for files that:
/// - Are in `Hydrated` state (not pinned, modified, etc.)
/// - Haven't been accessed recently
/// - Don't have open file handles
pub struct DehydrationManager {
    /// Policy for dehydration decisions.
    policy: DehydrationPolicy,
    /// Content cache for storing/removing file data.
    cache: Arc<ContentCache>,
    /// Inode table for checking open handles.
    inode_table: Arc<InodeTable>,
    /// Handle for serialized DB writes.
    write_handle: WriteSerializerHandle,
    /// Database pool for querying candidates.
    db_pool: DatabasePool,
    /// Flag to signal shutdown.
    shutdown: Arc<RwLock<bool>>,
}

impl DehydrationManager {
    /// Create a new DehydrationManager.
    ///
    /// # Arguments
    ///
    /// * `policy` - Dehydration policy configuration
    /// * `cache` - Content cache instance
    /// * `inode_table` - Inode table for checking handles
    /// * `write_handle` - Handle for state updates
    /// * `db_pool` - Database pool for queries
    pub fn new(
        policy: DehydrationPolicy,
        cache: Arc<ContentCache>,
        inode_table: Arc<InodeTable>,
        write_handle: WriteSerializerHandle,
        db_pool: DatabasePool,
    ) -> Self {
        Self {
            policy,
            cache,
            inode_table,
            write_handle,
            db_pool,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Get the policy used by this manager.
    pub fn policy(&self) -> &DehydrationPolicy {
        &self.policy
    }

    /// Notify the dehydration manager that a file's last handle was closed.
    ///
    /// If the cache is above the dehydration threshold, this will attempt
    /// to dehydrate the file immediately (if eligible). Otherwise, the file
    /// will be picked up by the next periodic sweep.
    ///
    /// # Arguments
    ///
    /// * `ino` - The inode number of the file that was released
    pub async fn notify_file_closed(&self, ino: u64) {
        // Check if cache is over threshold
        let current_usage = match self.cache.disk_usage() {
            Ok(u) => u,
            Err(_) => return,
        };
        let threshold = self.policy.threshold_bytes();

        if current_usage > threshold {
            debug!(
                ino = ino,
                usage_mb = current_usage / (1024 * 1024),
                threshold_mb = threshold / (1024 * 1024),
                "Cache over threshold, attempting immediate dehydration of released file"
            );
            if let Err(e) = self.dehydrate_path(ino).await {
                debug!(
                    ino = ino,
                    error = %e,
                    "Immediate dehydration skipped for released file"
                );
            }
        }
    }
}

// ============================================================================
// T081: DehydrationManager::run_sweep()
// ============================================================================

impl DehydrationManager {
    /// Run a dehydration sweep to reclaim disk space.
    ///
    /// Checks cache usage against the threshold. If above, queries the database
    /// for dehydration candidates and processes them until usage drops below
    /// the threshold or no more candidates are available.
    ///
    /// # Returns
    ///
    /// A report of the dehydration operation.
    pub async fn run_sweep(&self) -> Result<DehydrationReport, FuseError> {
        let mut report = DehydrationReport::default();

        // Check current cache usage
        let current_usage = self.cache.disk_usage()?;
        let threshold = self.policy.threshold_bytes();

        info!(
            current_usage_mb = current_usage / (1024 * 1024),
            threshold_mb = threshold / (1024 * 1024),
            "Starting dehydration sweep"
        );

        // If below threshold, no need to dehydrate
        if current_usage < threshold {
            debug!("Cache usage below threshold, skipping sweep");
            return Ok(report);
        }

        // Calculate how much space we need to free
        let target_usage = threshold * 80 / 100; // Target 80% of threshold
        let bytes_to_free = current_usage.saturating_sub(target_usage);

        debug!(
            bytes_to_free_mb = bytes_to_free / (1024 * 1024),
            "Need to free space"
        );

        // Get repository for queries
        let repo = lnxdrive_cache::SqliteStateRepository::new(self.db_pool.pool().clone());

        // Process candidates in batches until we've freed enough space
        let mut total_freed = 0u64;
        let batch_size = 100u32;

        loop {
            // Check for shutdown
            if *self.shutdown.read().await {
                debug!("Shutdown requested, stopping sweep");
                break;
            }

            // Query for dehydration candidates
            use lnxdrive_core::ports::IStateRepository;
            let candidates = repo
                .get_items_for_dehydration(self.policy.max_age_days, batch_size)
                .await
                .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

            if candidates.is_empty() {
                debug!("No more dehydration candidates");
                break;
            }

            debug!(count = candidates.len(), "Processing dehydration candidates");

            // Process each candidate
            for item in candidates {
                // Check for shutdown
                if *self.shutdown.read().await {
                    break;
                }

                // Skip if not in Hydrated state (defensive check)
                if !matches!(item.state(), ItemState::Hydrated) {
                    report.skipped_count += 1;
                    continue;
                }

                // Check for open handles in inode table
                if let Some(inode) = item.inode() {
                    if let Some(entry) = self.inode_table.get(inode) {
                        if entry.open_handles() > 0 {
                            debug!(
                                ino = inode,
                                handles = entry.open_handles(),
                                "Skipping file with open handles"
                            );
                            report.skipped_count += 1;
                            continue;
                        }
                    }
                }

                // Get file size before removal
                let file_size = if let Some(remote_id) = item.remote_id() {
                    let cache_path = self.cache.cache_path(remote_id);
                    if cache_path.exists() {
                        std::fs::metadata(&cache_path)
                            .map(|m| m.len())
                            .unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                // Remove cached content
                if let Some(remote_id) = item.remote_id() {
                    match self.cache.remove(remote_id) {
                        Ok(()) => {
                            // Update state to Online via WriteSerializer
                            match self
                                .write_handle
                                .update_state(*item.id(), ItemState::Online)
                                .await
                            {
                                Ok(()) => {
                                    // Note: InodeTable entry state will be refreshed when
                                    // the file is next accessed. Database is source of truth.

                                    debug!(
                                        path = %item.local_path(),
                                        freed_bytes = file_size,
                                        "Dehydrated file"
                                    );

                                    report.dehydrated_count += 1;
                                    report.bytes_freed += file_size;
                                    total_freed += file_size;
                                }
                                Err(e) => {
                                    warn!(
                                        path = %item.local_path(),
                                        error = %e,
                                        "Failed to update state after dehydration"
                                    );
                                    report.error_count += 1;
                                    report.errors.push(format!(
                                        "State update failed for {}: {}",
                                        item.local_path(),
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                path = %item.local_path(),
                                error = %e,
                                "Failed to remove cached content"
                            );
                            report.error_count += 1;
                            report.errors.push(format!(
                                "Cache removal failed for {}: {}",
                                item.local_path(),
                                e
                            ));
                        }
                    }
                } else {
                    // No remote_id means nothing to dehydrate
                    report.skipped_count += 1;
                }

                // Check if we've freed enough space
                if total_freed >= bytes_to_free {
                    debug!(
                        freed_mb = total_freed / (1024 * 1024),
                        target_mb = bytes_to_free / (1024 * 1024),
                        "Freed target amount of space"
                    );
                    break;
                }
            }

            // If we've freed enough, stop
            if total_freed >= bytes_to_free {
                break;
            }
        }

        info!(
            dehydrated = report.dehydrated_count,
            freed_mb = report.bytes_freed / (1024 * 1024),
            skipped = report.skipped_count,
            errors = report.error_count,
            "Dehydration sweep complete"
        );

        Ok(report)
    }
}

// ============================================================================
// T082: DehydrationManager::start_periodic()
// ============================================================================

impl DehydrationManager {
    /// Start periodic dehydration sweeps.
    ///
    /// Spawns a tokio task that runs `run_sweep()` at the configured interval.
    ///
    /// # Returns
    ///
    /// A `JoinHandle` for the background task. Drop or abort to stop sweeps.
    pub fn start_periodic(self: Arc<Self>) -> JoinHandle<()> {
        let interval_minutes = self.policy.interval_minutes;

        tokio::spawn(async move {
            let mut interval = time::interval(time::Duration::from_secs(
                interval_minutes as u64 * 60,
            ));

            // Skip the first immediate tick
            interval.tick().await;

            loop {
                interval.tick().await;

                // Check for shutdown
                if *self.shutdown.read().await {
                    info!("Periodic dehydration task shutting down");
                    break;
                }

                debug!("Running periodic dehydration sweep");

                match self.run_sweep().await {
                    Ok(report) => {
                        if report.dehydrated_count > 0 {
                            info!(
                                dehydrated = report.dehydrated_count,
                                freed_mb = report.bytes_freed / (1024 * 1024),
                                "Periodic sweep freed space"
                            );
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Periodic dehydration sweep failed");
                    }
                }
            }
        })
    }

    /// Signal the manager to stop periodic sweeps.
    pub async fn shutdown(&self) {
        *self.shutdown.write().await = true;
    }
}

// ============================================================================
// T083: Manual dehydration methods
// ============================================================================

impl DehydrationManager {
    /// Manually dehydrate a specific file.
    ///
    /// Checks that the file is eligible (Hydrated state, no open handles)
    /// and dehydrates it.
    ///
    /// # Arguments
    ///
    /// * `ino` - FUSE inode number of the file to dehydrate
    ///
    /// # Returns
    ///
    /// Number of bytes freed, or an error if dehydration is not allowed.
    pub async fn dehydrate_path(&self, ino: u64) -> Result<u64, FuseError> {
        // Look up the inode entry
        let entry = self.inode_table.get(ino).ok_or_else(|| {
            FuseError::NotFound(format!("Inode {} not found", ino))
        })?;

        // Check if file is open
        if entry.open_handles() > 0 {
            return Err(FuseError::PermissionDenied(format!(
                "Cannot dehydrate file with {} open handles",
                entry.open_handles()
            )));
        }

        // Check state
        let state = entry.state();
        if !matches!(state, ItemState::Hydrated) {
            return Err(FuseError::InvalidArgument(format!(
                "Cannot dehydrate file in state {:?}. Only Hydrated files can be dehydrated.",
                state
            )));
        }

        // Get file info (clone to release the borrow)
        let remote_id = entry.remote_id().cloned().ok_or_else(|| {
            FuseError::InvalidArgument("File has no remote ID".to_string())
        })?;
        let item_id = *entry.item_id();

        // Drop the entry ref before async operations
        drop(entry);

        // Get file size
        let cache_path = self.cache.cache_path(&remote_id);
        let file_size = if cache_path.exists() {
            std::fs::metadata(&cache_path)
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        // Remove cached content
        self.cache.remove(&remote_id)?;

        // Update state to Online
        self.write_handle
            .update_state(item_id, ItemState::Online)
            .await
            .map_err(|e| FuseError::DatabaseError(e.to_string()))?;

        // Note: InodeTable entry state will be refreshed when the file is next accessed.
        // Database is the source of truth for state.

        info!(ino, freed_bytes = file_size, "Manually dehydrated file");

        Ok(file_size)
    }

    /// Manually dehydrate multiple files.
    ///
    /// Processes each file and returns a comprehensive report.
    ///
    /// # Arguments
    ///
    /// * `inos` - List of FUSE inode numbers to dehydrate
    ///
    /// # Returns
    ///
    /// A report of the dehydration operation.
    pub async fn dehydrate_paths(&self, inos: Vec<u64>) -> Result<DehydrationReport, FuseError> {
        let mut report = DehydrationReport::default();

        for ino in inos {
            match self.dehydrate_path(ino).await {
                Ok(freed_bytes) => {
                    report.dehydrated_count += 1;
                    report.bytes_freed += freed_bytes;
                }
                Err(FuseError::NotFound(msg)) => {
                    report.skipped_count += 1;
                    report.errors.push(format!("Not found: {}", msg));
                }
                Err(FuseError::PermissionDenied(msg)) => {
                    report.skipped_count += 1;
                    report.errors.push(format!("Permission denied: {}", msg));
                }
                Err(FuseError::InvalidArgument(msg)) => {
                    report.skipped_count += 1;
                    report.errors.push(format!("Invalid state: {}", msg));
                }
                Err(e) => {
                    report.error_count += 1;
                    report.errors.push(format!("Error dehydrating {}: {}", ino, e));
                }
            }
        }

        Ok(report)
    }
}

impl std::fmt::Debug for DehydrationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DehydrationManager")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// T087: Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    mod dehydration_policy_tests {
        use super::*;

        #[test]
        fn test_from_config() {
            let config = FuseConfig {
                mount_point: "~/OneDrive".to_string(),
                auto_mount: true,
                cache_dir: "~/.local/share/lnxdrive/cache".to_string(),
                cache_max_size_gb: 20,
                dehydration_threshold_percent: 75,
                dehydration_max_age_days: 14,
                dehydration_interval_minutes: 30,
                hydration_concurrency: 8,
            };

            let policy = DehydrationPolicy::from_config(&config);

            assert_eq!(policy.cache_max_bytes, 20 * 1024 * 1024 * 1024);
            assert_eq!(policy.threshold_percent, 75);
            assert_eq!(policy.max_age_days, 14);
            assert_eq!(policy.interval_minutes, 30);
        }

        #[test]
        fn test_threshold_bytes() {
            let policy = DehydrationPolicy {
                cache_max_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
                threshold_percent: 80,
                max_age_days: 30,
                interval_minutes: 60,
            };

            let threshold = policy.threshold_bytes();
            assert_eq!(threshold, 8 * 1024 * 1024 * 1024); // 8 GB (80% of 10 GB)
        }

        #[test]
        fn test_default_policy() {
            let policy = DehydrationPolicy::default();

            assert_eq!(policy.cache_max_bytes, 10 * 1024 * 1024 * 1024);
            assert_eq!(policy.threshold_percent, 80);
            assert_eq!(policy.max_age_days, 30);
            assert_eq!(policy.interval_minutes, 60);
        }

        #[test]
        fn test_threshold_bytes_edge_cases() {
            // 0% threshold
            let policy_zero = DehydrationPolicy {
                cache_max_bytes: 1024 * 1024 * 1024,
                threshold_percent: 0,
                ..Default::default()
            };
            assert_eq!(policy_zero.threshold_bytes(), 0);

            // 100% threshold
            let policy_full = DehydrationPolicy {
                cache_max_bytes: 1024 * 1024 * 1024,
                threshold_percent: 100,
                ..Default::default()
            };
            assert_eq!(policy_full.threshold_bytes(), 1024 * 1024 * 1024);

            // 50% threshold
            let policy_half = DehydrationPolicy {
                cache_max_bytes: 2 * 1024 * 1024 * 1024,
                threshold_percent: 50,
                ..Default::default()
            };
            assert_eq!(policy_half.threshold_bytes(), 1024 * 1024 * 1024);
        }

        #[test]
        fn test_policy_clone() {
            let policy = DehydrationPolicy {
                cache_max_bytes: 5 * 1024 * 1024 * 1024,
                threshold_percent: 90,
                max_age_days: 7,
                interval_minutes: 15,
            };

            let cloned = policy.clone();
            assert_eq!(policy.cache_max_bytes, cloned.cache_max_bytes);
            assert_eq!(policy.threshold_percent, cloned.threshold_percent);
            assert_eq!(policy.max_age_days, cloned.max_age_days);
            assert_eq!(policy.interval_minutes, cloned.interval_minutes);
        }

        #[test]
        fn test_policy_debug() {
            let policy = DehydrationPolicy::default();
            let debug_str = format!("{:?}", policy);
            assert!(debug_str.contains("DehydrationPolicy"));
            assert!(debug_str.contains("cache_max_bytes"));
            assert!(debug_str.contains("threshold_percent"));
        }
    }

    mod dehydration_report_tests {
        use super::*;

        #[test]
        fn test_default_report() {
            let report = DehydrationReport::default();

            assert_eq!(report.dehydrated_count, 0);
            assert_eq!(report.bytes_freed, 0);
            assert_eq!(report.skipped_count, 0);
            assert_eq!(report.error_count, 0);
            assert!(report.errors.is_empty());
        }

        #[test]
        fn test_report_merge() {
            let mut report1 = DehydrationReport {
                dehydrated_count: 5,
                bytes_freed: 1000,
                skipped_count: 2,
                error_count: 1,
                errors: vec!["Error 1".to_string()],
            };

            let report2 = DehydrationReport {
                dehydrated_count: 3,
                bytes_freed: 500,
                skipped_count: 1,
                error_count: 2,
                errors: vec!["Error 2".to_string(), "Error 3".to_string()],
            };

            report1.merge(report2);

            assert_eq!(report1.dehydrated_count, 8);
            assert_eq!(report1.bytes_freed, 1500);
            assert_eq!(report1.skipped_count, 3);
            assert_eq!(report1.error_count, 3);
            assert_eq!(report1.errors.len(), 3);
        }

        #[test]
        fn test_report_debug() {
            let report = DehydrationReport {
                dehydrated_count: 10,
                bytes_freed: 1024 * 1024,
                skipped_count: 5,
                error_count: 0,
                errors: vec![],
            };

            let debug_str = format!("{:?}", report);
            assert!(debug_str.contains("DehydrationReport"));
            assert!(debug_str.contains("dehydrated_count: 10"));
            assert!(debug_str.contains("bytes_freed: 1048576"));
        }
    }

    mod dehydration_manager_tests {
        use super::*;

        #[test]
        fn test_policy_accessor() {
            // We can't easily create a full DehydrationManager in unit tests
            // due to dependencies, but we can test the policy methods
            let policy = DehydrationPolicy::default();
            assert_eq!(policy.threshold_bytes(), 8 * 1024 * 1024 * 1024);
        }

        #[test]
        fn test_item_state_can_dehydrate() {
            // Test that only Hydrated items can be dehydrated
            assert!(ItemState::Hydrated.can_dehydrate());
            assert!(!ItemState::Pinned.can_dehydrate());
            assert!(!ItemState::Modified.can_dehydrate());
            assert!(!ItemState::Online.can_dehydrate());
            assert!(!ItemState::Hydrating.can_dehydrate());
            assert!(!ItemState::Deleted.can_dehydrate());
            assert!(!ItemState::Conflicted.can_dehydrate());
        }

        #[test]
        fn test_report_skipped_reasons() {
            // Test that reports correctly track different skip reasons
            let report = DehydrationReport {
                dehydrated_count: 0,
                bytes_freed: 0,
                skipped_count: 3,
                error_count: 0,
                errors: vec![
                    "Permission denied: Cannot dehydrate file with 2 open handles".to_string(),
                    "Invalid state: Cannot dehydrate file in state Pinned".to_string(),
                    "Not found: Inode 123 not found".to_string(),
                ],
            };

            assert_eq!(report.skipped_count, 3);
            assert!(report.errors.iter().any(|e| e.contains("open handles")));
            assert!(report.errors.iter().any(|e| e.contains("Pinned")));
            assert!(report.errors.iter().any(|e| e.contains("not found")));
        }
    }
}
