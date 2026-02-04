//! Microsoft Graph Delta API for incremental synchronization
//!
//! Implements the delta query pattern for OneDrive, which provides efficient
//! incremental sync by returning only items that have changed since the last query.
//!
//! ## Delta Query Flow
//!
//! 1. **Initial sync**: Call [`get_delta`] with `token = None` to get all items
//! 2. **Follow pages**: The function automatically follows `@odata.nextLink` pages
//! 3. **Save token**: The returned [`DeltaResponse`] contains a `delta_link` with
//!    a token for the next sync
//! 4. **Incremental sync**: Call [`get_delta`] with the saved token to get only changes
//!
//! ## Usage
//!
//! ```rust,no_run
//! use lnxdrive_graph::client::GraphClient;
//! use lnxdrive_graph::delta;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = GraphClient::new("access-token");
//!
//! // Initial sync: get all items
//! let response = delta::get_delta(&client, None).await?;
//! println!("Got {} items", response.items.len());
//!
//! // Save delta_link for next sync...
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Method};
use serde::Deserialize;
use tracing::{debug, warn};

use lnxdrive_core::domain::newtypes::DeltaToken;
use lnxdrive_core::ports::cloud_provider::{DeltaItem, DeltaResponse};

use crate::client::GraphClient;

/// Path for the delta endpoint relative to the Graph API base URL
const DELTA_PATH: &str = "/me/drive/root/delta";

// ============================================================================
// Microsoft Graph API response types (JSON deserialization)
// ============================================================================

/// Raw response from the Microsoft Graph delta API
///
/// Represents the JSON structure returned by:
/// `GET /me/drive/root/delta`
///
/// See: <https://learn.microsoft.com/en-us/graph/api/driveitem-delta>
#[derive(Debug, Deserialize)]
struct GraphDeltaResponse {
    /// Array of changed drive items
    #[serde(default)]
    value: Vec<GraphDriveItem>,

    /// URL for the next page of results (present when more pages exist)
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,

    /// URL containing the delta token for the next sync cycle
    /// (present only on the last page of results)
    #[serde(rename = "@odata.deltaLink")]
    delta_link: Option<String>,
}

/// A drive item from the Microsoft Graph delta response
///
/// Maps to the DriveItem resource type in the Graph API.
/// Fields use camelCase to match the JSON format.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDriveItem {
    /// Unique identifier of the item within the drive
    id: String,

    /// Name of the item (filename or folder name)
    #[serde(default)]
    name: String,

    /// Size of the item in bytes (only for files)
    size: Option<u64>,

    /// Last modified date and time in ISO 8601 format
    last_modified_date_time: Option<DateTime<Utc>>,

    /// Reference to the parent item
    parent_reference: Option<GraphParentReference>,

    /// File facet (present if the item is a file)
    file: Option<GraphFileFacet>,

    /// Folder facet (present if the item is a folder)
    folder: Option<GraphFolderFacet>,

    /// Deleted facet (present if the item has been deleted)
    deleted: Option<GraphDeletedFacet>,
}

/// Parent reference information for a drive item
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphParentReference {
    /// Unique identifier of the parent item
    id: Option<String>,

    /// URL-decoded path of the parent in the drive
    /// Format: `/drive/root:/path/to/parent`
    path: Option<String>,
}

/// File facet indicating the item is a file
///
/// Contains file-specific metadata like hashes.
#[derive(Debug, Deserialize)]
struct GraphFileFacet {
    /// Content hashes for integrity verification
    hashes: Option<GraphHashes>,
}

/// Hash values for a file
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphHashes {
    /// QuickXorHash of the file content (Base64-encoded)
    quick_xor_hash: Option<String>,
}

/// Folder facet indicating the item is a folder
///
/// The mere presence of this facet indicates the item is a folder.
/// The child_count field provides the number of immediate children.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphFolderFacet {
    /// Number of immediate children in the folder
    #[allow(dead_code)]
    child_count: Option<u64>,
}

