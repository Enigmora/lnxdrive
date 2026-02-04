//! GraphCloudProvider - ICloudProvider implementation for Microsoft Graph API
//!
//! Wraps the [`GraphClient`] and delegates to the delta, upload, and client
//! modules to fulfil the [`ICloudProvider`] port contract.
//!
//! ## Design Notes
//!
//! - Uses `tokio::sync::Mutex` because `ICloudProvider` methods take `&self`
//!   while some `GraphClient` methods require `&mut self` (e.g., `set_access_token`).
//! - Authentication (`authenticate`, `refresh_tokens`) is handled separately
//!   by `GraphAuthAdapter`; this provider focuses on file operations.
//! - `get_metadata` and `delete_item` make direct Graph API calls via the
//!   underlying `GraphClient::request()` method.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::debug;

use lnxdrive_core::domain::newtypes::{DeltaToken, RemoteId, RemotePath};
use lnxdrive_core::ports::cloud_provider::{
    AuthFlow, DeltaItem, DeltaResponse, ICloudProvider, Tokens, UserInfo,
};

use crate::client::GraphClient;
use crate::delta;
use crate::upload;

// ============================================================================
// Graph API response type for get_metadata
// ============================================================================

/// Minimal DriveItem response for metadata queries
///
/// Used to parse the response from `GET /me/drive/items/{id}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphMetadataItem {
    /// OneDrive item ID
    id: String,
    /// Item name
    name: String,
    /// File size in bytes
    size: Option<u64>,
    /// Last modified timestamp
    last_modified_date_time: Option<DateTime<Utc>>,
    /// Parent reference
    parent_reference: Option<GraphParentRef>,
    /// File facet (present if item is a file)
    file: Option<GraphFileFacet>,
    /// Folder facet (present if item is a folder)
    folder: Option<serde_json::Value>,
    /// Deleted facet (present if item was deleted)
    deleted: Option<serde_json::Value>,
}

/// Parent reference from metadata response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphParentRef {
    /// Parent item ID
    id: Option<String>,
    /// Parent path (e.g., "/drive/root:/Documents")
    path: Option<String>,
}

/// File facet from metadata response
#[derive(Debug, Deserialize)]
struct GraphFileFacet {
    /// Content hashes
    hashes: Option<GraphHashes>,
}

/// Content hashes
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphHashes {
    /// QuickXorHash in Base64
    quick_xor_hash: Option<String>,
}

/// Converts a [`GraphMetadataItem`] into a port-level [`DeltaItem`]
fn metadata_to_delta_item(item: GraphMetadataItem) -> DeltaItem {
    let is_directory = item.folder.is_some();
    let is_deleted = item.deleted.is_some();

    let hash = item
        .file
        .as_ref()
        .and_then(|f| f.hashes.as_ref())
        .and_then(|h| h.quick_xor_hash.clone());

    let path = item
        .parent_reference
        .as_ref()
        .and_then(|pr| pr.path.as_deref())
        .map(|p| {
            let stripped = if let Some(rest) = p.strip_prefix("/drive/root:") {
                if rest.is_empty() {
                    "/".to_string()
                } else {
                    rest.to_string()
                }
            } else {
                p.to_string()
            };

            if stripped == "/" {
                format!("/{}", item.name)
            } else {
                format!("{}/{}", stripped, item.name)
            }
        });

    let parent_id = item.parent_reference.as_ref().and_then(|pr| pr.id.clone());

    DeltaItem {
        id: item.id,
        name: item.name,
        path,
        size: item.size,
        hash,
        modified: item.last_modified_date_time,
        is_deleted,
        is_directory,
        parent_id,
    }
}

// ============================================================================
// T150: GraphCloudProvider
// ============================================================================

/// Cloud provider implementation that delegates to the Microsoft Graph API
///
/// Wraps a [`GraphClient`] behind a `tokio::sync::Mutex` to allow interior
/// mutability (e.g., updating the access token) while satisfying the `&self`
/// signature of [`ICloudProvider`] methods.
pub struct GraphCloudProvider {
    /// The underlying Graph API client, protected by a mutex
    client: Mutex<GraphClient>,
}

