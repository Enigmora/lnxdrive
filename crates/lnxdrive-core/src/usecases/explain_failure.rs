//! Failure explanation use case
//!
//! Provides human-readable explanations of why a file failed to sync,
//! including actionable suggestions and audit history. This powers the
//! `lnxdrive explain <path>` CLI command.

use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::{AuditEntry, ItemState, SyncItem, SyncPath};
use crate::ports::IStateRepository;

/// Human-readable explanation of a file's sync state
///
/// Contains a summary message, actionable suggestions, and the
/// relevant audit history for the item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    /// The file path being explained
    pub path: SyncPath,
    /// The current sync state of the item
    pub state: String,
    /// Human-readable explanation of the current state
    pub message: String,
    /// Actionable suggestions for resolving issues
    pub suggestions: Vec<String>,
    /// Recent audit history entries for this item
    pub history: Vec<AuditEntry>,
}

impl Explanation {
    /// Creates a new Explanation for a sync item with its audit history
    fn from_item(item: &SyncItem, history: Vec<AuditEntry>) -> Self {
        let (message, suggestions) = Self::generate_explanation(item);

        Self {
            path: item.local_path().clone(),
            state: item.state().to_string(),
            message,
            suggestions,
            history,
        }
    }

    /// Creates an Explanation for a path that has no tracked sync item
    fn not_found(path: &SyncPath) -> Self {
        Self {
            path: path.clone(),
            state: "unknown".to_string(),
            message: "This file is not being tracked by LNXDrive.".to_string(),
            suggestions: vec![
                "Ensure the file is within the configured sync root directory.".to_string(),
                "Check that the file is not excluded by sync rules or .lnxdriveignore.".to_string(),
                "Run 'lnxdrive status' to verify the sync root configuration.".to_string(),
            ],
            history: Vec::new(),
        }
    }

    /// Generates a human-readable message and suggestions based on item state
    fn generate_explanation(item: &SyncItem) -> (String, Vec<String>) {
        match item.state() {
            ItemState::Online => (
                "This file exists only in the cloud. Its content has not been downloaded yet."
                    .to_string(),
                vec![
                    "Open the file to trigger automatic download (hydration).".to_string(),
                    "Use 'lnxdrive pin <path>' to force download.".to_string(),
                ],
            ),

            ItemState::Hydrating => (
                "This file is currently being downloaded from the cloud.".to_string(),
                vec![
                    "Wait for the download to complete.".to_string(),
                    "Check your network connection if the download seems stuck.".to_string(),
                ],
            ),

            ItemState::Hydrated => (
                "This file is fully synced. Local and cloud copies match.".to_string(),
                vec![],
            ),

            ItemState::Modified => (
                "This file has local changes that have not been uploaded yet.".to_string(),
                vec![
                    "Changes will be uploaded during the next sync cycle.".to_string(),
                    "Use 'lnxdrive sync' to trigger an immediate sync.".to_string(),
                ],
            ),

            ItemState::Conflicted => (
                "This file has conflicting changes in both local and cloud versions.".to_string(),
                vec![
                    "Use 'lnxdrive resolve <path> --keep-local' to keep your version.".to_string(),
                    "Use 'lnxdrive resolve <path> --keep-remote' to use the cloud version."
                        .to_string(),
                    "Use 'lnxdrive resolve <path> --keep-both' to keep both versions.".to_string(),
                ],
            ),

            ItemState::Error(reason) => {
                let message = format!(
                    "This file encountered an error during synchronization: {}",
                    reason
                );
                let mut suggestions = vec!["Check 'lnxdrive status' for more details.".to_string()];

                // Add context-specific suggestions based on error info
                if let Some(error_info) = item.error_info() {
                    match error_info.code() {
                        "NETWORK_ERROR" => {
                            suggestions
                                .push("Check your network connection and try again.".to_string());
                        }
                        "AUTH_ERROR" => {
                            suggestions.push("Re-authenticate with 'lnxdrive login'.".to_string());
                        }
                        "RATE_LIMITED" => {
                            suggestions.push(
                                "The cloud provider is rate-limiting requests. Wait a moment and retry."
                                    .to_string(),
                            );
                        }
                        _ => {
                            suggestions.push(
                                "Try 'lnxdrive sync --force' to retry the operation.".to_string(),
                            );
                        }
                    }

                    if error_info.retry_count() > 0 {
                        suggestions.push(format!(
                            "This operation has been retried {} time(s) already.",
                            error_info.retry_count()
                        ));
                    }
                }

                (message, suggestions)
            }

            ItemState::Deleted => (
                "This file has been marked for deletion.".to_string(),
                vec![
                    "The deletion will be synced during the next sync cycle.".to_string(),
                    "If this was unintentional, check the OneDrive recycle bin.".to_string(),
                ],
            ),
        }
    }
}

