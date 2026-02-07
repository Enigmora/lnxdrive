//! Conflict detection logic
//!
//! Determines whether a remote change conflicts with local modifications
//! by comparing content hashes and states.

use lnxdrive_core::domain::{
    conflict::{Conflict, VersionInfo},
    newtypes::FileHash,
    sync_item::{ItemState, SyncItem},
};
use tracing::{debug, info};

use crate::policy::PolicyEngine;

/// Result of conflict detection check
#[derive(Debug, Clone)]
pub enum DetectionResult {
    /// No conflict: safe to apply the remote change
    NoConflict,
    /// Conflict detected: both versions changed
    Conflicted(Box<Conflict>),
}

/// Detects conflicts between local and remote file versions
pub struct ConflictDetector;

impl ConflictDetector {
    /// Checks if a remote update conflicts with the local state
    ///
    /// A conflict exists when:
    /// 1. The item is in `Modified` state (local changes pending), AND
    /// 2. The remote content hash differs from the stored content hash
    ///    (remote also changed)
    ///
    /// Returns `DetectionResult::Conflicted` with a new `Conflict` entity
    /// if both sides changed, or `DetectionResult::NoConflict` otherwise.
    pub fn check_remote_update(
        existing: &SyncItem,
        remote_hash: Option<&str>,
        remote_size: Option<u64>,
        remote_modified: Option<chrono::DateTime<chrono::Utc>>,
        remote_etag: Option<&str>,
    ) -> DetectionResult {
        // Only check for conflicts if the item has local modifications
        if !matches!(existing.state(), ItemState::Modified) {
            return DetectionResult::NoConflict;
        }

        // If we can't determine the remote hash, we can't detect conflicts
        let Some(remote_hash_str) = remote_hash else {
            return DetectionResult::NoConflict;
        };

        // Compare stored content hash with the remote hash
        let stored_hash = existing.content_hash().map(|h| h.as_str());
        let remote_changed = match stored_hash {
            Some(stored) => stored != remote_hash_str,
            None => true, // No stored hash, assume changed
        };

        if !remote_changed {
            debug!(
                path = %existing.local_path(),
                "Remote hash matches stored hash, no conflict"
            );
            return DetectionResult::NoConflict;
        }

        // Both local (Modified state) and remote (hash changed) have changes
        info!(
            path = %existing.local_path(),
            stored_hash = ?stored_hash,
            remote_hash = %remote_hash_str,
            "Conflict detected: both local and remote versions changed"
        );

        // Build VersionInfo for both sides
        let local_version = build_local_version(existing);
        let remote_version = build_remote_version(
            remote_hash_str,
            remote_size.unwrap_or(0),
            remote_modified.unwrap_or_else(chrono::Utc::now),
            remote_etag,
        );

        let conflict = Conflict::new(*existing.id(), local_version, remote_version);
        DetectionResult::Conflicted(Box::new(conflict))
    }

