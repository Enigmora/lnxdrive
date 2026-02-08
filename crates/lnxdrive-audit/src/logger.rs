//! AuditLogger - high-level audit logging service
//!
//! Wraps `IStateRepository::save_audit()` with convenience methods for
//! each type of auditable operation. All methods are non-fatal: errors
//! in audit persistence are logged via `tracing::warn!` but never propagated.

use std::sync::Arc;

use chrono::Utc;
use lnxdrive_core::{
    domain::{
        audit::{AuditAction, AuditEntry, AuditResult},
        newtypes::{SessionId, UniqueId},
    },
    ports::state_repository::IStateRepository,
};
use serde_json::json;

/// High-level audit logger that wraps the state repository's audit persistence.
///
/// All methods silently swallow errors (logging a warning) to ensure
/// audit failures never break sync operations.
pub struct AuditLogger {
    state_repo: Arc<dyn IStateRepository>,
}

impl AuditLogger {
    /// Creates a new `AuditLogger` backed by the given state repository.
    pub fn new(state_repo: Arc<dyn IStateRepository>) -> Self {
        Self { state_repo }
    }

    /// Persist an audit entry, swallowing errors with a tracing warning.
    async fn save(&self, entry: &AuditEntry) {
        if let Err(e) = self.state_repo.save_audit(entry).await {
            tracing::warn!(error = %e, "Failed to save audit entry");
        }
    }

    // ========================================================================
    // Sync lifecycle
    // ========================================================================

    /// Log the start of a sync cycle.
    pub async fn log_sync_start(&self, session_id: SessionId) {
        let entry = AuditEntry::new(AuditAction::SyncStart, AuditResult::success())
            .with_session_id(session_id);
        self.save(&entry).await;
    }

    /// Log the successful completion of a sync cycle.
    pub async fn log_sync_complete(
        &self,
        session_id: SessionId,
        duration_ms: u64,
        downloaded: u32,
        uploaded: u32,
        deleted: u32,
        errors: usize,
    ) {
        let entry = AuditEntry::new(AuditAction::SyncComplete, AuditResult::success())
            .with_session_id(session_id)
            .with_duration_ms(duration_ms)
            .with_details(json!({
                "files_downloaded": downloaded,
                "files_uploaded": uploaded,
                "files_deleted": deleted,
                "errors": errors,
            }));
        self.save(&entry).await;
    }

    // ========================================================================
    // File operations
    // ========================================================================

    /// Log a file download from the cloud.
    pub async fn log_file_download(
        &self,
        item_id: UniqueId,
        path: &str,
        size_bytes: u64,
        duration_ms: u64,
    ) {
        let entry = AuditEntry::new(AuditAction::FileDownload, AuditResult::success())
            .with_item_id(item_id)
            .with_duration_ms(duration_ms)
            .with_details(json!({
                "path": path,
                "size_bytes": size_bytes,
            }));
        self.save(&entry).await;
    }

    /// Log a file upload to the cloud.
    pub async fn log_file_upload(
        &self,
        item_id: UniqueId,
        path: &str,
        size_bytes: u64,
        duration_ms: u64,
    ) {
        let entry = AuditEntry::new(AuditAction::FileUpload, AuditResult::success())
            .with_item_id(item_id)
            .with_duration_ms(duration_ms)
            .with_details(json!({
                "path": path,
                "size_bytes": size_bytes,
            }));
        self.save(&entry).await;
    }

    /// Log a file deletion (local or remote).
    pub async fn log_file_delete(&self, item_id: UniqueId, path: &str) {
        let entry = AuditEntry::new(AuditAction::FileDelete, AuditResult::success())
            .with_item_id(item_id)
            .with_details(json!({
                "path": path,
            }));
        self.save(&entry).await;
    }

    // ========================================================================
    // Conflicts and errors
    // ========================================================================

    /// Log detection of a sync conflict.
    pub async fn log_conflict_detected(&self, item_id: UniqueId, path: &str, reason: &str) {
        let entry = AuditEntry::new(AuditAction::ConflictDetected, AuditResult::success())
            .with_item_id(item_id)
            .with_details(json!({
                "path": path,
                "reason": reason,
            }));
        self.save(&entry).await;
    }

    /// Log resolution of a sync conflict.
    pub async fn log_conflict_resolved(
        &self,
        item_id: UniqueId,
        path: &str,
        resolution: &str,
    ) {
        let entry = AuditEntry::new(AuditAction::ConflictResolved, AuditResult::success())
            .with_item_id(item_id)
            .with_details(json!({
                "path": path,
                "resolution": resolution,
            }));
        self.save(&entry).await;
    }

