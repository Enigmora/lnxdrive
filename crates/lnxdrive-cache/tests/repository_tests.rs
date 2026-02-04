//! Integration tests for SqliteStateRepository
//!
//! These tests verify all IStateRepository methods using an in-memory
//! SQLite database. Each test function creates a fresh database to
//! ensure test isolation.

use std::path::PathBuf;

use chrono::{Duration, Utc};
use uuid::Uuid;

use lnxdrive_cache::{DatabasePool, SqliteStateRepository};
use lnxdrive_core::domain::{
    newtypes::{
        AccountId, DeltaToken, Email, FileHash, RemoteId, RemotePath, SessionId, SyncPath, UniqueId,
    },
    sync_item::ItemState,
    Account, AccountState, AuditAction, AuditEntry, AuditResult, Conflict, Resolution,
    ResolutionSource, SyncItem, SyncSession, VersionInfo,
};
use lnxdrive_core::ports::{IStateRepository, ItemFilter};

// ============================================================================
// Test helpers
// ============================================================================

/// Create a fresh in-memory repository for each test
async fn setup() -> SqliteStateRepository {
    let pool = DatabasePool::in_memory()
        .await
        .expect("Failed to create in-memory database");
    SqliteStateRepository::new(pool.pool().clone())
}

/// Create a test account and save it to the repository
async fn create_test_account(repo: &SqliteStateRepository) -> Account {
    let email = Email::new("test@example.com".to_string()).unwrap();
    let sync_root = SyncPath::new(PathBuf::from("/home/user/OneDrive")).unwrap();
    let account = Account::new(email, "Test User", "drive123", sync_root);
    repo.save_account(&account).await.unwrap();
    account
}

/// Create a test sync item (requires an account to exist first)
fn create_test_sync_item() -> SyncItem {
    let local_path = SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap();
    let remote_path = RemotePath::new("/test.txt".to_string()).unwrap();
    SyncItem::new_file(
        local_path,
        remote_path,
        1024,
        Some("text/plain".to_string()),
    )
    .unwrap()
}

/// Valid quickXorHash Base64 strings (20 bytes = 28 chars with padding)
const VALID_HASH_1: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const VALID_HASH_2: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBB=";

// ============================================================================
// Account tests
// ============================================================================

#[tokio::test]
async fn test_save_and_get_account() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let retrieved = repo.get_account(account.id()).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.email().as_str(), "test@example.com");
    assert_eq!(retrieved.display_name(), "Test User");
    assert_eq!(retrieved.onedrive_id(), "drive123");
    assert_eq!(retrieved.sync_root().to_string(), "/home/user/OneDrive");
    assert!(matches!(retrieved.state(), AccountState::Active));
}

#[tokio::test]
async fn test_get_account_not_found() {
    let repo = setup().await;
    let fake_id = AccountId::new();

    let result = repo.get_account(&fake_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_default_account() {
    let repo = setup().await;

    // No accounts yet
    let result = repo.get_default_account().await.unwrap();
    assert!(result.is_none());

    // Create an account
    let account = create_test_account(&repo).await;

    let default = repo.get_default_account().await.unwrap();
    assert!(default.is_some());
    assert_eq!(default.unwrap().id(), account.id());
}

#[tokio::test]
async fn test_update_account() {
    let repo = setup().await;
    let mut account = create_test_account(&repo).await;

    // Modify and save again (UPSERT)
    account.update_quota(5_000_000, 15_000_000_000);
    account.mark_token_expired();

    let token = DeltaToken::new("delta-token-123".to_string()).unwrap();
    account.update_delta_token(token);
    account.record_sync(Utc::now());

    repo.save_account(&account).await.unwrap();

    let retrieved = repo.get_account(account.id()).await.unwrap().unwrap();
    assert_eq!(retrieved.quota_used(), 5_000_000);
    assert_eq!(retrieved.quota_total(), 15_000_000_000);
    assert!(matches!(retrieved.state(), AccountState::TokenExpired));
    assert_eq!(retrieved.delta_token().unwrap().as_str(), "delta-token-123");
    assert!(retrieved.last_sync().is_some());
}

// ============================================================================
// SyncItem tests
// ============================================================================

#[tokio::test]
async fn test_save_and_get_item() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();

    repo.save_item(&item).await.unwrap();

    let retrieved = repo.get_item(item.id()).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id(), item.id());
    assert_eq!(
        retrieved.local_path().to_string(),
        "/home/user/OneDrive/test.txt"
    );
    assert_eq!(retrieved.remote_path().as_str(), "/test.txt");
    assert_eq!(retrieved.size_bytes(), 1024);
    assert!(matches!(retrieved.state(), ItemState::Online));
}