    /// Checks if a local update conflicts with a known remote change
    ///
    /// This is the reverse direction: before uploading a local change,
    /// verify the remote hasn't also changed (e.g., via a concurrent delta).
    pub fn check_local_update(
        existing: &SyncItem,
        current_remote_hash: Option<&str>,
    ) -> bool {
        let stored_hash = existing.content_hash().map(|h| h.as_str());

        match (stored_hash, current_remote_hash) {
            (Some(stored), Some(remote)) => {
                if stored != remote {
                    info!(
                        path = %existing.local_path(),
                        "Remote changed since last sync, potential conflict on local upload"
                    );
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Determines whether a conflict should be auto-resolved via policy
    ///
    /// Returns `Some(Resolution)` if the policy engine has a non-Manual
    /// resolution for this file path.
    pub fn should_auto_resolve(
        policy: &PolicyEngine,
        relative_path: &str,
    ) -> Option<lnxdrive_core::domain::conflict::Resolution> {
        let resolution = policy.evaluate(relative_path);
        if matches!(resolution, lnxdrive_core::domain::conflict::Resolution::Manual) {
            None
        } else {
            Some(resolution)
        }
    }
}

fn build_local_version(item: &SyncItem) -> VersionInfo {
    let hash = item
        .local_hash()
        .cloned()
        .or_else(|| item.content_hash().cloned())
        .unwrap_or_else(|| FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap());

    let modified = item.last_modified_local().unwrap_or_else(chrono::Utc::now);

    VersionInfo::new(hash, item.size_bytes(), modified)
}

fn build_remote_version(
    hash_str: &str,
    size: u64,
    modified: chrono::DateTime<chrono::Utc>,
    etag: Option<&str>,
) -> VersionInfo {
    let hash = FileHash::new(hash_str.to_string())
        .unwrap_or_else(|_| FileHash::new("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()).unwrap());

    let mut version = VersionInfo::new(hash, size, modified);
    if let Some(etag) = etag {
        version = version.with_etag(etag);
    }
    version
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnxdrive_core::domain::newtypes::{RemotePath, SyncPath};

    fn create_test_item(state: ItemState, content_hash: Option<&str>) -> SyncItem {
        let sync_path = SyncPath::new(std::path::PathBuf::from("/home/user/OneDrive/test.txt"))
            .expect("valid sync path");
        let remote_path =
            RemotePath::new("/test.txt".to_string()).expect("valid remote path");

        let mut item = SyncItem::new(sync_path, remote_path, false).expect("valid sync item");
        // Transition to the target state
        if state != ItemState::Online {
            // Need to go through valid transitions
            item.transition_to(ItemState::Hydrating).ok();
            item.transition_to(ItemState::Hydrated).ok();
            if matches!(state, ItemState::Modified | ItemState::Conflicted) {
                item.transition_to(ItemState::Modified).ok();
            }
            if matches!(state, ItemState::Conflicted) {
                item.transition_to(ItemState::Conflicted).ok();
            }
        }

        if let Some(hash_str) = content_hash {
            if let Ok(hash) = FileHash::new(hash_str.to_string()) {
                item.set_content_hash(hash);
            }
        }

        item
    }

    const HASH_A: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const HASH_B: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBB=";

    #[test]
    fn test_no_conflict_when_not_modified() {
        let item = create_test_item(ItemState::Hydrated, Some(HASH_A));

        let result =
            ConflictDetector::check_remote_update(&item, Some(HASH_B), Some(1024), None, None);

        assert!(matches!(result, DetectionResult::NoConflict));
    }

    #[test]
    fn test_no_conflict_when_remote_hash_matches() {
        let item = create_test_item(ItemState::Modified, Some(HASH_A));

        let result =
            ConflictDetector::check_remote_update(&item, Some(HASH_A), Some(1024), None, None);

        assert!(matches!(result, DetectionResult::NoConflict));
    }

    #[test]
    fn test_conflict_when_both_changed() {
        let item = create_test_item(ItemState::Modified, Some(HASH_A));

        let result =
            ConflictDetector::check_remote_update(&item, Some(HASH_B), Some(2048), None, None);

        match result {
            DetectionResult::Conflicted(conflict) => {
                assert_eq!(conflict.item_id(), item.id());
                assert!(!conflict.is_resolved());
            }
            DetectionResult::NoConflict => panic!("Expected conflict"),
        }
    }

    #[test]
    fn test_no_conflict_when_no_remote_hash() {
        let item = create_test_item(ItemState::Modified, Some(HASH_A));

        let result = ConflictDetector::check_remote_update(&item, None, Some(1024), None, None);

        assert!(matches!(result, DetectionResult::NoConflict));
    }

    #[test]
    fn test_check_local_update_no_conflict() {
        let item = create_test_item(ItemState::Modified, Some(HASH_A));

        assert!(!ConflictDetector::check_local_update(&item, Some(HASH_A)));
    }

    #[test]
    fn test_check_local_update_conflict() {
        let item = create_test_item(ItemState::Modified, Some(HASH_A));

        assert!(ConflictDetector::check_local_update(&item, Some(HASH_B)));
    }

    #[test]
    fn test_should_auto_resolve_manual() {
        let policy = PolicyEngine::new("manual", &[]);

        assert!(ConflictDetector::should_auto_resolve(&policy, "test.txt").is_none());
    }

    #[test]
    fn test_should_auto_resolve_with_policy() {
        use crate::policy::ConflictRule;
        use lnxdrive_core::domain::conflict::Resolution;

        let rules = vec![ConflictRule {
            pattern: "**/*.docx".to_string(),
            strategy: "keep_both".to_string(),
        }];
        let policy = PolicyEngine::new("manual", &rules);

        let result = ConflictDetector::should_auto_resolve(&policy, "report.docx");
        assert_eq!(result, Some(Resolution::KeepBoth));

        let result = ConflictDetector::should_auto_resolve(&policy, "report.txt");
        assert!(result.is_none());
    }
}