/// Use case for generating human-readable failure explanations
///
/// Provides the `lnxdrive explain` functionality by combining sync item
/// state with audit history to produce actionable explanations.
pub struct ExplainFailureUseCase {
    state_repository: Arc<dyn IStateRepository + Send + Sync>,
}

impl ExplainFailureUseCase {
    /// Creates a new ExplainFailureUseCase with the required dependencies
    ///
    /// # Arguments
    ///
    /// * `state_repository` - Persistent storage for querying item state and audit log
    pub fn new(state_repository: Arc<dyn IStateRepository + Send + Sync>) -> Self {
        Self { state_repository }
    }

    /// Generates a human-readable explanation for a file path
    ///
    /// This method:
    /// 1. Looks up the sync item by its local path
    /// 2. Retrieves the audit history for the item
    /// 3. Generates a human-readable message with suggestions
    ///
    /// # Arguments
    ///
    /// * `path` - The local sync path to explain
    ///
    /// # Returns
    ///
    /// An Explanation struct with state, message, suggestions, and history
    ///
    /// # Errors
    ///
    /// Returns an error if the repository query fails
    pub async fn explain(&self, path: &SyncPath) -> Result<Explanation> {
        // Step 1: Look up the sync item by path
        let item = self
            .state_repository
            .get_item_by_path(path)
            .await
            .context("Failed to look up sync item by path")?;

        let Some(item) = item else {
            return Ok(Explanation::not_found(path));
        };

        // Step 2: Get audit history for this item
        let history = self
            .state_repository
            .get_audit_trail(item.id())
            .await
            .context("Failed to retrieve audit history for item")?;

        // Step 3: Generate the explanation
        Ok(Explanation::from_item(&item, history))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::domain::{ErrorInfo, FileHash, RemotePath, SyncItem};

    fn test_path() -> SyncPath {
        SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap()
    }

    fn test_remote_path() -> RemotePath {
        RemotePath::new("/test.txt".to_string()).unwrap()
    }

    fn create_item_in_state(state: ItemState) -> SyncItem {
        let mut item = SyncItem::new_file(test_path(), test_remote_path(), 1024, None).unwrap();

        // Walk through valid state transitions to reach the target state
        match state {
            ItemState::Online => {} // Already the default state
            ItemState::Hydrating => {
                item.start_hydrating().unwrap();
            }
            ItemState::Hydrated => {
                item.start_hydrating().unwrap();
                item.complete_hydration().unwrap();
            }
            ItemState::Modified => {
                item.start_hydrating().unwrap();
                item.complete_hydration().unwrap();
                item.mark_modified().unwrap();
            }
            ItemState::Conflicted => {
                item.start_hydrating().unwrap();
                item.complete_hydration().unwrap();
                item.mark_modified().unwrap();
                item.mark_conflicted().unwrap();
            }
            ItemState::Error(ref reason) => {
                item.transition_to_error(ErrorInfo::new("TEST", reason.clone()))
                    .unwrap();
            }
            ItemState::Deleted => {
                item.mark_deleted().unwrap();
            }
        }

        item
    }

    #[test]
    fn test_explanation_online() {
        let item = create_item_in_state(ItemState::Online);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("cloud"));
        assert!(!explanation.suggestions.is_empty());
    }

    #[test]
    fn test_explanation_hydrating() {
        let item = create_item_in_state(ItemState::Hydrating);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("downloaded"));
    }

    #[test]
    fn test_explanation_hydrated() {
        let item = create_item_in_state(ItemState::Hydrated);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("synced"));
        assert!(explanation.suggestions.is_empty());
    }

    #[test]
    fn test_explanation_modified() {
        let item = create_item_in_state(ItemState::Modified);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("local changes"));
    }

    #[test]
    fn test_explanation_conflicted() {
        let item = create_item_in_state(ItemState::Conflicted);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("conflicting"));
        assert!(explanation.suggestions.len() >= 3);
    }

    #[test]
    fn test_explanation_error_network() {
        let mut item = create_item_in_state(ItemState::Online);
        item.transition_to_error(ErrorInfo::network_error("Connection failed"))
            .unwrap();

        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("error"));
        assert!(explanation
            .suggestions
            .iter()
            .any(|s| s.contains("network")));
    }

    #[test]
    fn test_explanation_error_auth() {
        let mut item = create_item_in_state(ItemState::Online);
        item.transition_to_error(ErrorInfo::auth_error("Token expired"))
            .unwrap();

        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.suggestions.iter().any(|s| s.contains("login")));
    }

    #[test]
    fn test_explanation_deleted() {
        let item = create_item_in_state(ItemState::Deleted);
        let explanation = Explanation::from_item(&item, vec![]);

        assert!(explanation.message.contains("deletion"));
    }

    #[test]
    fn test_explanation_not_found() {
        let path = test_path();
        let explanation = Explanation::not_found(&path);

        assert_eq!(explanation.state, "unknown");
        assert!(explanation.message.contains("not being tracked"));
        assert!(explanation.history.is_empty());
    }
}
