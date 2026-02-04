//! T015: Shared test helpers for Graph API integration tests
//!
//! Provides wiremock-based mock server setup for Microsoft Graph API endpoints.
//! Each helper mounts the necessary mock endpoints and returns a configured
//! GraphClient pointing at the mock server.

use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use lnxdrive_graph::client::GraphClient;

/// Sets up a mock server with common Graph API endpoints and returns
/// a (MockServer, GraphClient) tuple.
///
/// Pre-configured endpoints:
/// - GET /me → user profile
/// - GET /me/drive → drive quota
pub async fn setup_graph_mock() -> (MockServer, GraphClient) {
    let server = MockServer::start().await;

    // Mock GET /me - user profile
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "displayName": "Test User",
            "mail": "test@example.com",
            "userPrincipalName": "test@example.com",
            "id": "user-test-001"
        })))
        .mount(&server)
        .await;

    // Mock GET /me/drive - drive quota
    Mock::given(method("GET"))
        .and(path("/me/drive"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "drive-test-001",
            "quota": {
                "total": 5368709120_u64,
                "used": 1073741824_u64,
                "remaining": 4294967296_u64
            }
        })))
        .mount(&server)
        .await;

    let client = GraphClient::with_base_url("test-access-token", server.uri());

    (server, client)
}

/// Mounts a delta endpoint that returns a single page with given items.
pub async fn mount_delta_single_page(
    server: &MockServer,
    items: serde_json::Value,
    delta_token: &str,
) {
    Mock::given(method("GET"))
        .and(path("/me/drive/root/delta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": items,
            "@odata.deltaLink": format!(
                "{}/me/drive/root/delta?token={}",
                server.uri(),
                delta_token
            )
        })))
        .mount(server)
        .await;
}

/// Mounts a delta endpoint that returns two pages (pagination test).
///
/// First request returns page 1 with a nextLink.
/// Second request (to the nextLink) returns page 2 with a deltaLink.
#[allow(dead_code)]
pub async fn mount_delta_paginated(
    server: &MockServer,
    page1_items: serde_json::Value,
    page2_items: serde_json::Value,
    delta_token: &str,
) {
    // Page 1: returns nextLink
    Mock::given(method("GET"))
        .and(path("/me/drive/root/delta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": page1_items,
            "@odata.nextLink": format!(
                "{}/me/drive/root/delta?$skiptoken=page2",
                server.uri()
            )
        })))
        .up_to_n_times(1)
        .mount(server)
        .await;

    // Page 2: returns deltaLink (via the nextLink URL)
    // Note: The nextLink is an absolute URL, so the delta module will
    // use a direct HTTP GET. We mock any path containing $skiptoken=page2.
    Mock::given(method("GET"))
        .and(path_regex(r"/me/drive/root/delta\??\$skiptoken=page2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "value": page2_items,
            "@odata.deltaLink": format!(
                "{}/me/drive/root/delta?token={}",
                server.uri(),
                delta_token
            )
        })))
        .mount(server)
        .await;
}

/// Mounts a file download endpoint for a specific item ID.
pub async fn mount_download(server: &MockServer, item_id: &str, content: &[u8]) {
    let path_str = format!("/me/drive/items/{}/content", item_id);
    Mock::given(method("GET"))
        .and(path(&path_str))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(content.to_vec())
                .append_header("Content-Type", "application/octet-stream"),
        )
        .mount(server)
        .await;
}

/// Mounts a small file upload endpoint that accepts PUT requests.
pub async fn mount_upload_small(
    server: &MockServer,
    remote_path: &str,
    response_id: &str,
    response_name: &str,
) {
    // PUT /me/drive/root:/{path}:/content
    let path_str = format!("/me/drive/root:{}:/content", remote_path);
    Mock::given(method("PUT"))
        .and(path(&path_str))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "id": response_id,
            "name": response_name,
            "size": 1024,
            "lastModifiedDateTime": "2026-01-15T10:00:00Z",
            "parentReference": {
                "id": "parent-001",
                "path": "/drive/root:/Documents"
            },
            "file": {
                "hashes": {
                    "quickXorHash": "AAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }
        })))
        .mount(server)
        .await;
}
