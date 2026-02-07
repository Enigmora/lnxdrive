//! Conflict resolution executor
//!
//! Applies resolution strategies by performing the actual file operations:
//! - `KeepLocal`: upload local version to cloud
//! - `KeepRemote`: download remote version to replace local
//! - `KeepBoth`: rename local with conflict suffix, download remote

use std::sync::Arc;

use tracing::{debug, info, warn};

use lnxdrive_core::{
    domain::{
        conflict::{Conflict, Resolution, ResolutionSource},
        newtypes::{RemotePath, SyncPath},
        sync_item::SyncItem,
    },
    ports::{
        cloud_provider::ICloudProvider,
        local_filesystem::ILocalFileSystem,
        state_repository::IStateRepository,
    },
};

use crate::{error::ConflictError, namer::ConflictNamer};

/// Result of a batch resolution operation
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub resolved: u32,
    pub failed: u32,
    pub errors: Vec<String>,
}

/// Applies conflict resolutions with real file operations
pub struct ConflictResolver {
    cloud_provider: Arc<dyn ICloudProvider>,
    local_filesystem: Arc<dyn ILocalFileSystem>,
    state_repository: Arc<dyn IStateRepository>,
}

impl ConflictResolver {
    pub fn new(
        cloud_provider: Arc<dyn ICloudProvider>,
        local_filesystem: Arc<dyn ILocalFileSystem>,
        state_repository: Arc<dyn IStateRepository>,
    ) -> Self {
        Self {
            cloud_provider,
            local_filesystem,
            state_repository,
        }
    }

    /// Apply a resolution to a conflict
    ///
    /// Performs the actual file operations and updates the conflict record.
    pub async fn apply_resolution(
        &self,
        conflict: Conflict,
        resolution: Resolution,
        source: ResolutionSource,
        item: &SyncItem,
        sync_root: &SyncPath,
    ) -> Result<Conflict, ConflictError> {
        info!(
            conflict_id = %conflict.id(),
            resolution = %resolution,
            path = %item.local_path(),
            "Applying conflict resolution"
        );

        if conflict.is_resolved() {
            return Err(ConflictError::AlreadyResolved(conflict.id().to_string()));
        }

        match &resolution {
            Resolution::KeepLocal => {
                let remote_etag = conflict.remote_version().etag();
                self.apply_keep_local(item, sync_root, remote_etag).await?;
            }
            Resolution::KeepRemote => {
                self.apply_keep_remote(item).await?;
            }
            Resolution::KeepBoth => {
                self.apply_keep_both(item).await?;
            }
            Resolution::Manual => {
                debug!("Manual resolution - no file operations");
            }
        }

        // Resolve the conflict entity
        let resolved = conflict.resolve(resolution, source);

        // Persist the resolved conflict
        self.state_repository
            .save_conflict(&resolved)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("save conflict: {e}")))?;

        // Transition item from Conflicted to Hydrated
        let mut updated_item = item.clone();
        updated_item
            .resolve_conflict()
            .map_err(|e| ConflictError::ResolutionFailed(format!("state transition: {e}")))?;
        updated_item.mark_synced();
        self.state_repository
            .save_item(&updated_item)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("save item: {e}")))?;

        info!(
            conflict_id = %resolved.id(),
            "Conflict resolved successfully"
        );

        Ok(resolved)
    }

    /// Keep local version: upload it to the cloud, overwriting remote
    async fn apply_keep_local(
        &self,
        item: &SyncItem,
        sync_root: &SyncPath,
        remote_etag: Option<&str>,
    ) -> Result<(), ConflictError> {
        debug!(path = %item.local_path(), "Applying keep-local: uploading local version");

        let data = self
            .local_filesystem
            .read_file(item.local_path())
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("read local file: {e}")))?;

        let relative = item
            .local_path()
            .relative_to(sync_root)
            .map_err(|e| ConflictError::ResolutionFailed(format!("compute relative path: {e}")))?;

        let remote_path_str = format!("/{}", relative.display()).replace('\\', "/");
        let (parent_path, file_name) = split_remote_path(&remote_path_str)?;

        let _result = self
            .cloud_provider
            .upload_file(&parent_path, &file_name, &data, remote_etag)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("upload: {e}")))?;

        Ok(())
    }

    /// Keep remote version: download and replace local
    async fn apply_keep_remote(&self, item: &SyncItem) -> Result<(), ConflictError> {
        debug!(path = %item.local_path(), "Applying keep-remote: downloading remote version");

        let remote_id = item
            .remote_id()
            .ok_or_else(|| ConflictError::ResolutionFailed("item has no remote ID".to_string()))?
            .clone();

        let data = self
            .cloud_provider
            .download_file(&remote_id)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("download: {e}")))?;

        self.local_filesystem
            .write_file(item.local_path(), &data)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("write local file: {e}")))?;

        Ok(())
    }

    /// Keep both: rename local with conflict suffix, download remote as original
    async fn apply_keep_both(
        &self,
        item: &SyncItem,
    ) -> Result<(), ConflictError> {
        let local_path = item.local_path();
        let path_buf = local_path.as_path();

        let file_name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        debug!(
            path = %local_path,
            "Applying keep-both: rename local + download remote"
        );

        // Generate conflict copy name, checking for existing files on disk
        let parent_dir = path_buf
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path_buf.clone());

        let conflict_name = ConflictNamer::generate_unique(file_name, |candidate| {
            parent_dir.join(candidate).exists()
        });

        let conflict_path_buf = parent_dir.join(&conflict_name);

        // Rename local file to conflict copy using tokio::fs
        tokio::fs::rename(path_buf, &conflict_path_buf)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("rename local: {e}")))?;

        // Download remote to original path
        let remote_id = item
            .remote_id()
            .ok_or_else(|| ConflictError::ResolutionFailed("item has no remote ID".to_string()))?
            .clone();

        let data = self
            .cloud_provider
            .download_file(&remote_id)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("download remote: {e}")))?;

        self.local_filesystem
            .write_file(local_path, &data)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("write remote to original: {e}")))?;

        info!(
            original = %local_path,
            conflict_copy = %conflict_name,
            "Keep-both: local renamed, remote downloaded"
        );

        Ok(())
    }

    /// Resolve multiple conflicts with the same strategy
    pub async fn resolve_batch(
        &self,
        conflicts: Vec<(Conflict, SyncItem)>,
        resolution: Resolution,
        source: ResolutionSource,
        sync_root: &SyncPath,
    ) -> BatchResult {
        let mut result = BatchResult {
            resolved: 0,
            failed: 0,
            errors: Vec::new(),
        };

        for (conflict, item) in conflicts {
            match self
                .apply_resolution(conflict, resolution.clone(), source.clone(), &item, sync_root)
                .await
            {
                Ok(_) => result.resolved += 1,
                Err(e) => {
                    warn!(error = %e, "Batch resolution failed for item");
                    result.failed += 1;
                    result.errors.push(e.to_string());
                }
            }
        }

        result
    }
}

fn split_remote_path(path: &str) -> Result<(RemotePath, String), ConflictError> {
    let remote_path = RemotePath::new(path.to_string())
        .map_err(|e| ConflictError::ResolutionFailed(format!("invalid remote path: {e}")))?;

    let file_name = remote_path
        .file_name()
        .ok_or_else(|| {
            ConflictError::ResolutionFailed(format!("remote path has no file name: {}", path))
        })?
        .to_string();

    let parent = remote_path.parent().unwrap_or_else(RemotePath::root);

    Ok((parent, file_name))
}
