//! File synchronization use case
//!
//! Orchestrates the upload and download of individual files between
//! the local filesystem and the cloud provider. Handles hash comparison,
//! sync direction determination, and state transitions.

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::domain::newtypes::FileHash;
use crate::domain::{AuditAction, AuditEntry, AuditResult, SyncItem};
use crate::ports::{ICloudProvider, ILocalFileSystem, IStateRepository};

/// Threshold in bytes for choosing simple PUT upload vs. resumable session upload.
/// Files smaller than 4 MB use a simple PUT request.
const SIMPLE_UPLOAD_THRESHOLD: u64 = 4 * 1024 * 1024;

/// Use case for synchronizing individual files
///
/// Coordinates file transfers between local filesystem and cloud provider,
/// comparing hashes to determine sync direction and verifying integrity
/// after transfer.
pub struct SyncFileUseCase {
    cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
    state_repository: Arc<dyn IStateRepository + Send + Sync>,
    local_filesystem: Arc<dyn ILocalFileSystem + Send + Sync>,
}

impl SyncFileUseCase {
    /// Creates a new SyncFileUseCase with the required dependencies
    ///
    /// # Arguments
    ///
    /// * `cloud_provider` - Cloud storage provider for upload/download operations
    /// * `state_repository` - Persistent storage for sync state and audit log
    /// * `local_filesystem` - Local filesystem operations for reading/writing files
    pub fn new(
        cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
        state_repository: Arc<dyn IStateRepository + Send + Sync>,
        local_filesystem: Arc<dyn ILocalFileSystem + Send + Sync>,
    ) -> Self {
        Self {
            cloud_provider,
            state_repository,
            local_filesystem,
        }
    }

    /// Synchronizes a single item by comparing hashes and transferring in the appropriate direction
    ///
    /// This method:
    /// 1. Compares local and remote content hashes
    /// 2. Determines the sync direction (upload if local is newer, download if remote is newer)
    /// 3. Performs the transfer via `upload` or `download`
    /// 4. Updates the sync item state in the repository
    ///
    /// # Arguments
    ///
    /// * `item` - The sync item to synchronize
    ///
    /// # Returns
    ///
    /// The updated SyncItem after synchronization
    ///
    /// # Errors
    ///
    /// Returns an error if hash comparison, transfer, or state update fails
    pub async fn sync_single(&self, item: &SyncItem) -> Result<SyncItem> {
        // Skip directories - they don't need content sync
        if item.is_directory() {
            return Ok(item.clone());
        }

        // Determine sync direction based on hash comparison and timestamps
        let needs_upload = item.state().has_pending_changes();
        let needs_download = item.state().is_placeholder() || !item.hashes_match();

        let updated_item = if needs_upload {
            self.upload(item)
                .await
                .context("Failed to upload file to cloud")?
        } else if needs_download {
            self.download(item)
                .await
                .context("Failed to download file from cloud")?
        } else {
            // Already in sync
            item.clone()
        };

        // Persist the updated item state
        self.state_repository
            .save_item(&updated_item)
            .await
            .context("Failed to persist sync item state after transfer")?;

        Ok(updated_item)
    }

