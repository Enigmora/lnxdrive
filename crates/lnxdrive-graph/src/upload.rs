//! Upload operations for Microsoft Graph API (OneDrive)
//!
//! Provides functions for uploading files to OneDrive:
//! - [`upload_small`] - Single-request upload for files under 4MB
//! - [`upload_large`] - Resumable upload session for large files (10MB chunks)
//! - [`create_upload_session`] - Creates a resumable upload session
//! - [`upload_chunk`] - Uploads a single chunk within a session
//!
//! ## Microsoft Graph API References
//!
//! - [Upload small files](https://learn.microsoft.com/en-us/graph/api/driveitem-put-content)
//! - [Upload large files](https://learn.microsoft.com/en-us/graph/api/driveitem-createuploadsession)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use lnxdrive_core::domain::newtypes::RemotePath;
use lnxdrive_core::ports::cloud_provider::DeltaItem;
use reqwest::Method;
use serde::Deserialize;
use tracing::{debug, info};

use crate::client::GraphClient;

/// Chunk size for large file uploads: 10 MiB (10,485,760 bytes)
///
/// Microsoft recommends chunk sizes that are multiples of 320 KiB.
/// 10 MiB = 10,485,760 = 320 KiB * 32, which satisfies this requirement.
const CHUNK_SIZE: usize = 10 * 1024 * 1024;

// ============================================================================
// Graph API DriveItem response types for deserialization
// ============================================================================

/// Represents a DriveItem response from the Microsoft Graph API
///
/// This struct maps the JSON response returned after upload operations.
/// Fields use `Option` because not all fields are present in every response
/// (e.g., deleted items lack file metadata, folders lack file hashes).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDriveItem {
    /// OneDrive item ID
    id: String,
    /// Item name (file or folder name)
    name: String,
    /// File size in bytes
    size: Option<u64>,
    /// Last modified timestamp in ISO 8601 format
    last_modified_date_time: Option<String>,
    /// Reference to the parent folder
    parent_reference: Option<ParentReference>,
    /// Present if the item is a file (contains hashes)
    file: Option<FileInfo>,
    /// Present if the item is a folder
    folder: Option<serde_json::Value>,
    /// Present if the item has been deleted
    deleted: Option<serde_json::Value>,
}

/// Parent folder reference in a DriveItem response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParentReference {
    /// Full path of the parent folder (e.g., "/drive/root:/Documents")
    path: Option<String>,
    /// Parent folder item ID
    id: Option<String>,
}

/// File-specific metadata in a DriveItem response
#[derive(Debug, Deserialize)]
struct FileInfo {
    /// File content hashes
    hashes: Option<FileHashes>,
}

/// Content hashes for a file
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileHashes {
    /// QuickXorHash used by OneDrive for integrity verification
    quick_xor_hash: Option<String>,
}

/// Response from creating an upload session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadSessionResponse {
    /// The URL to use for uploading chunks
    upload_url: String,
}

// ============================================================================
// GraphDriveItem -> DeltaItem conversion
// ============================================================================

/// Converts a `GraphDriveItem` into the port-level `DeltaItem` DTO
///
/// This extracts and maps fields from the Graph API response format into
/// the provider-agnostic `DeltaItem` structure used by the core layer.
fn drive_item_to_delta(item: GraphDriveItem) -> DeltaItem {
    let is_directory = item.folder.is_some();
    let is_deleted = item.deleted.is_some();

    // Parse the ISO 8601 timestamp if present
    let modified = item
        .last_modified_date_time
        .as_deref()
        .and_then(|s| s.parse::<DateTime<Utc>>().ok());

    // Extract the quickXorHash from nested file info
    let hash = item
        .file
        .as_ref()
        .and_then(|f| f.hashes.as_ref())
        .and_then(|h| h.quick_xor_hash.clone());

    // Extract parent path, stripping the "/drive/root:" prefix if present
    let path = item
        .parent_reference
        .as_ref()
        .and_then(|pr| pr.path.as_deref())
        .map(|p| {
            // Graph API returns paths like "/drive/root:/Documents"
            // We want just "/Documents" or "/" for the root
            if let Some(stripped) = p.strip_prefix("/drive/root:") {
                if stripped.is_empty() {
                    "/".to_string()
                } else {
                    stripped.to_string()
                }
            } else {
                p.to_string()
            }
        })
        .map(|parent_path| {
            // Build full path: parent_path + "/" + name
            if parent_path == "/" {
                format!("/{}", item.name)
            } else {
                format!("{}/{}", parent_path, item.name)
            }
        });

    let parent_id = item.parent_reference.as_ref().and_then(|pr| pr.id.clone());

    DeltaItem {
        id: item.id,
        name: item.name,
        path,
        size: item.size,
        hash,
        modified,
        is_deleted,
        is_directory,
        parent_id,
    }
}