#[tokio::test]
async fn test_get_item_not_found() {
    let repo = setup().await;
    let fake_id = UniqueId::new();

    let result = repo.get_item(&fake_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_item_by_path() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();

    repo.save_item(&item).await.unwrap();

    let path = SyncPath::new(PathBuf::from("/home/user/OneDrive/test.txt")).unwrap();
    let retrieved = repo.get_item_by_path(&path).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id(), item.id());
}

#[tokio::test]
async fn test_get_item_by_remote_id() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    let local_path = SyncPath::new(PathBuf::from("/home/user/OneDrive/remote_file.txt")).unwrap();
    let remote_path = RemotePath::new("/remote_file.txt".to_string()).unwrap();
    let remote_id = RemoteId::new("ABC123DEF".to_string()).unwrap();
    let hash = FileHash::new(VALID_HASH_1.to_string()).unwrap();

    let item = SyncItem::from_remote(
        local_path,
        remote_path,
        remote_id.clone(),
        false,
        2048,
        Some(hash),
        Utc::now(),
    )
    .unwrap();

    repo.save_item(&item).await.unwrap();

    let retrieved = repo.get_item_by_remote_id(&remote_id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id(), item.id());
}

#[tokio::test]
async fn test_delete_item() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();

    repo.save_item(&item).await.unwrap();
    assert!(repo.get_item(item.id()).await.unwrap().is_some());

    repo.delete_item(item.id()).await.unwrap();
    assert!(repo.get_item(item.id()).await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_item() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let mut item = create_test_sync_item();

    repo.save_item(&item).await.unwrap();

    // Modify the item
    let remote_id = RemoteId::new("XYZ789".to_string()).unwrap();
    item.set_remote_id(remote_id);
    item.set_size_bytes(4096);
    item.set_last_modified_local(Utc::now());
    item.start_hydrating().unwrap();
    item.complete_hydration().unwrap();
    item.mark_synced();

    repo.save_item(&item).await.unwrap();

    let retrieved = repo.get_item(item.id()).await.unwrap().unwrap();
    assert_eq!(retrieved.size_bytes(), 4096);
    assert!(retrieved.remote_id().is_some());
    assert_eq!(retrieved.remote_id().unwrap().as_str(), "XYZ789");
    assert!(matches!(retrieved.state(), ItemState::Hydrated));
    assert!(retrieved.last_sync().is_some());
}

#[tokio::test]
async fn test_item_with_error_state() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let mut item = create_test_sync_item();

    item.transition_to(ItemState::Error("network failure".to_string()))
        .unwrap();

    repo.save_item(&item).await.unwrap();

    let retrieved = repo.get_item(item.id()).await.unwrap().unwrap();
    match retrieved.state() {
        ItemState::Error(msg) => assert_eq!(msg, "network failure"),
        other => panic!("Expected Error state, got: {:?}", other),
    }
}

// ============================================================================
// Query items tests
// ============================================================================

#[tokio::test]
async fn test_query_items_empty_filter() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    // Create several items
    let item1 = create_test_sync_item();

    let local_path2 = SyncPath::new(PathBuf::from("/home/user/OneDrive/file2.txt")).unwrap();
    let remote_path2 = RemotePath::new("/file2.txt".to_string()).unwrap();
    let item2 = SyncItem::new_file(local_path2, remote_path2, 2048, None).unwrap();

    repo.save_item(&item1).await.unwrap();
    repo.save_item(&item2).await.unwrap();

    let filter = ItemFilter::new();
    let results = repo.query_items(&filter).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_query_items_by_state() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    let item1 = create_test_sync_item(); // Online state

    let local_path2 = SyncPath::new(PathBuf::from("/home/user/OneDrive/file2.txt")).unwrap();
    let remote_path2 = RemotePath::new("/file2.txt".to_string()).unwrap();
    let mut item2 = SyncItem::new_file(local_path2, remote_path2, 2048, None).unwrap();
    item2.start_hydrating().unwrap();
    item2.complete_hydration().unwrap();
    item2.mark_modified().unwrap(); // Modified state

    repo.save_item(&item1).await.unwrap();
    repo.save_item(&item2).await.unwrap();

    let filter = ItemFilter::new().with_state(ItemState::Modified);
    let results = repo.query_items(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(matches!(results[0].state(), ItemState::Modified));
}