/// Deleted facet indicating the item has been deleted
///
/// The mere presence of this facet indicates deletion.
/// The state field may provide additional information.
#[derive(Debug, Deserialize)]
struct GraphDeletedFacet {
    /// Reason or state of deletion (often absent)
    #[allow(dead_code)]
    state: Option<String>,
}

// ============================================================================
// DeltaParser - converts Graph API responses to port-level types
// ============================================================================

/// Parser for converting Microsoft Graph delta responses into port-level types
///
/// Transforms the raw JSON-deserialized Graph API structs into the
/// [`DeltaItem`] and [`DeltaResponse`] types defined in `lnxdrive-core`.
pub struct DeltaParser;

impl DeltaParser {
    /// Parse a single Graph API drive item into a port-level [`DeltaItem`]
    ///
    /// Extracts and normalizes fields from the Graph API format:
    /// - Determines if the item is a directory based on the `folder` facet
    /// - Determines if the item is deleted based on the `deleted` facet
    /// - Extracts the quickXorHash from the file facet
    /// - Strips the `/drive/root:` prefix from the parent path
    fn parse_item(item: GraphDriveItem) -> DeltaItem {
        let is_deleted = item.deleted.is_some();
        let is_directory = item.folder.is_some();

        // Extract the quickXorHash from the file facet
        let hash = item
            .file
            .as_ref()
            .and_then(|f| f.hashes.as_ref())
            .and_then(|h| h.quick_xor_hash.clone());

        // Extract and normalize the parent path
        // Graph API returns paths like "/drive/root:/Documents/Subfolder"
        // We strip the "/drive/root:" prefix to get "/Documents/Subfolder"
        let path = item
            .parent_reference
            .as_ref()
            .and_then(|pr| pr.path.as_ref())
            .map(|p| Self::normalize_parent_path(p, &item.name));

        // Extract parent ID
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

    /// Normalize a parent path from Graph API format to a clean path
    ///
    /// The Graph API returns parent paths like `/drive/root:/Documents/Subfolder`.
    /// This method strips the `/drive/root:` prefix and appends the item name
    /// to produce a full path like `/Documents/Subfolder/filename.txt`.
    ///
    /// If the parent path is exactly `/drive/root:` (root folder), the result
    /// is `/<item_name>`.
    fn normalize_parent_path(parent_path: &str, item_name: &str) -> String {
        let stripped = if let Some(rest) = parent_path.strip_prefix("/drive/root:") {
            if rest.is_empty() {
                "/".to_string()
            } else {
                rest.to_string()
            }
        } else {
            // Fallback: use the path as-is
            parent_path.to_string()
        };

        if stripped == "/" {
            format!("/{item_name}")
        } else {
            format!("{stripped}/{item_name}")
        }
    }

    /// Parse a complete Graph API delta response into a port-level [`DeltaResponse`]
    ///
    /// Converts all items and preserves the pagination links.
    fn parse_response(response: GraphDeltaResponse) -> DeltaResponse {
        let items = response.value.into_iter().map(Self::parse_item).collect();

        DeltaResponse {
            items,
            next_link: response.next_link,
            delta_link: response.delta_link,
        }
    }

    /// Extract the delta token value from a delta link URL
    ///
    /// The delta link is a full URL like:
    /// `https://graph.microsoft.com/v1.0/me/drive/root/delta?token=...`
    ///
    /// This extracts just the token parameter value.
    pub fn extract_delta_token(delta_link: &str) -> Option<String> {
        url::Url::parse(delta_link).ok().and_then(|u| {
            u.query_pairs()
                .find(|(key, _)| key == "token")
                .map(|(_, value)| value.into_owned())
        })
    }
}

// ============================================================================
// Delta query functions
// ============================================================================

/// Fetches all delta changes from OneDrive, automatically following pagination
///
/// Makes the initial delta request and follows all `@odata.nextLink` pages
/// until the final page with `@odata.deltaLink` is reached.
///
/// # Arguments
///
/// * `client` - A reference to the authenticated [`GraphClient`]
/// * `token` - Optional delta token from a previous sync. Pass `None` for initial sync.
///
/// # Returns
///
/// A [`DeltaResponse`] containing all changed items across all pages,
/// with `delta_link` set to the token URL for the next sync cycle.
///
/// # Errors
///
/// Returns an error if:
/// - The HTTP request fails
/// - The API returns a non-success status
/// - The response cannot be parsed as JSON
pub async fn get_delta(client: &GraphClient, token: Option<&DeltaToken>) -> Result<DeltaResponse> {
    // Build the initial request URL
    let path = match token {
        Some(t) => format!("{}?token={}", DELTA_PATH, t.as_str()),
        None => DELTA_PATH.to_string(),
    };

    debug!(has_token = token.is_some(), "Starting delta query");

    // Make the initial request using GraphClient's request() method
    let http_response = client
        .request(Method::GET, &path)
        .send()
        .await
        .context("Failed to send delta request")?;

    // T169: Check for 410 Gone before calling error_for_status().
    // A 410 means the delta token has expired and the client must
    // perform a full resync by re-querying without a token.
    if http_response.status() == reqwest::StatusCode::GONE {
        anyhow::bail!("Delta token expired (410 Gone)");
    }

    let raw_response: GraphDeltaResponse = http_response
        .error_for_status()
        .context("Delta request returned error status")?
        .json()
        .await
        .context("Failed to parse delta response JSON")?;

    let mut response = DeltaParser::parse_response(raw_response);

    debug!(
        items = response.items.len(),
        has_next = response.next_link.is_some(),
        "Received initial delta page"
    );

    // Follow pagination via nextLink
    let mut page_count: u32 = 1;
    while let Some(next_link) = response.next_link.take() {
        page_count += 1;
        debug!(page = page_count, "Following delta nextLink");

        let page = get_delta_page(client, &next_link).await?;

        debug!(
            page = page_count,
            items = page.items.len(),
            has_next = page.next_link.is_some(),
            "Received delta page"
        );

        // Accumulate items from this page
        response.items.extend(page.items);
        response.next_link = page.next_link;
        response.delta_link = page.delta_link;
    }

    debug!(
        total_items = response.items.len(),
        total_pages = page_count,
        has_delta_link = response.delta_link.is_some(),
        "Delta query complete"
    );

    if response.delta_link.is_none() {
        warn!("Delta query completed without a deltaLink; next sync may require full re-scan");
    }

    Ok(response)
}

/// Fetches a single page of delta results from a nextLink URL
///
/// The `@odata.nextLink` URL from the Graph API is an absolute URL,
/// so this function makes a direct HTTP request rather than using
/// the `GraphClient::request()` method (which prepends the base URL).
///
/// # Arguments
///
/// * `client` - A reference to the authenticated [`GraphClient`] (used for the access token)
/// * `next_link` - The absolute URL from `@odata.nextLink` in a previous response
///
/// # Returns
///
/// A [`DeltaResponse`] for this single page, which may contain its own
/// `next_link` (if more pages follow) or `delta_link` (if this is the last page).
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be parsed.
pub async fn get_delta_page(client: &GraphClient, next_link: &str) -> Result<DeltaResponse> {
    // nextLink is an absolute URL, so we cannot use client.request()
    // which prepends the base URL. Instead, create a direct request
    // with Bearer auth using the client's access token.
    let http_client = Client::new();

    let raw_response: GraphDeltaResponse = http_client
        .get(next_link)
        .bearer_auth(client.access_token())
        .send()
        .await
        .context("Failed to send delta page request")?
        .error_for_status()
        .context("Delta page request returned error status")?
        .json()
        .await
        .context("Failed to parse delta page response JSON")?;

    Ok(DeltaParser::parse_response(raw_response))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // JSON deserialization tests
    // ========================================================================

    #[test]
    fn test_deserialize_delta_response_with_items() {
        let json = r#"{
            "value": [
                {
                    "id": "item-001",
                    "name": "document.docx",
                    "size": 12345,
                    "lastModifiedDateTime": "2025-06-15T10:30:00Z",
                    "parentReference": {
                        "id": "parent-001",
                        "path": "/drive/root:/Documents"
                    },
                    "file": {
                        "hashes": {
                            "quickXorHash": "AAAAAAAAAAAAAAAAAAAAAAAAAAA="
                        }
                    }
                }
            ],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=abc123"
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.value.len(), 1);
        assert!(response.next_link.is_none());
        assert!(response.delta_link.is_some());

        let item = &response.value[0];
        assert_eq!(item.id, "item-001");
        assert_eq!(item.name, "document.docx");
        assert_eq!(item.size, Some(12345));
        assert!(item.file.is_some());
        assert!(item.folder.is_none());
        assert!(item.deleted.is_none());
    }

