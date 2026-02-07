//! Conflict use cases - orchestrate detection and resolution
//!
//! These use cases integrate the conflict detector, policy engine, and
//! resolver into coherent workflows used by the sync engine.

use std::sync::Arc;

use tracing::{debug, info};

use lnxdrive_core::{
    domain::{
        conflict::{Conflict, Resolution, ResolutionSource},
        newtypes::SyncPath,
        sync_item::SyncItem,
    },
    ports::state_repository::IStateRepository,
};

use crate::{
    detector::{ConflictDetector, DetectionResult},
    error::ConflictError,
    policy::PolicyEngine,
    resolver::ConflictResolver,
};

/// Orchestrates conflict detection + policy evaluation + auto-resolution
pub struct DetectConflictUseCase {
    policy_engine: PolicyEngine,
    state_repository: Arc<dyn IStateRepository>,
    resolver: Option<Arc<ConflictResolver>>,
    sync_root: SyncPath,
}

impl DetectConflictUseCase {
    pub fn new(
        policy_engine: PolicyEngine,
        state_repository: Arc<dyn IStateRepository>,
        resolver: Option<Arc<ConflictResolver>>,
        sync_root: SyncPath,
    ) -> Self {
        Self {
            policy_engine,
            state_repository,
            resolver,
            sync_root,
        }
    }

    /// Check a remote update for conflicts and handle accordingly
    ///
    /// Returns `Some(Conflict)` if a conflict was detected and NOT auto-resolved,
    /// meaning it needs user intervention. Returns `None` if no conflict or
    /// if the conflict was auto-resolved by policy.
    pub async fn check_and_handle(
        &self,
        existing: &SyncItem,
        remote_hash: Option<&str>,
        remote_size: Option<u64>,
        remote_modified: Option<chrono::DateTime<chrono::Utc>>,
        remote_etag: Option<&str>,
    ) -> Result<Option<Conflict>, ConflictError> {
        let result = ConflictDetector::check_remote_update(
            existing,
            remote_hash,
            remote_size,
            remote_modified,
            remote_etag,
        );

        let conflict = match result {
            DetectionResult::NoConflict => return Ok(None),
            DetectionResult::Conflicted(c) => *c,
        };

        // Save the conflict
        self.state_repository
            .save_conflict(&conflict)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("save conflict: {e}")))?;

        // Mark the item as conflicted
        let mut updated_item = existing.clone();
        updated_item
            .mark_conflicted()
            .map_err(|e| ConflictError::ResolutionFailed(format!("mark conflicted: {e}")))?;
        self.state_repository
            .save_item(&updated_item)
            .await
            .map_err(|e| ConflictError::ResolutionFailed(format!("save item: {e}")))?;

        // Check if policy auto-resolves this
        let relative_path = existing
            .local_path()
            .relative_to(&self.sync_root)
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        if let Some(auto_resolution) =
            ConflictDetector::should_auto_resolve(&self.policy_engine, &relative_path)
        {
            info!(
                path = %existing.local_path(),
                resolution = %auto_resolution,
                "Auto-resolving conflict via policy"
            );

            if let Some(ref resolver) = self.resolver {
                match resolver
                    .apply_resolution(
                        conflict.clone(),
                        auto_resolution,
                        ResolutionSource::Policy,
                        &updated_item,
                        &self.sync_root,
                    )
                    .await
                {
                    Ok(_resolved) => {
                        debug!("Conflict auto-resolved by policy");
                        return Ok(None);
                    }
                    Err(e) => {
                        // Auto-resolution failed, fall through to manual
                        tracing::warn!(error = %e, "Auto-resolution failed, leaving as unresolved");
                    }
                }
            }
        }

        // Conflict needs manual resolution
        Ok(Some(conflict))
    }
}

/// Orchestrates manual conflict resolution
pub struct ResolveConflictUseCase {
    resolver: Arc<ConflictResolver>,
    sync_root: SyncPath,
}

impl ResolveConflictUseCase {
    pub fn new(
        resolver: Arc<ConflictResolver>,
        sync_root: SyncPath,
    ) -> Self {
        Self {
            resolver,
            sync_root,
        }
    }

    /// Resolve a conflict by its ID
    pub async fn resolve(
        &self,
        conflict: Conflict,
        resolution: Resolution,
        item: &SyncItem,
    ) -> Result<Conflict, ConflictError> {
        self.resolver
            .apply_resolution(
                conflict,
                resolution,
                ResolutionSource::User,
                item,
                &self.sync_root,
            )
            .await
    }
}