impl GraphCloudProvider {
    /// Creates a new `GraphCloudProvider` wrapping the given [`GraphClient`]
    pub fn new(client: GraphClient) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }
}

#[async_trait::async_trait]
impl ICloudProvider for GraphCloudProvider {
    /// Authentication is handled separately by `GraphAuthAdapter`.
    ///
    /// This method is not implemented on `GraphCloudProvider` because the
    /// OAuth PKCE flow requires browser interaction and a local HTTP callback
    /// server, which are orchestrated by the auth module.
    async fn authenticate(&self, _auth_flow: &AuthFlow) -> Result<Tokens> {
        anyhow::bail!("Use GraphAuthAdapter for authentication")
    }

    /// Token refresh is handled separately by `GraphAuthAdapter`.
    ///
    /// See [`authenticate`](Self::authenticate) for rationale.
    async fn refresh_tokens(&self, _refresh_token: &str) -> Result<Tokens> {
        anyhow::bail!("Use GraphAuthAdapter for token refresh")
    }

    /// Queries for changes since the last delta token
    ///
    /// Delegates to [`delta::get_delta`] which handles pagination automatically.
    async fn get_delta(&self, token: Option<&DeltaToken>) -> Result<DeltaResponse> {
        let client = self.client.lock().await;
        debug!(has_token = token.is_some(), "GraphCloudProvider::get_delta");
        delta::get_delta(&client, token).await
    }

    /// Downloads a file's content by its remote ID
    ///
    /// Delegates to [`GraphClient::download_file`].
    async fn download_file(&self, remote_id: &RemoteId) -> Result<Vec<u8>> {
        let client = self.client.lock().await;
        debug!(id = %remote_id, "GraphCloudProvider::download_file");
        client.download_file(remote_id).await
    }

    /// Uploads a small file (< 4MB) in a single request
    ///
    /// Delegates to [`upload::upload_small`].
    async fn upload_file(
        &self,
        parent_path: &RemotePath,
        name: &str,
        data: &[u8],
    ) -> Result<DeltaItem> {
        let client = self.client.lock().await;
        debug!(
            parent = %parent_path,
            name,
            size = data.len(),
            "GraphCloudProvider::upload_file"
        );
        upload::upload_small(&client, parent_path, name, data).await
    }