    /// Uploads a local file to the cloud provider
    ///
    /// This method:
    /// 1. Reads the file content from the local filesystem
    /// 2. Chooses upload method based on file size:
    ///    - Simple PUT for files < 4MB
    ///    - Resumable upload session for larger files
    /// 3. Verifies the content hash after upload
    /// 4. Updates the SyncItem state to Hydrated
    ///
    /// # Arguments
    ///
    /// * `item` - The sync item to upload
    ///
    /// # Returns
    ///
    /// The updated SyncItem with Hydrated state and matching hashes
    ///
    /// # Errors
    ///
    /// Returns an error if file read, upload, or hash verification fails
    pub async fn upload(&self, item: &SyncItem) -> Result<SyncItem> {
        let mut updated_item = item.clone();

        // Step 1: Read the local file content
        let content = self
            .local_filesystem
            .read_file(item.local_path())
            .await
            .context("Failed to read local file for upload")?;

        // Step 2: Extract parent path and file name from remote path
        let remote_path = item.remote_path();
        let parent_path = remote_path
            .parent()
            .context("Remote path has no parent directory")?;
        let file_name = remote_path
            .file_name()
            .context("Remote path has no file name")?;

        // Step 3: Upload based on file size
        let delta_item = if item.size_bytes() < SIMPLE_UPLOAD_THRESHOLD {
            // Simple PUT upload for small files
            self.cloud_provider
                .upload_file(&parent_path, file_name, &content)
                .await
                .context("Failed to upload small file via PUT")?
        } else {
            // Resumable upload session for larger files
            self.cloud_provider
                .upload_file_session(&parent_path, file_name, &content, None)
                .await
                .context("Failed to upload large file via session")?
        };

        // Step 4: Extract and convert the remote hash from the DeltaItem response
        let remote_hash = match delta_item.hash {
            Some(hash_str) => FileHash::try_from(hash_str)
                .context("Failed to parse hash returned by cloud provider")?,
            None => bail!("Cloud provider did not return a content hash after upload"),
        };

        // Step 5: Verify hash integrity after upload
        if let Some(local_hash) = updated_item.local_hash() {
            if *local_hash != remote_hash {
                bail!(
                    "Hash mismatch after upload: local={}, remote={}",
                    local_hash,
                    remote_hash
                );
            }
        }

        // Step 6: Update item state
        updated_item.set_content_hash(remote_hash.clone());
        updated_item.set_local_hash(remote_hash);
        updated_item
            .complete_sync()
            .context("Invalid state transition to Hydrated after upload")?;

        // Step 7: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::FileUpload, AuditResult::success())
            .with_details(json!({
                "path": item.local_path().to_string(),
                "remote_path": item.remote_path().to_string(),
                "size_bytes": item.size_bytes(),
                "upload_method": if item.size_bytes() < SIMPLE_UPLOAD_THRESHOLD {
                    "simple_put"
                } else {
                    "resumable_session"
                },
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record upload audit entry")?;

        Ok(updated_item)
    }

    /// Downloads a file from the cloud provider to the local filesystem
    ///
    /// This method:
    /// 1. Streams the file content from the cloud provider
    /// 2. Writes the content to the local filesystem
    /// 3. Verifies the content hash after download
    /// 4. Updates the SyncItem state to Hydrated
    ///
    /// # Arguments
    ///
    /// * `item` - The sync item to download
    ///
    /// # Returns
    ///
    /// The updated SyncItem with Hydrated state and matching hashes
    ///
    /// # Errors
    ///
    /// Returns an error if download, write, or hash verification fails
    pub async fn download(&self, item: &SyncItem) -> Result<SyncItem> {
        let mut updated_item = item.clone();

        // Step 1: Transition to Hydrating state
        updated_item
            .start_hydrating()
            .context("Invalid state transition to Hydrating for download")?;

        // Step 2: Get the remote ID (required for download)
        let remote_id = item
            .remote_id()
            .context("Cannot download file without a remote ID")?;

        // Step 3: Download content from cloud provider
        let content = self
            .cloud_provider
            .download_file(remote_id)
            .await
            .context("Failed to download file from cloud provider")?;

        // Step 4: Write to local filesystem
        self.local_filesystem
            .write_file(item.local_path(), &content)
            .await
            .context("Failed to write downloaded file to local filesystem")?;

        // Step 5: Compute and verify hash
        let local_hash = self
            .local_filesystem
            .compute_hash(item.local_path())
            .await
            .context("Failed to compute hash of downloaded file")?;

        if let Some(remote_hash) = item.content_hash() {
            if local_hash != *remote_hash {
                bail!(
                    "Hash mismatch after download: local={}, remote={}",
                    local_hash,
                    remote_hash
                );
            }
        }

        // Step 6: Update item state
        updated_item.set_local_hash(local_hash);
        updated_item
            .complete_hydration()
            .context("Invalid state transition to Hydrated after download")?;
        updated_item.mark_synced();

        // Step 7: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::FileDownload, AuditResult::success())
            .with_details(json!({
                "path": item.local_path().to_string(),
                "remote_path": item.remote_path().to_string(),
                "size_bytes": item.size_bytes(),
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record download audit entry")?;

        Ok(updated_item)
    }
}
