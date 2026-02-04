//! T015: Integration test for Graph API user info endpoint
//!
//! Verifies that GraphClient::get_user_info() correctly fetches
//! and parses the /me and /me/drive responses.

use crate::common;

#[tokio::test]
async fn test_get_user_info_returns_profile_and_quota() {
    let (_server, client) = common::setup_graph_mock().await;

    let user_info = client.get_user_info().await.expect("get_user_info failed");

    assert_eq!(user_info.email, "test@example.com");
    assert_eq!(user_info.display_name, "Test User");
    assert_eq!(user_info.id, "user-test-001");
    assert_eq!(user_info.quota_total, 5_368_709_120);
    assert_eq!(user_info.quota_used, 1_073_741_824);
}

#[tokio::test]
async fn test_get_drive_quota() {
    let (_server, client) = common::setup_graph_mock().await;

    let (used, total) = client
        .get_drive_quota()
        .await
        .expect("get_drive_quota failed");

    assert_eq!(used, 1_073_741_824);
    assert_eq!(total, 5_368_709_120);
}
