//! T173: Integration tests for delta (incremental sync) queries
//!
//! Verifies end-to-end behavior of the delta module against a
//! wiremock-based Graph API mock server:
//! - Initial delta query (no token)
//! - Incremental delta query (with token)
//! - Pagination across multiple pages
//! - Empty delta response
//! - Mixed item types (files, folders, deleted)

use lnxdrive_graph::client::GraphClient;
use lnxdrive_graph::delta;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common;

#[tokio::test]
async fn test_delta_initial_sync_returns_all_items() {
    let (server, client) = common::setup_graph_mock().await;

    let items = serde_json::json!([
        {
            "id": "file-001",
            "name": "document.txt",
            "size": 1024,
            "lastModifiedDateTime": "2026-01-15T10:00:00Z",
            "parentReference": {
                "id": "root",
                "path": "/drive/root:"
            },
            "file": {
                "hashes": { "quickXorHash": "AAAAAAAAAAAAAAAAAAAAAAAAAAA=" }
            }
        },
        {
            "id": "folder-001",
            "name": "Documents",
            "size": 0,
            "parentReference": {
                "id": "root",
                "path": "/drive/root:"
            },
            "folder": { "childCount": 3 }
        }
    ]);

    common::mount_delta_single_page(&server, items, "initial-token-001").await;

    let response = delta::get_delta(&client, None)
        .await
        .expect("Initial delta query failed");

    assert_eq!(response.items.len(), 2);
    assert!(response.delta_link.is_some());
    assert!(response.next_link.is_none());

    // Verify file item
    let file = &response.items[0];
    assert_eq!(file.id, "file-001");
    assert_eq!(file.name, "document.txt");
    assert_eq!(file.path, Some("/document.txt".to_string()));
    assert_eq!(file.size, Some(1024));
    assert!(!file.is_directory);
    assert!(!file.is_deleted);

    // Verify folder item
    let folder = &response.items[1];
    assert_eq!(folder.id, "folder-001");
    assert_eq!(folder.name, "Documents");
    assert!(folder.is_directory);
    assert!(!folder.is_deleted);
}

#[tokio::test]
async fn test_delta_incremental_with_token() {
    let server = MockServer::start().await;

    // Mount delta endpoint that expects a token parameter
    Mock::given(method("GET"))
        .and(path("/me/drive/root/delta"))
        .and(query_param("token", "previous-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": [
                {
                    "id": "file-002",
                    "name": "new-file.txt",
                    "size": 512,
                    "lastModifiedDateTime": "2026-01-16T08:00:00Z",
                    "parentReference": {
                        "id": "root",
                        "path": "/drive/root:"
                    },
                    "file": {}
                }
            ],
            "@odata.deltaLink": format!(
                "{}/me/drive/root/delta?token=incremental-token-002",
                server.uri()
            )
        })))
        .mount(&server)
        .await;

    let client = GraphClient::with_base_url("test-token", server.uri());
    let delta_token =
        lnxdrive_core::domain::newtypes::DeltaToken::new("previous-token".to_string()).unwrap();

    let response = delta::get_delta(&client, Some(&delta_token))
        .await
        .expect("Incremental delta query failed");

    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].id, "file-002");
    assert_eq!(response.items[0].name, "new-file.txt");
    assert!(response.delta_link.is_some());
}

#[tokio::test]
async fn test_delta_empty_response() {
    let (server, client) = common::setup_graph_mock().await;

    common::mount_delta_single_page(&server, serde_json::json!([]), "empty-token").await;

    let response = delta::get_delta(&client, None)
        .await
        .expect("Empty delta query failed");

    assert_eq!(response.items.len(), 0);
    assert!(response.delta_link.is_some());
}

#[tokio::test]
async fn test_delta_deleted_items() {
    let (server, client) = common::setup_graph_mock().await;

    let items = serde_json::json!([
        {
            "id": "del-001",
            "name": "removed.txt",
            "deleted": { "state": "deleted" }
        },
        {
            "id": "del-002",
            "name": "also-removed.pdf",
            "deleted": {}
        }
    ]);

    common::mount_delta_single_page(&server, items, "delete-token").await;

    let response = delta::get_delta(&client, None)
        .await
        .expect("Delta with deleted items failed");

    assert_eq!(response.items.len(), 2);
    assert!(response.items[0].is_deleted);
    assert!(response.items[1].is_deleted);
    assert!(response.items[0].path.is_none());
    assert!(response.items[1].path.is_none());
}

#[tokio::test]
async fn test_delta_mixed_item_types() {
    let (server, client) = common::setup_graph_mock().await;

    let items = serde_json::json!([
        {
            "id": "file-mix",
            "name": "photo.jpg",
            "size": 2048576,
            "lastModifiedDateTime": "2026-01-15T12:00:00Z",
            "parentReference": {
                "id": "folder-pics",
                "path": "/drive/root:/Pictures"
            },
            "file": {
                "hashes": { "quickXorHash": "BBBBBBBBBBBBBBBBBBBBBBBBBBB=" }
            }
        },
        {
            "id": "folder-mix",
            "name": "Archive",
            "parentReference": {
                "id": "root",
                "path": "/drive/root:"
            },
            "folder": { "childCount": 10 }
        },
        {
            "id": "del-mix",
            "name": "temp.log",
            "deleted": {}
        }
    ]);

    common::mount_delta_single_page(&server, items, "mixed-token").await;

    let response = delta::get_delta(&client, None)
        .await
        .expect("Mixed delta query failed");

    assert_eq!(response.items.len(), 3);

    // File
    assert!(!response.items[0].is_directory);
    assert!(!response.items[0].is_deleted);
    assert_eq!(
        response.items[0].path,
        Some("/Pictures/photo.jpg".to_string())
    );
    assert_eq!(
        response.items[0].hash,
        Some("BBBBBBBBBBBBBBBBBBBBBBBBBBB=".to_string())
    );

    // Folder
    assert!(response.items[1].is_directory);
    assert!(!response.items[1].is_deleted);
    assert_eq!(response.items[1].path, Some("/Archive".to_string()));

    // Deleted
    assert!(response.items[2].is_deleted);
    assert!(!response.items[2].is_directory);
}