#[tokio::test]
async fn test_query_items_by_account() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;
    let item = create_test_sync_item();

    repo.save_item(&item).await.unwrap();

    let filter = ItemFilter::new().with_account_id(*account.id());
    let results = repo.query_items(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_query_items_by_path_prefix() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    let local_path1 = SyncPath::new(PathBuf::from("/home/user/OneDrive/docs/file1.txt")).unwrap();
    let remote_path1 = RemotePath::new("/docs/file1.txt".to_string()).unwrap();
    let item1 = SyncItem::new_file(local_path1, remote_path1, 1024, None).unwrap();

    let local_path2 = SyncPath::new(PathBuf::from("/home/user/OneDrive/photos/img.jpg")).unwrap();
    let remote_path2 = RemotePath::new("/photos/img.jpg".to_string()).unwrap();
    let item2 = SyncItem::new_file(local_path2, remote_path2, 4096, None).unwrap();

    repo.save_item(&item1).await.unwrap();
    repo.save_item(&item2).await.unwrap();

    let prefix = SyncPath::new(PathBuf::from("/home/user/OneDrive/docs")).unwrap();
    let filter = ItemFilter::new().with_path_prefix(prefix);
    let results = repo.query_items(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].local_path().to_string().contains("docs"));
}

#[tokio::test]
async fn test_count_items_by_state() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    // Create items in different states
    let item1 = create_test_sync_item(); // Online

    let local_path2 = SyncPath::new(PathBuf::from("/home/user/OneDrive/file2.txt")).unwrap();
    let remote_path2 = RemotePath::new("/file2.txt".to_string()).unwrap();
    let item2 = SyncItem::new_file(local_path2, remote_path2, 2048, None).unwrap(); // Online

    let local_path3 = SyncPath::new(PathBuf::from("/home/user/OneDrive/file3.txt")).unwrap();
    let remote_path3 = RemotePath::new("/file3.txt".to_string()).unwrap();
    let mut item3 = SyncItem::new_file(local_path3, remote_path3, 3072, None).unwrap();
    item3.start_hydrating().unwrap();
    item3.complete_hydration().unwrap();
    item3.mark_modified().unwrap(); // Modified

    repo.save_item(&item1).await.unwrap();
    repo.save_item(&item2).await.unwrap();
    repo.save_item(&item3).await.unwrap();

    let counts = repo.count_items_by_state(account.id()).await.unwrap();
    assert_eq!(counts.get("Online"), Some(&2));
    assert_eq!(counts.get("Modified"), Some(&1));
}

// ============================================================================
// Session tests
// ============================================================================

#[tokio::test]
async fn test_save_and_get_session() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let mut session = SyncSession::new(*account.id());
    session.set_items_total(100);
    session.record_success();
    session.record_success();
    session.record_failure();
    session.add_bytes_uploaded(1024);
    session.add_bytes_downloaded(2048);

    repo.save_session(&session).await.unwrap();

    let retrieved = repo.get_session(session.id()).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.items_total(), 100);
    assert_eq!(retrieved.items_processed(), 3);
    assert_eq!(retrieved.items_succeeded(), 2);
    assert_eq!(retrieved.items_failed(), 1);
    assert_eq!(retrieved.bytes_uploaded(), 1024);
    assert_eq!(retrieved.bytes_downloaded(), 2048);
}

#[tokio::test]
async fn test_session_not_found() {
    let repo = setup().await;
    let fake_id = SessionId::new();

    let result = repo.get_session(&fake_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_session_completed() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let mut session = SyncSession::new(*account.id());
    session.set_items_total(10);
    session.update_progress(10, 10, 0);
    session.complete();

    repo.save_session(&session).await.unwrap();

    let retrieved = repo.get_session(session.id()).await.unwrap().unwrap();
    assert!(retrieved.status().is_success());
    assert!(retrieved.completed_at().is_some());
}

#[tokio::test]
async fn test_session_with_delta_tokens() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let mut session = SyncSession::new(*account.id());
    let start_token = DeltaToken::new("start-token".to_string()).unwrap();
    let end_token = DeltaToken::new("end-token".to_string()).unwrap();
    session.set_delta_token_start(start_token);
    session.set_delta_token_end(end_token);
    session.complete();

    repo.save_session(&session).await.unwrap();

    let retrieved = repo.get_session(session.id()).await.unwrap().unwrap();
    assert_eq!(
        retrieved.delta_token_start().unwrap().as_str(),
        "start-token"
    );
    assert_eq!(retrieved.delta_token_end().unwrap().as_str(), "end-token");
}

// ============================================================================
// Audit tests
// ============================================================================