    /// Log a non-fatal error during sync.
    pub async fn log_error(&self, message: &str, context: Option<&str>) {
        let result = AuditResult::failed("SYNC_ERROR", message);
        let mut entry = AuditEntry::new(AuditAction::Error, result);
        if let Some(ctx) = context {
            entry = entry.with_details(json!({
                "context": ctx,
                "timestamp": Utc::now().to_rfc3339(),
            }));
        }
        self.save(&entry).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use chrono::DateTime;
    use lnxdrive_core::{
        domain::{
            newtypes::{AccountId, ConflictId, RemoteId, SyncPath},
            Account, AuditEntry, Conflict, SyncItem, SyncSession,
        },
        ports::state_repository::ItemFilter,
    };

    /// In-memory mock repository that records saved audit entries
    struct MockRepo {
        entries: Mutex<Vec<AuditEntry>>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                entries: Mutex::new(Vec::new()),
            }
        }

        fn entries(&self) -> Vec<AuditEntry> {
            self.entries.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl IStateRepository for MockRepo {
        async fn save_item(&self, _item: &SyncItem) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_item(&self, _id: &UniqueId) -> anyhow::Result<Option<SyncItem>> {
            Ok(None)
        }
        async fn get_item_by_path(&self, _p: &SyncPath) -> anyhow::Result<Option<SyncItem>> {
            Ok(None)
        }
        async fn get_item_by_remote_id(
            &self,
            _r: &RemoteId,
        ) -> anyhow::Result<Option<SyncItem>> {
            Ok(None)
        }
        async fn query_items(&self, _f: &ItemFilter) -> anyhow::Result<Vec<SyncItem>> {
            Ok(vec![])
        }
        async fn delete_item(&self, _id: &UniqueId) -> anyhow::Result<()> {
            Ok(())
        }
        async fn count_items_by_state(
            &self,
            _a: &AccountId,
        ) -> anyhow::Result<HashMap<String, u64>> {
            Ok(HashMap::new())
        }
        async fn save_account(&self, _a: &Account) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_account(&self, _id: &AccountId) -> anyhow::Result<Option<Account>> {
            Ok(None)
        }
        async fn get_default_account(&self) -> anyhow::Result<Option<Account>> {
            Ok(None)
        }
        async fn save_session(&self, _s: &SyncSession) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_session(&self, _id: &SessionId) -> anyhow::Result<Option<SyncSession>> {
            Ok(None)
        }
        async fn save_audit(&self, entry: &AuditEntry) -> anyhow::Result<()> {
            self.entries.lock().unwrap().push(entry.clone());
            Ok(())
        }
        async fn get_audit_trail(&self, _id: &UniqueId) -> anyhow::Result<Vec<AuditEntry>> {
            Ok(vec![])
        }
        async fn get_audit_since(
            &self,
            _since: DateTime<Utc>,
            _limit: u32,
        ) -> anyhow::Result<Vec<AuditEntry>> {
            Ok(vec![])
        }
        async fn save_conflict(&self, _c: &Conflict) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_unresolved_conflicts(&self) -> anyhow::Result<Vec<Conflict>> {
            Ok(vec![])
        }
        async fn get_conflict_by_id(
            &self,
            _id: &ConflictId,
        ) -> anyhow::Result<Option<Conflict>> {
            Ok(None)
        }
        async fn get_next_inode(&self) -> anyhow::Result<u64> {
            Ok(2)
        }
        async fn update_inode(&self, _id: &UniqueId, _inode: u64) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_item_by_inode(&self, _inode: u64) -> anyhow::Result<Option<SyncItem>> {
            Ok(None)
        }
        async fn update_last_accessed(
            &self,
            _id: &UniqueId,
            _accessed: DateTime<Utc>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn update_hydration_progress(
            &self,
            _id: &UniqueId,
            _progress: Option<u8>,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_items_for_dehydration(
            &self,
            _max_age_days: u32,
            _limit: u32,
        ) -> anyhow::Result<Vec<SyncItem>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_log_sync_start() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let sid = SessionId::new();

        logger.log_sync_start(sid).await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::SyncStart);
        assert_eq!(entries[0].session_id(), Some(&sid));
    }

    #[tokio::test]
    async fn test_log_sync_complete() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let sid = SessionId::new();

        logger.log_sync_complete(sid, 1500, 3, 2, 1, 0).await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::SyncComplete);
        assert_eq!(entries[0].duration_ms(), Some(1500));
        assert_eq!(entries[0].details()["files_downloaded"], 3);
    }

    #[tokio::test]
    async fn test_log_file_download() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let item_id = UniqueId::new();

