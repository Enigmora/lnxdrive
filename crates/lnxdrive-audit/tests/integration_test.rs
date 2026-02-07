//! Integration test: AuditLogger → SQLite → query back
//!
//! Uses a real in-memory SQLite database to verify the full flow:
//! AuditLogger creates entries → IStateRepository persists them →
//! get_audit_since returns them.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Duration, Utc};
use lnxdrive_audit::AuditLogger;
use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
use lnxdrive_core::{
    domain::{
        newtypes::{Email, SyncPath, UniqueId},
        Account, SyncSession,
    },
    ports::state_repository::IStateRepository,
};

async fn make_repo() -> Arc<SqliteStateRepository> {
    let pool = DatabasePool::in_memory()
        .await
        .expect("Failed to create in-memory database");
    Arc::new(SqliteStateRepository::new(pool.pool().clone()))
}

#[tokio::test]
async fn test_audit_logger_integration_with_sqlite() {
    let repo = make_repo().await;
    let logger = AuditLogger::new(Arc::clone(&repo) as Arc<dyn IStateRepository>);

    // Satisfy FK chain: accounts → sync_sessions → audit_log
    let email = Email::new("test@example.com".to_string()).unwrap();
    let sync_root = SyncPath::new(PathBuf::from("/tmp/test-sync")).unwrap();
    let account = Account::new(email, "Test User", "drive-123", sync_root);
    let account_id = *account.id();
    repo.save_account(&account).await.unwrap();

    let session = SyncSession::new(account_id);
    let session_id = *session.id();
    repo.save_session(&session).await.unwrap();

    let item_id = UniqueId::new();

    // Log sync start, file download, and sync complete
    logger.log_sync_start(session_id).await;
    logger
        .log_file_download(item_id, "/documents/test.pdf", 4096, 150)
        .await;
    logger
        .log_sync_complete(session_id, 500, 1, 0, 0, 0)
        .await;

    // Query back
    let since = Utc::now() - Duration::minutes(5);
    let entries = repo.get_audit_since(since, 50).await.unwrap();

    assert_eq!(
        entries.len(),
        3,
        "Expected 3 audit entries, got {}",
        entries.len()
    );

    // Entries are returned newest-first, so: SyncComplete, FileDownload, SyncStart
    let actions: Vec<String> = entries.iter().map(|e| e.action().to_string()).collect();
    assert!(
        actions.contains(&"sync_start".to_string()),
        "Missing sync_start"
    );
    assert!(
        actions.contains(&"file_download".to_string()),
        "Missing file_download"
    );
    assert!(
        actions.contains(&"sync_complete".to_string()),
        "Missing sync_complete"
    );
}