// ============================================================================
// API path construction helper
// ============================================================================

/// Builds the Graph API path for file operations using the item-by-path pattern
///
/// For OneDrive, the path format is:
/// - Root: `/me/drive/root:/{name}:/{suffix}`
/// - Subfolder: `/me/drive/root:{parent_path}/{name}:/{suffix}`
///
/// # Arguments
/// * `parent_path` - Parent folder remote path (e.g., "/" or "/Documents")
/// * `name` - File name (e.g., "file.txt")
/// * `suffix` - API operation suffix (e.g., "content" or "createUploadSession")
fn build_item_path(parent_path: &RemotePath, name: &str, suffix: &str) -> String {
    if parent_path.as_str() == "/" {
        // Root: /me/drive/root:/file.txt:/content
        format!("/me/drive/root:/{}:/{}", name, suffix)
    } else {
        // Subfolder: /me/drive/root:/Documents/file.txt:/content
        format!(
            "/me/drive/root:{}/{}:/{}",
            parent_path.as_str(),
            name,
            suffix
        )
    }
}

// ============================================================================
// T140: upload_small
// ============================================================================

/// Uploads a small file (< 4MB) in a single PUT request
///
/// Uses the simple upload API: `PUT /me/drive/root:{path}:/content`
/// with the file bytes as the request body.
///
/// # Arguments
/// * `client` - The authenticated GraphClient
/// * `parent_path` - Remote path of the parent folder
/// * `name` - File name to create/overwrite
/// * `data` - File contents (must be < 4MB)
///
/// # Returns
/// A `DeltaItem` with the metadata of the uploaded file
///
/// # Errors
/// Returns an error if the upload request fails or the response cannot be parsed
pub async fn upload_small(
    client: &GraphClient,
    parent_path: &RemotePath,
    name: &str,
    data: &[u8],
) -> Result<DeltaItem> {
    let path = build_item_path(parent_path, name, "content");
    debug!(
        "Uploading small file ({} bytes): {} -> {}",
        data.len(),
        name,
        path
    );

    let item: GraphDriveItem = client
        .request(Method::PUT, &path)
        .header("Content-Type", "application/octet-stream")
        .body(data.to_vec())
        .send()
        .await
        .context("Failed to send small upload request")?
        .error_for_status()
        .context("Small upload returned error status")?
        .json()
        .await
        .context("Failed to parse upload response")?;

    debug!("Small upload completed: id={}, name={}", item.id, item.name);
    Ok(drive_item_to_delta(item))
}

// ============================================================================
// T141: create_upload_session
// ============================================================================

/// Creates a resumable upload session for large files
///
/// Uses the upload session API: `POST /me/drive/root:{path}:/createUploadSession`
///
/// The returned upload URL can be used with [`upload_chunk`] to upload the file
/// in parts. The session URL is valid for a limited time (typically 15 minutes
/// of inactivity).
///
/// # Arguments
/// * `client` - The authenticated GraphClient
/// * `parent_path` - Remote path of the parent folder
/// * `name` - File name to create/overwrite
///
/// # Returns
/// The upload session URL as a `String`
///
/// # Errors
/// Returns an error if the session creation request fails
pub async fn create_upload_session(
    client: &GraphClient,
    parent_path: &RemotePath,
    name: &str,
) -> Result<String> {
    let path = build_item_path(parent_path, name, "createUploadSession");
    debug!("Creating upload session for: {}", name);

    let response: UploadSessionResponse = client
        .request(Method::POST, &path)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .context("Failed to create upload session")?
        .error_for_status()
        .context("Create upload session returned error status")?
        .json()
        .await
        .context("Failed to parse upload session response")?;

    debug!("Upload session created: {}", response.upload_url);
    Ok(response.upload_url)
}

// ============================================================================
// T142: upload_chunk
// ============================================================================