        logger
            .log_file_download(item_id, "/docs/file.txt", 4096, 200)
            .await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::FileDownload);
        assert_eq!(entries[0].item_id(), Some(&item_id));
        assert_eq!(entries[0].details()["path"], "/docs/file.txt");
    }

    #[tokio::test]
    async fn test_log_file_upload() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let item_id = UniqueId::new();

        logger
            .log_file_upload(item_id, "/photos/img.jpg", 2048000, 500)
            .await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::FileUpload);
    }

    #[tokio::test]
    async fn test_log_file_delete() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let item_id = UniqueId::new();

        logger.log_file_delete(item_id, "/old/file.bak").await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::FileDelete);
    }

    #[tokio::test]
    async fn test_log_conflict_detected() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let item_id = UniqueId::new();

        logger
            .log_conflict_detected(item_id, "/doc.txt", "both_modified")
            .await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::ConflictDetected);
        assert_eq!(entries[0].details()["reason"], "both_modified");
    }

    #[tokio::test]
    async fn test_log_conflict_resolved() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());
        let item_id = UniqueId::new();

        logger
            .log_conflict_resolved(item_id, "/doc.txt", "keep_local")
            .await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::ConflictResolved);
    }

    #[tokio::test]
    async fn test_log_error() {
        let repo = Arc::new(MockRepo::new());
        let logger = AuditLogger::new(repo.clone());

        logger
            .log_error("Connection timed out", Some("upload"))
            .await;

        let entries = repo.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].action(), AuditAction::Error);
        assert!(entries[0].result().is_failed());
    }

    #[tokio::test]
    async fn test_audit_failure_is_non_fatal() {
        // A repository that always fails on save_audit
        struct FailingRepo;

        #[async_trait]
        impl IStateRepository for FailingRepo {
            async fn save_item(&self, _: &SyncItem) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_item(&self, _: &UniqueId) -> anyhow::Result<Option<SyncItem>> {
                Ok(None)
            }
            async fn get_item_by_path(&self, _: &SyncPath) -> anyhow::Result<Option<SyncItem>> {
                Ok(None)
            }
            async fn get_item_by_remote_id(
                &self,
                _: &RemoteId,
            ) -> anyhow::Result<Option<SyncItem>> {
                Ok(None)
            }
            async fn query_items(&self, _: &ItemFilter) -> anyhow::Result<Vec<SyncItem>> {
                Ok(vec![])
            }
            async fn delete_item(&self, _: &UniqueId) -> anyhow::Result<()> {
                Ok(())
            }
            async fn count_items_by_state(
                &self,
                _: &AccountId,
            ) -> anyhow::Result<HashMap<String, u64>> {
                Ok(HashMap::new())
            }
            async fn save_account(&self, _: &Account) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_account(&self, _: &AccountId) -> anyhow::Result<Option<Account>> {
                Ok(None)
            }
            async fn get_default_account(&self) -> anyhow::Result<Option<Account>> {
                Ok(None)
            }
            async fn save_session(&self, _: &SyncSession) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_session(&self, _: &SessionId) -> anyhow::Result<Option<SyncSession>> {
                Ok(None)
            }
            async fn save_audit(&self, _: &AuditEntry) -> anyhow::Result<()> {
                anyhow::bail!("Database write error")
            }
            async fn get_audit_trail(&self, _: &UniqueId) -> anyhow::Result<Vec<AuditEntry>> {
                Ok(vec![])
            }
            async fn get_audit_since(
                &self,
                _: DateTime<Utc>,
                _: u32,
            ) -> anyhow::Result<Vec<AuditEntry>> {
                Ok(vec![])
            }
            async fn save_conflict(&self, _: &Conflict) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_unresolved_conflicts(&self) -> anyhow::Result<Vec<Conflict>> {
                Ok(vec![])
            }
            async fn get_conflict_by_id(&self, _: &ConflictId) -> anyhow::Result<Option<Conflict>> {
                Ok(None)
            }
            async fn get_next_inode(&self) -> anyhow::Result<u64> {
                Ok(2)
            }
            async fn update_inode(&self, _: &UniqueId, _: u64) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_item_by_inode(&self, _: u64) -> anyhow::Result<Option<SyncItem>> {
                Ok(None)
            }
            async fn update_last_accessed(
                &self,
                _: &UniqueId,
                _: DateTime<Utc>,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn update_hydration_progress(
                &self,
                _: &UniqueId,
                _: Option<u8>,
            ) -> anyhow::Result<()> {
                Ok(())
            }
            async fn get_items_for_dehydration(
                &self,
                _: u32,
                _: u32,
            ) -> anyhow::Result<Vec<SyncItem>> {
                Ok(vec![])
            }
        }

        let repo = Arc::new(FailingRepo);
        let logger = AuditLogger::new(repo);

        // This should NOT panic or return an error
        logger.log_sync_start(SessionId::new()).await;
        logger
            .log_file_download(UniqueId::new(), "/test", 0, 0)
            .await;
        logger.log_error("test error", None).await;
    }
}