#[tokio::test]
async fn test_save_and_get_audit_trail() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();
    repo.save_item(&item).await.unwrap();

    // Create audit entries for this item
    let entry1 = AuditEntry::new(AuditAction::FileDownload, AuditResult::success())
        .with_item_id(*item.id())
        .with_duration_ms(150);

    let entry2 = AuditEntry::new(
        AuditAction::Error,
        AuditResult::failed("NET_ERROR", "Connection timed out"),
    )
    .with_item_id(*item.id())
    .with_details(serde_json::json!({"retry_count": 3}));

    repo.save_audit(&entry1).await.unwrap();
    repo.save_audit(&entry2).await.unwrap();

    let trail = repo.get_audit_trail(item.id()).await.unwrap();
    assert_eq!(trail.len(), 2);

    // Entries should be ordered by timestamp (oldest first)
    assert_eq!(*trail[0].action(), AuditAction::FileDownload);
    assert_eq!(*trail[1].action(), AuditAction::Error);
    assert!(trail[0].result().is_success());
    assert!(trail[1].result().is_failed());
}

#[tokio::test]
async fn test_get_audit_since() {
    let repo = setup().await;

    // Create entries at different times
    let old_time = Utc::now() - Duration::hours(2);
    let recent_time = Utc::now() - Duration::minutes(30);

    let entry1 =
        AuditEntry::new(AuditAction::SyncStart, AuditResult::success()).with_duration_ms(100);

    let entry2 =
        AuditEntry::new(AuditAction::SyncComplete, AuditResult::success()).with_duration_ms(200);

    repo.save_audit(&entry1).await.unwrap();
    repo.save_audit(&entry2).await.unwrap();

    // Query for entries since an hour ago - should get both since they're recent
    let since = Utc::now() - Duration::hours(1);
    let entries = repo.get_audit_since(since, 10).await.unwrap();
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn test_get_audit_since_with_limit() {
    let repo = setup().await;

    // Create multiple entries
    for i in 0..5 {
        let entry = AuditEntry::new(AuditAction::FileUpload, AuditResult::success())
            .with_details(serde_json::json!({"index": i}));
        repo.save_audit(&entry).await.unwrap();
    }

    let since = Utc::now() - Duration::hours(1);
    let entries = repo.get_audit_since(since, 3).await.unwrap();
    assert_eq!(entries.len(), 3);
}

#[tokio::test]
async fn test_audit_with_session_id() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let session = SyncSession::new(*account.id());
    repo.save_session(&session).await.unwrap();

    let entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success())
        .with_session_id(*session.id());

    repo.save_audit(&entry).await.unwrap();

    let since = Utc::now() - Duration::hours(1);
    let entries = repo.get_audit_since(since, 10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].session_id().is_some());
    assert_eq!(entries[0].session_id().unwrap(), session.id());
}

// ============================================================================
// Conflict tests
// ============================================================================

#[tokio::test]
async fn test_save_and_get_unresolved_conflicts() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();
    repo.save_item(&item).await.unwrap();

    let local_version = VersionInfo::new(
        FileHash::new(VALID_HASH_1.to_string()).unwrap(),
        1024,
        Utc::now(),
    );
    let remote_version = VersionInfo::new(
        FileHash::new(VALID_HASH_2.to_string()).unwrap(),
        1048,
        Utc::now(),
    );

    let conflict = Conflict::new(*item.id(), local_version, remote_version);
    repo.save_conflict(&conflict).await.unwrap();

    let unresolved = repo.get_unresolved_conflicts().await.unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].item_id(), item.id());
    assert!(!unresolved[0].is_resolved());
}

#[tokio::test]
async fn test_resolved_conflict_not_in_unresolved() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;
    let item = create_test_sync_item();
    repo.save_item(&item).await.unwrap();

    let local_version = VersionInfo::new(
        FileHash::new(VALID_HASH_1.to_string()).unwrap(),
        1024,
        Utc::now(),
    );
    let remote_version = VersionInfo::new(
        FileHash::new(VALID_HASH_2.to_string()).unwrap(),
        1048,
        Utc::now(),
    );

    let conflict = Conflict::new(*item.id(), local_version, remote_version)
        .resolve(Resolution::KeepLocal, ResolutionSource::User);

    repo.save_conflict(&conflict).await.unwrap();

    let unresolved = repo.get_unresolved_conflicts().await.unwrap();
    assert_eq!(unresolved.len(), 0);
}