    #[test]
    fn test_deserialize_folder_item() {
        let json = r#"{
            "value": [
                {
                    "id": "folder-001",
                    "name": "Documents",
                    "size": 0,
                    "lastModifiedDateTime": "2025-06-15T08:00:00Z",
                    "parentReference": {
                        "id": "root-id",
                        "path": "/drive/root:"
                    },
                    "folder": {
                        "childCount": 5
                    }
                }
            ],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=xyz"
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.value.len(), 1);

        let item = &response.value[0];
        assert_eq!(item.id, "folder-001");
        assert_eq!(item.name, "Documents");
        assert!(item.folder.is_some());
        assert!(item.file.is_none());
        assert_eq!(item.folder.as_ref().unwrap().child_count, Some(5));
    }

    #[test]
    fn test_deserialize_deleted_item() {
        let json = r#"{
            "value": [
                {
                    "id": "deleted-001",
                    "name": "old-file.txt",
                    "deleted": {
                        "state": "deleted"
                    }
                }
            ],
            "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=page2"
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.value.len(), 1);
        assert!(response.next_link.is_some());
        assert!(response.delta_link.is_none());

        let item = &response.value[0];
        assert_eq!(item.id, "deleted-001");
        assert!(item.deleted.is_some());
        assert!(item.size.is_none());
        assert!(item.last_modified_date_time.is_none());
    }

    #[test]
    fn test_deserialize_empty_response() {
        let json = r#"{
            "value": [],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=empty"
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.value.len(), 0);
        assert!(response.delta_link.is_some());
    }

    #[test]
    fn test_deserialize_minimal_item() {
        // Items can have very few fields, especially deleted items
        let json = r#"{
            "value": [
                {
                    "id": "min-001",
                    "name": ""
                }
            ]
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.value.len(), 1);
        assert_eq!(response.value[0].id, "min-001");
        assert!(response.next_link.is_none());
        assert!(response.delta_link.is_none());
    }

    #[test]
    fn test_deserialize_item_without_hashes() {
        let json = r#"{
            "value": [
                {
                    "id": "nohash-001",
                    "name": "file.bin",
                    "size": 999,
                    "file": {}
                }
            ]
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        let item = &response.value[0];
        assert!(item.file.is_some());
        assert!(item.file.as_ref().unwrap().hashes.is_none());
    }

    #[test]
    fn test_deserialize_next_link_pagination() {
        let json = r#"{
            "value": [
                {
                    "id": "page1-001",
                    "name": "file1.txt"
                }
            ],
            "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?$skiptoken=abc"
        }"#;

        let response: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        assert!(response.next_link.is_some());
        assert!(response.delta_link.is_none());
        assert!(response.next_link.unwrap().contains("$skiptoken=abc"));
    }

    // ========================================================================
    // DeltaParser tests
    // ========================================================================

    #[test]
    fn test_parse_file_item() {
        let graph_item = GraphDriveItem {
            id: "file-001".to_string(),
            name: "report.pdf".to_string(),
            size: Some(524288),
            last_modified_date_time: Some("2025-07-01T14:00:00Z".parse().unwrap()),
            parent_reference: Some(GraphParentReference {
                id: Some("parent-001".to_string()),
                path: Some("/drive/root:/Documents/Reports".to_string()),
            }),
            file: Some(GraphFileFacet {
                hashes: Some(GraphHashes {
                    quick_xor_hash: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()),
                }),
            }),
            folder: None,
            deleted: None,
        };

        let item = DeltaParser::parse_item(graph_item);

        assert_eq!(item.id, "file-001");
        assert_eq!(item.name, "report.pdf");
        assert_eq!(item.path, Some("/Documents/Reports/report.pdf".to_string()));
        assert_eq!(item.size, Some(524288));
        assert_eq!(item.hash, Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()));
        assert!(item.modified.is_some());
        assert!(!item.is_deleted);
        assert!(!item.is_directory);
        assert_eq!(item.parent_id, Some("parent-001".to_string()));
    }

    #[test]
    fn test_parse_folder_item() {
        let graph_item = GraphDriveItem {
            id: "folder-001".to_string(),
            name: "Photos".to_string(),
            size: Some(0),
            last_modified_date_time: Some("2025-06-20T09:00:00Z".parse().unwrap()),
            parent_reference: Some(GraphParentReference {
                id: Some("root-id".to_string()),
                path: Some("/drive/root:".to_string()),
            }),
            file: None,
            folder: Some(GraphFolderFacet {
                child_count: Some(42),
            }),
            deleted: None,
        };

        let item = DeltaParser::parse_item(graph_item);

        assert_eq!(item.id, "folder-001");
        assert_eq!(item.name, "Photos");
        assert_eq!(item.path, Some("/Photos".to_string()));
        assert!(item.is_directory);
        assert!(!item.is_deleted);
        assert!(item.hash.is_none());
    }

    #[test]
    fn test_parse_deleted_item() {
        let graph_item = GraphDriveItem {
            id: "deleted-001".to_string(),
            name: "obsolete.txt".to_string(),
            size: None,
            last_modified_date_time: None,
            parent_reference: None,
            file: None,
            folder: None,
            deleted: Some(GraphDeletedFacet {
                state: Some("deleted".to_string()),
            }),
        };

        let item = DeltaParser::parse_item(graph_item);

        assert_eq!(item.id, "deleted-001");
        assert_eq!(item.name, "obsolete.txt");
        assert!(item.path.is_none());
        assert!(item.is_deleted);
        assert!(!item.is_directory);
        assert!(item.size.is_none());
        assert!(item.modified.is_none());
        assert!(item.parent_id.is_none());
    }

    #[test]
    fn test_parse_item_in_root() {
        let graph_item = GraphDriveItem {
            id: "root-file".to_string(),
            name: "readme.md".to_string(),
            size: Some(1024),
            last_modified_date_time: None,
            parent_reference: Some(GraphParentReference {
                id: Some("root-id".to_string()),
                path: Some("/drive/root:".to_string()),
            }),
            file: Some(GraphFileFacet { hashes: None }),
            folder: None,
            deleted: None,
        };

        let item = DeltaParser::parse_item(graph_item);

        assert_eq!(item.path, Some("/readme.md".to_string()));
        assert!(!item.is_directory);
        assert!(item.hash.is_none());
    }

    #[test]
    fn test_parse_item_deeply_nested() {
        let graph_item = GraphDriveItem {
            id: "deep-001".to_string(),
            name: "deep-file.txt".to_string(),
            size: Some(256),
            last_modified_date_time: None,
            parent_reference: Some(GraphParentReference {
                id: Some("parent-deep".to_string()),
                path: Some("/drive/root:/A/B/C/D".to_string()),
            }),
            file: Some(GraphFileFacet { hashes: None }),
            folder: None,
            deleted: None,
        };

        let item = DeltaParser::parse_item(graph_item);

        assert_eq!(item.path, Some("/A/B/C/D/deep-file.txt".to_string()));
    }

    #[test]
    fn test_parse_complete_response() {
        let response = GraphDeltaResponse {
            value: vec![
                GraphDriveItem {
                    id: "item-1".to_string(),
                    name: "file1.txt".to_string(),
                    size: Some(100),
                    last_modified_date_time: None,
                    parent_reference: Some(GraphParentReference {
                        id: Some("root".to_string()),
                        path: Some("/drive/root:".to_string()),
                    }),
                    file: Some(GraphFileFacet { hashes: None }),
                    folder: None,
                    deleted: None,
                },
                GraphDriveItem {
                    id: "item-2".to_string(),
                    name: "folder1".to_string(),
                    size: Some(0),
                    last_modified_date_time: None,
                    parent_reference: Some(GraphParentReference {
                        id: Some("root".to_string()),
                        path: Some("/drive/root:".to_string()),
                    }),
                    file: None,
                    folder: Some(GraphFolderFacet {
                        child_count: Some(3),
                    }),
                    deleted: None,
                },
                GraphDriveItem {
                    id: "item-3".to_string(),
                    name: "deleted.txt".to_string(),
                    size: None,
                    last_modified_date_time: None,
                    parent_reference: None,
                    file: None,
                    folder: None,
                    deleted: Some(GraphDeletedFacet { state: None }),
                },
            ],
            next_link: None,
            delta_link: Some(
                "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=final".to_string(),
            ),
        };

        let result = DeltaParser::parse_response(response);

        assert_eq!(result.items.len(), 3);
        assert!(result.next_link.is_none());
        assert!(result.delta_link.is_some());

        // Verify individual items
        assert_eq!(result.items[0].id, "item-1");
        assert!(!result.items[0].is_directory);
        assert!(!result.items[0].is_deleted);

        assert_eq!(result.items[1].id, "item-2");
        assert!(result.items[1].is_directory);
        assert!(!result.items[1].is_deleted);

        assert_eq!(result.items[2].id, "item-3");
        assert!(!result.items[2].is_directory);
        assert!(result.items[2].is_deleted);
    }

    #[test]
    fn test_parse_empty_response() {
        let response = GraphDeltaResponse {
            value: vec![],
            next_link: None,
            delta_link: Some(
                "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=empty".to_string(),
            ),
        };

        let result = DeltaParser::parse_response(response);

        assert_eq!(result.items.len(), 0);
        assert!(result.delta_link.is_some());
    }

    // ========================================================================
    // Path normalization tests
    // ========================================================================

    #[test]
    fn test_normalize_parent_path_root() {
        let result = DeltaParser::normalize_parent_path("/drive/root:", "file.txt");
        assert_eq!(result, "/file.txt");
    }

    #[test]
    fn test_normalize_parent_path_subfolder() {
        let result = DeltaParser::normalize_parent_path("/drive/root:/Documents", "report.pdf");
        assert_eq!(result, "/Documents/report.pdf");
    }

    #[test]
    fn test_normalize_parent_path_deep_nesting() {
        let result = DeltaParser::normalize_parent_path("/drive/root:/A/B/C", "deep.txt");
        assert_eq!(result, "/A/B/C/deep.txt");
    }

    #[test]
    fn test_normalize_parent_path_no_prefix() {
        // Fallback behavior when path doesn't have the expected prefix
        let result = DeltaParser::normalize_parent_path("/some/other/path", "file.txt");
        assert_eq!(result, "/some/other/path/file.txt");
    }

    // ========================================================================
    // Delta token extraction tests
    // ========================================================================

    #[test]
    fn test_extract_delta_token() {
        let link = "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=abc123xyz";
        let token = DeltaParser::extract_delta_token(link);
        assert_eq!(token, Some("abc123xyz".to_string()));
    }

    #[test]
    fn test_extract_delta_token_encoded() {
        let link =
            "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=aHR0cHM6Ly9ncmFwaA%3D%3D";
        let token = DeltaParser::extract_delta_token(link);
        assert_eq!(token, Some("aHR0cHM6Ly9ncmFwaA==".to_string()));
    }

    #[test]
    fn test_extract_delta_token_missing() {
        let link = "https://graph.microsoft.com/v1.0/me/drive/root/delta";
        let token = DeltaParser::extract_delta_token(link);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_delta_token_invalid_url() {
        let link = "not a valid url";
        let token = DeltaParser::extract_delta_token(link);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_delta_token_with_other_params() {
        let link =
            "https://graph.microsoft.com/v1.0/me/drive/root/delta?foo=bar&token=mytoken&baz=qux";
        let token = DeltaParser::extract_delta_token(link);
        assert_eq!(token, Some("mytoken".to_string()));
    }

    // ========================================================================
    // Full JSON-to-DeltaResponse integration tests
    // ========================================================================

    #[test]
    fn test_full_json_parse_file_item() {
        let json = r#"{
            "value": [
                {
                    "id": "ABC123",
                    "name": "photo.jpg",
                    "size": 2048576,
                    "lastModifiedDateTime": "2025-08-01T12:00:00Z",
                    "parentReference": {
                        "id": "PARENT001",
                        "path": "/drive/root:/Pictures/Vacation"
                    },
                    "file": {
                        "hashes": {
                            "quickXorHash": "AAAAAAAAAAAAAAAAAAAAAAAAAAA="
                        }
                    }
                }
            ],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=saved"
        }"#;

        let raw: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        let response = DeltaParser::parse_response(raw);

        assert_eq!(response.items.len(), 1);
        let item = &response.items[0];
        assert_eq!(item.id, "ABC123");
        assert_eq!(item.name, "photo.jpg");
        assert_eq!(item.path, Some("/Pictures/Vacation/photo.jpg".to_string()));
        assert_eq!(item.size, Some(2048576));
        assert_eq!(item.hash, Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()));
        assert!(!item.is_deleted);
        assert!(!item.is_directory);
        assert_eq!(item.parent_id, Some("PARENT001".to_string()));
    }

    #[test]
    fn test_full_json_parse_mixed_items() {
        let json = r#"{
            "value": [
                {
                    "id": "file-1",
                    "name": "notes.txt",
                    "size": 512,
                    "lastModifiedDateTime": "2025-09-01T08:00:00Z",
                    "parentReference": {
                        "id": "root",
                        "path": "/drive/root:"
                    },
                    "file": {}
                },
                {
                    "id": "folder-1",
                    "name": "Archive",
                    "size": 0,
                    "lastModifiedDateTime": "2025-09-01T07:00:00Z",
                    "parentReference": {
                        "id": "root",
                        "path": "/drive/root:"
                    },
                    "folder": {
                        "childCount": 10
                    }
                },
                {
                    "id": "del-1",
                    "name": "temp.log",
                    "deleted": {}
                }
            ],
            "@odata.deltaLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?token=mixed"
        }"#;

        let raw: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        let response = DeltaParser::parse_response(raw);

        assert_eq!(response.items.len(), 3);

        // File
        assert_eq!(response.items[0].name, "notes.txt");
        assert!(!response.items[0].is_directory);
        assert!(!response.items[0].is_deleted);
        assert_eq!(response.items[0].path, Some("/notes.txt".to_string()));

        // Folder
        assert_eq!(response.items[1].name, "Archive");
        assert!(response.items[1].is_directory);
        assert!(!response.items[1].is_deleted);
        assert_eq!(response.items[1].path, Some("/Archive".to_string()));

        // Deleted
        assert_eq!(response.items[2].name, "temp.log");
        assert!(!response.items[2].is_directory);
        assert!(response.items[2].is_deleted);
        assert!(response.items[2].path.is_none());
    }

    #[test]
    fn test_full_json_parse_pagination_page() {
        let json = r#"{
            "value": [
                {
                    "id": "page-item-1",
                    "name": "data.csv",
                    "size": 4096,
                    "parentReference": {
                        "id": "root",
                        "path": "/drive/root:/Data"
                    },
                    "file": {}
                }
            ],
            "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/drive/root/delta?$skiptoken=next123"
        }"#;

        let raw: GraphDeltaResponse = serde_json::from_str(json).unwrap();
        let response = DeltaParser::parse_response(raw);

        assert_eq!(response.items.len(), 1);
        assert!(response.next_link.is_some());
        assert!(response.delta_link.is_none());
        assert!(response.next_link.unwrap().contains("$skiptoken=next123"));
    }

    // ========================================================================
    // get_delta URL construction test (verifies path building)
    // ========================================================================

    #[test]
    fn test_delta_path_without_token() {
        let path = DELTA_PATH.to_string();
        assert_eq!(path, "/me/drive/root/delta");
    }

    #[test]
    fn test_delta_path_with_token() {
        let token = DeltaToken::new("test-token-value".to_string()).unwrap();
        let path = format!("{}?token={}", DELTA_PATH, token.as_str());
        assert_eq!(path, "/me/drive/root/delta?token=test-token-value");
    }
}
