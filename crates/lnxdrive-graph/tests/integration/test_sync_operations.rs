//! T166: Integration tests for sync operations (upload/download)
//!
//! Verifies end-to-end behavior of file upload and download operations
//! against a wiremock-based Graph API mock server.

use lnxdrive_core::domain::newtypes::RemoteId;
use lnxdrive_graph::{client::GraphClient, upload};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::common;

// ============================================================================
// Download tests
// ============================================================================

#[tokio::test]
async fn test_download_file_returns_content() {
    let (server, client) = common::setup_graph_mock().await;

    let file_content = b"Hello, OneDrive! This is test content.";
    common::mount_download(&server, "download-001", file_content).await;

    let remote_id = RemoteId::new("download-001".to_string()).unwrap();
    let data = client
        .download_file(&remote_id)
        .await
        .expect("Download failed");

    assert_eq!(data, file_content);
}

#[tokio::test]
async fn test_download_large_file() {
    let (server, client) = common::setup_graph_mock().await;

    // Create a 1MB test file
    let file_content: Vec<u8> = (0..1_048_576).map(|i| (i % 256) as u8).collect();
    common::mount_download(&server, "large-001", &file_content).await;

    let remote_id = RemoteId::new("large-001".to_string()).unwrap();
    let data = client
        .download_file(&remote_id)
        .await
        .expect("Large download failed");

    assert_eq!(data.len(), 1_048_576);
    assert_eq!(data, file_content);
}

#[tokio::test]
async fn test_download_empty_file() {
    let (server, client) = common::setup_graph_mock().await;

    common::mount_download(&server, "empty-001", &[]).await;

    let remote_id = RemoteId::new("empty-001".to_string()).unwrap();
    let data = client
        .download_file(&remote_id)
        .await
        .expect("Empty download failed");

    assert!(data.is_empty());
}

// ============================================================================
// Upload tests
// ============================================================================

#[tokio::test]
async fn test_upload_small_file() {
    let (server, client) = common::setup_graph_mock().await;

    common::mount_upload_small(&server, "/Documents/test.txt", "upload-001", "test.txt").await;

    let parent_path =
        lnxdrive_core::domain::newtypes::RemotePath::new("/Documents".to_string()).unwrap();
    let data = b"Small file content for upload test";

    let result = upload::upload_small(&client, &parent_path, "test.txt", data)
        .await
        .expect("Small upload failed");

    assert_eq!(result.id, "upload-001");
    assert_eq!(result.name, "test.txt");
    assert!(!result.is_deleted);
    assert!(!result.is_directory);
}

// ============================================================================
// Error handling tests
// ============================================================================

#[tokio::test]
async fn test_download_returns_error_on_404() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/me/drive/items/nonexistent/content"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": {
                "code": "itemNotFound",
                "message": "Item not found"
            }
        })))
        .mount(&server)
        .await;

    let client = GraphClient::with_base_url("test-token", server.uri());
    let remote_id = RemoteId::new("nonexistent".to_string()).unwrap();

    let result = client.download_file(&remote_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_user_info_returns_error_on_401() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": {
                "code": "InvalidAuthenticationToken",
                "message": "Access token has expired"
            }
        })))
        .mount(&server)
        .await;

    let client = GraphClient::with_base_url("expired-token", server.uri());

    let result = client.get_user_info().await;
    assert!(result.is_err());
}