    /// Uploads a large file using a resumable upload session
    ///
    /// Delegates to [`upload::upload_large`].
    async fn upload_file_session(
        &self,
        parent_path: &RemotePath,
        name: &str,
        data: &[u8],
        progress: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> Result<DeltaItem> {
        let client = self.client.lock().await;
        debug!(
            parent = %parent_path,
            name,
            size = data.len(),
            "GraphCloudProvider::upload_file_session"
        );
        upload::upload_large(&client, parent_path, name, data, progress).await
    }

    /// Retrieves metadata for a specific item by its remote ID
    ///
    /// Makes `GET /me/drive/items/{id}` and converts the response to a [`DeltaItem`].
    async fn get_metadata(&self, remote_id: &RemoteId) -> Result<DeltaItem> {
        let client = self.client.lock().await;
        let path = format!("/me/drive/items/{}", remote_id.as_str());
        debug!(id = %remote_id, "GraphCloudProvider::get_metadata");

        let item: GraphMetadataItem = client
            .request(Method::GET, &path)
            .send()
            .await
            .context("Failed to send metadata request")?
            .error_for_status()
            .context("Metadata request returned error status")?
            .json()
            .await
            .context("Failed to parse metadata response")?;

        Ok(metadata_to_delta_item(item))
    }

    /// Retrieves information about the authenticated user
    ///
    /// Delegates to [`GraphClient::get_user_info`].
    async fn get_user_info(&self) -> Result<UserInfo> {
        let client = self.client.lock().await;
        debug!("GraphCloudProvider::get_user_info");
        client.get_user_info().await
    }

    /// Deletes an item from OneDrive
    ///
    /// Makes `DELETE /me/drive/items/{id}`. OneDrive moves the item to the
    /// recycle bin by default (soft delete).
    async fn delete_item(&self, remote_id: &RemoteId) -> Result<()> {
        let client = self.client.lock().await;
        let path = format!("/me/drive/items/{}", remote_id.as_str());
        debug!(id = %remote_id, "GraphCloudProvider::delete_item");

        client
            .request(Method::DELETE, &path)
            .send()
            .await
            .context("Failed to send delete request")?
            .error_for_status()
            .context("Delete request returned error status")?;

        debug!(id = %remote_id, "Item deleted successfully");
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_to_delta_item_file() {
        let item = GraphMetadataItem {
            id: "FILE001".to_string(),
            name: "test.txt".to_string(),
            size: Some(1024),
            last_modified_date_time: Some("2025-06-15T10:30:00Z".parse().unwrap()),
            parent_reference: Some(GraphParentRef {
                id: Some("PARENT001".to_string()),
                path: Some("/drive/root:/Documents".to_string()),
            }),
            file: Some(GraphFileFacet {
                hashes: Some(GraphHashes {
                    quick_xor_hash: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()),
                }),
            }),
            folder: None,
            deleted: None,
        };

        let delta = metadata_to_delta_item(item);
        assert_eq!(delta.id, "FILE001");
        assert_eq!(delta.name, "test.txt");
        assert_eq!(delta.path, Some("/Documents/test.txt".to_string()));
        assert_eq!(delta.size, Some(1024));
        assert_eq!(delta.hash, Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()));
        assert!(!delta.is_deleted);
        assert!(!delta.is_directory);
        assert_eq!(delta.parent_id, Some("PARENT001".to_string()));
    }

    #[test]
    fn test_metadata_to_delta_item_folder() {
        let item = GraphMetadataItem {
            id: "FOLDER001".to_string(),
            name: "Photos".to_string(),
            size: Some(0),
            last_modified_date_time: None,
            parent_reference: Some(GraphParentRef {
                id: Some("ROOT".to_string()),
                path: Some("/drive/root:".to_string()),
            }),
            file: None,
            folder: Some(serde_json::json!({"childCount": 5})),
            deleted: None,
        };

        let delta = metadata_to_delta_item(item);
        assert_eq!(delta.id, "FOLDER001");
        assert_eq!(delta.name, "Photos");
        assert_eq!(delta.path, Some("/Photos".to_string()));
        assert!(delta.is_directory);
        assert!(!delta.is_deleted);
        assert!(delta.hash.is_none());
    }

    #[test]
    fn test_metadata_to_delta_item_deleted() {
        let item = GraphMetadataItem {
            id: "DELETED001".to_string(),
            name: "old.txt".to_string(),
            size: None,
            last_modified_date_time: None,
            parent_reference: None,
            file: None,
            folder: None,
            deleted: Some(serde_json::json!({})),
        };

        let delta = metadata_to_delta_item(item);
        assert_eq!(delta.id, "DELETED001");
        assert!(delta.is_deleted);
        assert!(delta.path.is_none());
        assert!(delta.parent_id.is_none());
    }

    #[test]
    fn test_metadata_to_delta_item_root_file() {
        let item = GraphMetadataItem {
            id: "ROOTFILE".to_string(),
            name: "readme.md".to_string(),
            size: Some(512),
            last_modified_date_time: None,
            parent_reference: Some(GraphParentRef {
                id: Some("ROOT".to_string()),
                path: Some("/drive/root:".to_string()),
            }),
            file: Some(GraphFileFacet { hashes: None }),
            folder: None,
            deleted: None,
        };

        let delta = metadata_to_delta_item(item);
        assert_eq!(delta.path, Some("/readme.md".to_string()));
        assert!(delta.hash.is_none());
    }

    #[test]
    fn test_graph_cloud_provider_creation() {
        let client = GraphClient::new("test-token");
        let _provider = GraphCloudProvider::new(client);
        // Just verify it compiles and constructs without panic
    }
}