/// Uploads a single chunk of data to a resumable upload session
///
/// Sends a PUT request to the upload session URL with a `Content-Range` header
/// specifying the byte range being uploaded.
///
/// # Arguments
/// * `client` - An HTTP client (the raw reqwest client, not the GraphClient,
///   because upload session URLs are absolute and don't need the base URL)
/// * `upload_url` - The upload session URL from [`create_upload_session`]
/// * `access_token` - Bearer token for authentication
/// * `data` - The chunk bytes to upload
/// * `offset` - Byte offset of this chunk within the total file
/// * `total` - Total file size in bytes
///
/// # Returns
/// - `Some(Value)` with the completed DriveItem JSON on the final chunk
/// - `None` for intermediate chunks (HTTP 202 Accepted)
///
/// # Errors
/// Returns an error if the chunk upload fails
pub async fn upload_chunk(
    client: &reqwest::Client,
    upload_url: &str,
    access_token: &str,
    data: &[u8],
    offset: u64,
    total: u64,
) -> Result<Option<serde_json::Value>> {
    let chunk_len = data.len() as u64;
    let range_end = offset + chunk_len - 1;
    let content_range = format!("bytes {}-{}/{}", offset, range_end, total);

    debug!("Uploading chunk: {} ({} bytes)", content_range, chunk_len);

    let response = client
        .put(upload_url)
        .bearer_auth(access_token)
        .header("Content-Length", chunk_len.to_string())
        .header("Content-Range", &content_range)
        .body(data.to_vec())
        .send()
        .await
        .context("Failed to send chunk upload request")?;

    let status = response.status();

    if status.is_success() {
        let body: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse chunk response body")?;

        if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::CREATED {
            // Upload complete - the response contains the final DriveItem
            debug!("Upload session completed (status {})", status);
            Ok(Some(body))
        } else {
            // Intermediate chunk accepted (202)
            debug!("Chunk accepted (status {})", status);
            Ok(None)
        }
    } else {
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "unable to read error body".to_string());
        anyhow::bail!("Chunk upload failed with status {}: {}", status, error_body);
    }
}

// ============================================================================
// T143: upload_large
// ============================================================================