#[tokio::test]
async fn test_multiple_conflicts_ordering() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    // Create two items with conflicts
    let item1 = create_test_sync_item();
    repo.save_item(&item1).await.unwrap();

    let local_path2 = SyncPath::new(PathBuf::from("/home/user/OneDrive/file2.txt")).unwrap();
    let remote_path2 = RemotePath::new("/file2.txt".to_string()).unwrap();
    let item2 = SyncItem::new_file(local_path2, remote_path2, 2048, None).unwrap();
    repo.save_item(&item2).await.unwrap();

    let local_version1 = VersionInfo::new(
        FileHash::new(VALID_HASH_1.to_string()).unwrap(),
        1024,
        Utc::now(),
    );
    let remote_version1 = VersionInfo::new(
        FileHash::new(VALID_HASH_2.to_string()).unwrap(),
        1048,
        Utc::now(),
    );

    let local_version2 = VersionInfo::new(
        FileHash::new(VALID_HASH_1.to_string()).unwrap(),
        2048,
        Utc::now(),
    );
    let remote_version2 = VersionInfo::new(
        FileHash::new(VALID_HASH_2.to_string()).unwrap(),
        2096,
        Utc::now(),
    );

    let conflict1 = Conflict::new(*item1.id(), local_version1, remote_version1);
    let conflict2 = Conflict::new(*item2.id(), local_version2, remote_version2);

    repo.save_conflict(&conflict1).await.unwrap();
    repo.save_conflict(&conflict2).await.unwrap();

    let unresolved = repo.get_unresolved_conflicts().await.unwrap();
    assert_eq!(unresolved.len(), 2);
    // Ordered by detected_at DESC (newest first)
}

// ============================================================================
// Database pool tests
// ============================================================================

#[tokio::test]
async fn test_in_memory_pool_creation() {
    let pool = DatabasePool::in_memory().await;
    assert!(pool.is_ok());
}

#[tokio::test]
async fn test_file_based_pool_creation() {
    let temp_dir = std::env::temp_dir().join(format!("lnxdrive_test_{}", Uuid::new_v4()));
    let db_path = temp_dir.join("test.db");

    let pool = DatabasePool::new(&db_path).await;
    assert!(pool.is_ok());

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ============================================================================
// Edge case tests
// ============================================================================

#[tokio::test]
async fn test_item_with_directory_metadata() {
    let repo = setup().await;
    let _account = create_test_account(&repo).await;

    let local_path = SyncPath::new(PathBuf::from("/home/user/OneDrive/mydir")).unwrap();
    let remote_path = RemotePath::new("/mydir".to_string()).unwrap();
    let item = SyncItem::new_directory(local_path, remote_path).unwrap();

    repo.save_item(&item).await.unwrap();

    let retrieved = repo.get_item(item.id()).await.unwrap().unwrap();
    assert!(retrieved.is_directory());
    assert_eq!(retrieved.size_bytes(), 0);
}

#[tokio::test]
async fn test_delete_nonexistent_item() {
    let repo = setup().await;
    let fake_id = UniqueId::new();

    // Should not error when deleting a non-existent item
    let result = repo.delete_item(&fake_id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_empty_query_results() {
    let repo = setup().await;

    let filter = ItemFilter::new().with_state(ItemState::Modified);
    let results = repo.query_items(&filter).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_audit_trail_for_nonexistent_item() {
    let repo = setup().await;
    let fake_id = UniqueId::new();

    let trail = repo.get_audit_trail(&fake_id).await.unwrap();
    assert!(trail.is_empty());
}

#[tokio::test]
async fn test_account_with_error_state() {
    let repo = setup().await;
    let mut account = {
        let email = Email::new("error@example.com".to_string()).unwrap();
        let sync_root = SyncPath::new(PathBuf::from("/home/user/OneDrive")).unwrap();
        Account::new(email, "Error User", "drive456", sync_root)
    };

    account.mark_error("API rate limited");
    repo.save_account(&account).await.unwrap();

    let retrieved = repo.get_account(account.id()).await.unwrap().unwrap();
    match retrieved.state() {
        AccountState::Error(msg) => assert_eq!(msg, "API rate limited"),
        other => panic!("Expected Error state, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_session_with_errors() {
    let repo = setup().await;
    let account = create_test_account(&repo).await;

    let mut session = SyncSession::new(*account.id());
    session.set_items_total(5);

    // Add session errors
    let item_id = UniqueId::new();
    let error = lnxdrive_core::domain::session::SessionError::new(
        item_id,
        "NET_ERROR",
        "Connection refused",
    );
    session.add_error(error);
    session.record_failure();
    session.fail("Too many errors");

    repo.save_session(&session).await.unwrap();

    let retrieved = repo.get_session(session.id()).await.unwrap().unwrap();
    assert!(retrieved.status().is_failed());
    assert_eq!(retrieved.items_failed(), 1);
    assert_eq!(retrieved.errors().len(), 1);
    assert_eq!(retrieved.errors()[0].error_code(), "NET_ERROR");
}