/// Uploads a large file using a resumable upload session with 10MB chunks
///
/// This function orchestrates the entire large file upload process:
/// 1. Creates an upload session via [`create_upload_session`]
/// 2. Splits the data into 10MB chunks
/// 3. Uploads each chunk via [`upload_chunk`]
/// 4. Reports progress after each chunk via the optional callback
/// 5. Parses the final response into a `DeltaItem`
///
/// # Arguments
/// * `client` - The authenticated GraphClient
/// * `parent_path` - Remote path of the parent folder
/// * `name` - File name to create/overwrite
/// * `data` - Complete file contents
/// * `progress` - Optional callback `(bytes_sent, total_bytes)` called after each chunk
///
/// # Returns
/// A `DeltaItem` with the metadata of the uploaded file
///
/// # Errors
/// Returns an error if session creation, any chunk upload, or response parsing fails
pub async fn upload_large(
    client: &GraphClient,
    parent_path: &RemotePath,
    name: &str,
    data: &[u8],
    progress: Option<Box<dyn Fn(u64, u64) + Send>>,
) -> Result<DeltaItem> {
    let total = data.len() as u64;
    info!(
        "Starting large file upload: {} ({} bytes, {} chunks)",
        name,
        total,
        (total + CHUNK_SIZE as u64 - 1) / CHUNK_SIZE as u64
    );

    // Step 1: Create upload session
    let upload_url = create_upload_session(client, parent_path, name).await?;

    // Step 2: Upload chunks
    let http_client = client.http_client();
    let access_token = client.access_token();
    let mut offset: u64 = 0;
    let mut final_response: Option<serde_json::Value> = None;

    while offset < total {
        let end = std::cmp::min(offset + CHUNK_SIZE as u64, total);
        let chunk = &data[offset as usize..end as usize];

        let result = upload_chunk(http_client, &upload_url, access_token, chunk, offset, total)
            .await
            .with_context(|| {
                format!(
                    "Failed to upload chunk at offset {}/{} for {}",
                    offset, total, name
                )
            })?;

        offset = end;

        // Report progress
        if let Some(ref cb) = progress {
            cb(offset, total);
        }

        if let Some(response) = result {
            final_response = Some(response);
        }
    }

    // Step 3: Parse the final response into a DeltaItem
    let response_json = final_response
        .context("Upload session completed without receiving a final DriveItem response")?;

    let item: GraphDriveItem = serde_json::from_value(response_json)
        .context("Failed to deserialize final upload response into DriveItem")?;

    info!(
        "Large upload completed: id={}, name={}, size={:?}",
        item.id, item.name, item.size
    );

    Ok(drive_item_to_delta(item))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- GraphDriveItem deserialization tests ----

    #[test]
    fn test_drive_item_deserialization_file() {
        let json = r#"{
            "id": "01BYE5RZ6QN3ZWBTUFOFD3GSPGOHDJD36K",
            "name": "document.pdf",
            "size": 1048576,
            "lastModifiedDateTime": "2025-06-15T10:30:00Z",
            "parentReference": {
                "path": "/drive/root:/Documents",
                "id": "01BYE5RZ5PXRAAAAAAAAAAAAAAAA"
            },
            "file": {
                "hashes": {
                    "quickXorHash": "AAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }
        }"#;

        let item: GraphDriveItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "01BYE5RZ6QN3ZWBTUFOFD3GSPGOHDJD36K");
        assert_eq!(item.name, "document.pdf");
        assert_eq!(item.size, Some(1048576));
        assert!(item.file.is_some());
        assert!(item.folder.is_none());
        assert!(item.deleted.is_none());
    }

    #[test]
    fn test_drive_item_deserialization_folder() {
        let json = r#"{
            "id": "FOLDER123",
            "name": "My Folder",
            "size": 0,
            "lastModifiedDateTime": "2025-06-15T10:30:00Z",
            "parentReference": {
                "path": "/drive/root:",
                "id": "ROOT_ID"
            },
            "folder": {
                "childCount": 5
            }
        }"#;

        let item: GraphDriveItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.name, "My Folder");
        assert!(item.folder.is_some());
        assert!(item.file.is_none());
    }

    #[test]
    fn test_drive_item_deserialization_minimal() {
        let json = r#"{
            "id": "ITEM_ID",
            "name": "file.txt"
        }"#;

        let item: GraphDriveItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "ITEM_ID");
        assert_eq!(item.name, "file.txt");
        assert!(item.size.is_none());
        assert!(item.last_modified_date_time.is_none());
        assert!(item.parent_reference.is_none());
        assert!(item.file.is_none());
        assert!(item.folder.is_none());
        assert!(item.deleted.is_none());
    }

    // ---- drive_item_to_delta conversion tests ----

    #[test]
    fn test_drive_item_to_delta_file() {
        let item = GraphDriveItem {
            id: "FILE_ID".to_string(),
            name: "report.docx".to_string(),
            size: Some(2048),
            last_modified_date_time: Some("2025-06-15T10:30:00Z".to_string()),
            parent_reference: Some(ParentReference {
                path: Some("/drive/root:/Documents".to_string()),
                id: Some("PARENT_ID".to_string()),
            }),
            file: Some(FileInfo {
                hashes: Some(FileHashes {
                    quick_xor_hash: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()),
                }),
            }),
            folder: None,
            deleted: None,
        };

        let delta = drive_item_to_delta(item);
        assert_eq!(delta.id, "FILE_ID");
        assert_eq!(delta.name, "report.docx");
        assert_eq!(delta.path, Some("/Documents/report.docx".to_string()));
        assert_eq!(delta.size, Some(2048));
        assert_eq!(delta.hash, Some("AAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()));
        assert!(delta.modified.is_some());
        assert!(!delta.is_deleted);
        assert!(!delta.is_directory);
        assert_eq!(delta.parent_id, Some("PARENT_ID".to_string()));
    }

    #[test]
    fn test_drive_item_to_delta_folder() {
        let item = GraphDriveItem {
            id: "FOLDER_ID".to_string(),
            name: "Photos".to_string(),
            size: Some(0),
            last_modified_date_time: Some("2025-01-01T00:00:00Z".to_string()),
            parent_reference: Some(ParentReference {
                path: Some("/drive/root:".to_string()),
                id: Some("ROOT_ID".to_string()),
            }),
            file: None,
            folder: Some(serde_json::json!({"childCount": 10})),
            deleted: None,
        };

        let delta = drive_item_to_delta(item);
        assert_eq!(delta.id, "FOLDER_ID");
        assert_eq!(delta.name, "Photos");
        assert_eq!(delta.path, Some("/Photos".to_string()));
        assert!(delta.is_directory);
        assert!(!delta.is_deleted);
        assert!(delta.hash.is_none());
    }

    #[test]
    fn test_drive_item_to_delta_deleted() {
        let item = GraphDriveItem {
            id: "DELETED_ID".to_string(),
            name: "old-file.txt".to_string(),
            size: None,
            last_modified_date_time: None,
            parent_reference: None,
            file: None,
            folder: None,
            deleted: Some(serde_json::json!({})),
        };

        let delta = drive_item_to_delta(item);
        assert_eq!(delta.id, "DELETED_ID");
        assert!(delta.is_deleted);
        assert!(!delta.is_directory);
        assert!(delta.path.is_none());
        assert!(delta.modified.is_none());
        assert!(delta.parent_id.is_none());
    }

    #[test]
    fn test_drive_item_to_delta_root_parent() {
        let item = GraphDriveItem {
            id: "ROOT_FILE".to_string(),
            name: "readme.md".to_string(),
            size: Some(512),
            last_modified_date_time: None,
            parent_reference: Some(ParentReference {
                path: Some("/drive/root:".to_string()),
                id: Some("ROOT_ID".to_string()),
            }),
            file: Some(FileInfo { hashes: None }),
            folder: None,
            deleted: None,
        };

        let delta = drive_item_to_delta(item);
        assert_eq!(delta.path, Some("/readme.md".to_string()));
        assert!(delta.hash.is_none());
    }

    #[test]
    fn test_drive_item_to_delta_nested_path() {
        let item = GraphDriveItem {
            id: "NESTED_FILE".to_string(),
            name: "data.csv".to_string(),
            size: Some(1024),
            last_modified_date_time: None,
            parent_reference: Some(ParentReference {
                path: Some("/drive/root:/Projects/Analysis".to_string()),
                id: Some("ANALYSIS_ID".to_string()),
            }),
            file: None,
            folder: None,
            deleted: None,
        };

        let delta = drive_item_to_delta(item);
        assert_eq!(delta.path, Some("/Projects/Analysis/data.csv".to_string()));
    }

    // ---- build_item_path tests ----

    #[test]
    fn test_build_item_path_root() {
        let path = RemotePath::root();
        let result = build_item_path(&path, "file.txt", "content");
        assert_eq!(result, "/me/drive/root:/file.txt:/content");
    }

    #[test]
    fn test_build_item_path_subfolder() {
        let path = RemotePath::new("/Documents".to_string()).unwrap();
        let result = build_item_path(&path, "file.txt", "content");
        assert_eq!(result, "/me/drive/root:/Documents/file.txt:/content");
    }

    #[test]
    fn test_build_item_path_nested_subfolder() {
        let path = RemotePath::new("/Documents/Projects".to_string()).unwrap();
        let result = build_item_path(&path, "report.pdf", "content");
        assert_eq!(
            result,
            "/me/drive/root:/Documents/Projects/report.pdf:/content"
        );
    }

    #[test]
    fn test_build_item_path_create_upload_session() {
        let path = RemotePath::new("/Documents".to_string()).unwrap();
        let result = build_item_path(&path, "large.zip", "createUploadSession");
        assert_eq!(
            result,
            "/me/drive/root:/Documents/large.zip:/createUploadSession"
        );
    }

    // ---- UploadSessionResponse deserialization test ----

    #[test]
    fn test_upload_session_response_deserialization() {
        let json = r#"{
            "uploadUrl": "https://sn3302.up.1drv.com/up/fe6987415ace7X4811700/myfile.txt",
            "expirationDateTime": "2025-06-15T12:00:00Z"
        }"#;

        let response: UploadSessionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            response.upload_url,
            "https://sn3302.up.1drv.com/up/fe6987415ace7X4811700/myfile.txt"
        );
    }

    // ---- CHUNK_SIZE constant test ----

    #[test]
    fn test_chunk_size_is_multiple_of_320kib() {
        // Microsoft requires chunk sizes to be multiples of 320 KiB
        let kib_320 = 320 * 1024;
        assert_eq!(
            CHUNK_SIZE % kib_320,
            0,
            "CHUNK_SIZE must be a multiple of 320 KiB"
        );
    }

    #[test]
    fn test_chunk_size_is_10mib() {
        assert_eq!(CHUNK_SIZE, 10 * 1024 * 1024);
    }
}
