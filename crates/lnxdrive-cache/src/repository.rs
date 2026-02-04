//! SQLite implementation of IStateRepository
//!
//! This module provides the concrete SQLite-based implementation of the
//! state repository port defined in lnxdrive-core. It handles all domain
//! type serialization/deserialization and SQL query construction.
//!
//! ## Type Mapping
//!
//! | Domain Type         | SQL Type | Strategy                    |
//! |---------------------|----------|-----------------------------|
//! | UniqueId, AccountId | TEXT     | UUID string via `.to_string()` / `FromStr` |
//! | SyncPath            | TEXT     | Path string via `.to_string()` / `SyncPath::new()` |
//! | RemotePath          | TEXT     | String via `.as_str()` / `RemotePath::new()` |
//! | RemoteId            | TEXT     | String via `.as_str()` / `RemoteId::new()` |
//! | FileHash            | TEXT     | String via `.as_str()` / `FileHash::new()` |
//! | DeltaToken          | TEXT     | String via `.as_str()` / `DeltaToken::new()` |
//! | Email               | TEXT     | String via `.as_str()` / `Email::new()` |
//! | DateTime<Utc>       | TEXT     | ISO 8601 via `to_rfc3339()` / `DateTime::parse_from_rfc3339()` |
//! | ItemState           | TEXT     | serde_json serialization    |
//! | ItemMetadata        | TEXT     | serde_json serialization    |
//! | ErrorInfo           | TEXT     | serde_json serialization    |
//! | SessionError[]      | TEXT     | serde_json array            |
//! | VersionInfo         | TEXT     | serde_json serialization    |
//! | AuditAction         | TEXT     | serde_json serialization    |
//! | AuditResult         | TEXT     | serde_json serialization    |

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use lnxdrive_core::domain::{
    newtypes::{AccountId, ConflictId, DeltaToken, Email, RemoteId, SessionId, SyncPath, UniqueId},
    session::{SessionError, SessionStatus},
    sync_item::ItemState,
    Account, AccountState, AuditAction, AuditEntry, AuditResult, Conflict, Resolution,
    ResolutionSource, SyncItem, SyncSession, VersionInfo,
};
use lnxdrive_core::ports::{IStateRepository, ItemFilter};

use crate::CacheError;

/// SQLite-based implementation of the state repository port
///
/// Provides persistent storage for all domain entities using SQLite.
/// All operations are performed through a connection pool for concurrency.
pub struct SqliteStateRepository {
    pool: SqlitePool,
}

impl SqliteStateRepository {
    /// Creates a new repository instance with the given connection pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// ============================================================================
// Helper functions for type conversion
// ============================================================================

/// Serialize an ItemState to a string for storage
///
/// Simple states are stored as plain strings (e.g., "online", "hydrated").
/// The Error variant is stored as "error:<message>".
fn item_state_to_string(state: &ItemState) -> String {
    match state {
        ItemState::Online => "online".to_string(),
        ItemState::Hydrating => "hydrating".to_string(),
        ItemState::Hydrated => "hydrated".to_string(),
        ItemState::Modified => "modified".to_string(),
        ItemState::Conflicted => "conflicted".to_string(),
        ItemState::Deleted => "deleted".to_string(),
        ItemState::Error(msg) => format!("error:{}", msg),
    }
}

/// Deserialize an ItemState from its stored string representation
fn item_state_from_string(s: &str) -> Result<ItemState, CacheError> {
    match s {
        "online" => Ok(ItemState::Online),
        "hydrating" => Ok(ItemState::Hydrating),
        "hydrated" => Ok(ItemState::Hydrated),
        "modified" => Ok(ItemState::Modified),
        "conflicted" => Ok(ItemState::Conflicted),
        "deleted" => Ok(ItemState::Deleted),
        s if s.starts_with("error:") => Ok(ItemState::Error(s[6..].to_string())),
        other => Err(CacheError::SerializationError(format!(
            "Unknown item state: {}",
            other
        ))),
    }
}

/// Serialize an AccountState to a string for storage
fn account_state_to_string(state: &AccountState) -> String {
    match state {
        AccountState::Active => "active".to_string(),
        AccountState::TokenExpired => "token_expired".to_string(),
        AccountState::Suspended => "suspended".to_string(),
        AccountState::Error(msg) => format!("error:{}", msg),
    }
}

/// Deserialize an AccountState from its stored string representation
fn account_state_from_string(s: &str) -> Result<AccountState, CacheError> {
    match s {
        "active" => Ok(AccountState::Active),
        "token_expired" => Ok(AccountState::TokenExpired),
        "suspended" => Ok(AccountState::Suspended),
        s if s.starts_with("error:") => Ok(AccountState::Error(s[6..].to_string())),
        other => Err(CacheError::SerializationError(format!(
            "Unknown account state: {}",
            other
        ))),
    }
}

/// Serialize a SessionStatus to a string for storage
fn session_status_to_string(status: &SessionStatus) -> String {
    match status {
        SessionStatus::Running => "running".to_string(),
        SessionStatus::Completed => "completed".to_string(),
        SessionStatus::Cancelled => "cancelled".to_string(),
        SessionStatus::Failed(msg) => format!("failed:{}", msg),
    }
}

/// Deserialize a SessionStatus from its stored string representation
fn session_status_from_string(s: &str) -> Result<SessionStatus, CacheError> {
    match s {
        "running" => Ok(SessionStatus::Running),
        "completed" => Ok(SessionStatus::Completed),
        "cancelled" => Ok(SessionStatus::Cancelled),
        s if s.starts_with("failed:") => Ok(SessionStatus::Failed(s[7..].to_string())),
        other => Err(CacheError::SerializationError(format!(
            "Unknown session status: {}",
            other
        ))),
    }
}

/// Parse a DateTime<Utc> from an ISO 8601 string
fn parse_datetime(s: &str) -> Result<DateTime<Utc>, CacheError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try parsing without timezone (SQLite default format)
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
                .map(|ndt| ndt.and_utc())
        })
        .map_err(|e| {
            CacheError::SerializationError(format!("Failed to parse datetime '{}': {}", s, e))
        })
}

/// Parse an optional DateTime<Utc> from an optional string
fn parse_optional_datetime(s: Option<String>) -> Result<Option<DateTime<Utc>>, CacheError> {
    match s {
        Some(ref val) if !val.is_empty() => parse_datetime(val).map(Some),
        _ => Ok(None),
    }
}

// ============================================================================
// Row mapping functions
// ============================================================================

/// Reconstruct a SyncItem from a database row
///
/// Uses serde JSON deserialization to reconstruct the SyncItem since
/// the struct has private fields that can only be set through constructors
/// or deserialization.
fn sync_item_from_row(row: &SqliteRow) -> Result<SyncItem, CacheError> {
    let id_str: String = row.get("id");
    let _account_id_str: String = row.get("account_id");
    let local_path_str: String = row.get("local_path");
    let remote_id_str: Option<String> = row.get("remote_id");
    let remote_path_str: String = row.get("remote_path");
    let state_str: String = row.get("state");
    let content_hash_str: Option<String> = row.get("content_hash");
    let local_hash_str: Option<String> = row.get("local_hash");
    let size_bytes: i64 = row.get("size_bytes");
    let last_sync_str: Option<String> = row.get("last_sync");
    let last_modified_local_str: Option<String> = row.get("last_modified_local");
    let last_modified_remote_str: Option<String> = row.get("last_modified_remote");
    let metadata_str: String = row.get("metadata");
    let error_info_str: Option<String> = row.get("error_info");

    // Parse the state string to the serde-compatible JSON representation
    let state = item_state_from_string(&state_str)?;
    let state_val: serde_json::Value = match &state {
        ItemState::Online => serde_json::Value::String("online".to_string()),
        ItemState::Hydrating => serde_json::Value::String("hydrating".to_string()),
        ItemState::Hydrated => serde_json::Value::String("hydrated".to_string()),
        ItemState::Modified => serde_json::Value::String("modified".to_string()),
        ItemState::Conflicted => serde_json::Value::String("conflicted".to_string()),
        ItemState::Deleted => serde_json::Value::String("deleted".to_string()),
        ItemState::Error(msg) => serde_json::json!({"error": msg}),
    };

    // Convert optional strings to serde Values
    let remote_id_val = match &remote_id_str {
        Some(rid) => serde_json::Value::String(rid.clone()),
        None => serde_json::Value::Null,
    };

    let content_hash_val = match &content_hash_str {
        Some(h) => serde_json::Value::String(h.clone()),
        None => serde_json::Value::Null,
    };

    let local_hash_val = match &local_hash_str {
        Some(h) => serde_json::Value::String(h.clone()),
        None => serde_json::Value::Null,
    };

    // Parse datetime fields
    let last_sync_val = match parse_optional_datetime(last_sync_str)? {
        Some(dt) => serde_json::Value::String(dt.to_rfc3339()),
        None => serde_json::Value::Null,
    };

    let last_modified_local_val = match parse_optional_datetime(last_modified_local_str)? {
        Some(dt) => serde_json::Value::String(dt.to_rfc3339()),
        None => serde_json::Value::Null,
    };

    let last_modified_remote_val = match parse_optional_datetime(last_modified_remote_str)? {
        Some(dt) => serde_json::Value::String(dt.to_rfc3339()),
        None => serde_json::Value::Null,
    };

    // Parse complex JSON fields
    let error_info_val: serde_json::Value = match error_info_str {
        Some(ref s) if !s.is_empty() => serde_json::from_str(s).unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::Null,
    };

    let metadata_val: serde_json::Value = serde_json::from_str(&metadata_str)
        .map_err(|e| CacheError::SerializationError(format!("Invalid metadata JSON: {}", e)))?;

    // Reconstruct via JSON deserialization for correct field mapping
    let item_json = serde_json::json!({
        "id": id_str,
        "local_path": local_path_str,
        "remote_id": remote_id_val,
        "remote_path": remote_path_str,
        "state": state_val,
        "content_hash": content_hash_val,
        "local_hash": local_hash_val,
        "size_bytes": size_bytes as u64,
        "last_sync": last_sync_val,
        "last_modified_local": last_modified_local_val,
        "last_modified_remote": last_modified_remote_val,
        "metadata": metadata_val,
        "error_info": error_info_val,
    });

    let item: SyncItem = serde_json::from_value(item_json).map_err(|e| {
        CacheError::SerializationError(format!("Failed to reconstruct SyncItem from row: {}", e))
    })?;

    Ok(item)
}

/// Reconstruct an Account from a database row
fn account_from_row(row: &SqliteRow) -> Result<Account, CacheError> {
    let id_str: String = row.get("id");
    let email_str: String = row.get("email");
    let display_name: String = row.get("display_name");
    let onedrive_id: String = row.get("onedrive_id");
    let sync_root_str: String = row.get("sync_root");
    let quota_used: i64 = row.get("quota_used");
    let quota_total: i64 = row.get("quota_total");
    let delta_token_str: Option<String> = row.get("delta_token");
    let last_sync_str: Option<String> = row.get("last_sync");
    let state_str: String = row.get("state");
    let created_at_str: String = row.get("created_at");

    let id = AccountId::from_str(&id_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid AccountId '{}': {}", id_str, e))
    })?;

    let email = Email::new(email_str.clone()).map_err(|e| {
        CacheError::SerializationError(format!("Invalid Email '{}': {}", email_str, e))
    })?;

    let sync_root = SyncPath::new(PathBuf::from(&sync_root_str)).map_err(|e| {
        CacheError::SerializationError(format!("Invalid SyncPath '{}': {}", sync_root_str, e))
    })?;

    let created_at = parse_datetime(&created_at_str)?;
    let state = account_state_from_string(&state_str)?;
    let last_sync = parse_optional_datetime(last_sync_str)?;

    // Reconstruct the account with its stored ID
    let mut account = Account::with_id(id, email, display_name, onedrive_id, sync_root, created_at);
    account.update_quota(quota_used as u64, quota_total as u64);
    account.set_state(state);

    if let Some(ts) = last_sync {
        account.record_sync(ts);
    }

    if let Some(token_str) = delta_token_str {
        if !token_str.is_empty() {
            if let Ok(token) = DeltaToken::new(token_str) {
                account.update_delta_token(token);
            }
        }
    }

    Ok(account)
}

/// Reconstruct a SyncSession from a database row
fn session_from_row(row: &SqliteRow) -> Result<SyncSession, CacheError> {
    let id_str: String = row.get("id");
    let account_id_str: String = row.get("account_id");
    let started_at_str: String = row.get("started_at");
    let completed_at_str: Option<String> = row.get("completed_at");
    let status_str: String = row.get("status");
    let items_total: i64 = row.get("items_total");
    let items_processed: i64 = row.get("items_processed");
    let items_succeeded: i64 = row.get("items_succeeded");
    let items_failed: i64 = row.get("items_failed");
    let bytes_uploaded: i64 = row.get("bytes_uploaded");
    let bytes_downloaded: i64 = row.get("bytes_downloaded");
    let delta_token_start_str: Option<String> = row.get("delta_token_start");
    let delta_token_end_str: Option<String> = row.get("delta_token_end");
    let errors_str: String = row.get("errors");

    let id = SessionId::from_str(&id_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid SessionId '{}': {}", id_str, e))
    })?;

    let account_id = AccountId::from_str(&account_id_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid AccountId '{}': {}", account_id_str, e))
    })?;

    let started_at = parse_datetime(&started_at_str)?;
    let _completed_at = parse_optional_datetime(completed_at_str)?;
    let status = session_status_from_string(&status_str)?;

    // Reconstruct the session
    let mut session = SyncSession::with_id(id, account_id, started_at);
    session.set_items_total(items_total as u64);
    session.update_progress(
        items_processed as u64,
        items_succeeded as u64,
        items_failed as u64,
    );
    session.add_bytes_uploaded(bytes_uploaded as u64);
    session.add_bytes_downloaded(bytes_downloaded as u64);

    // Set delta tokens
    if let Some(ref token_str) = delta_token_start_str {
        if !token_str.is_empty() {
            if let Ok(token) = DeltaToken::new(token_str.clone()) {
                session.set_delta_token_start(token);
            }
        }
    }

    if let Some(ref token_str) = delta_token_end_str {
        if !token_str.is_empty() {
            if let Ok(token) = DeltaToken::new(token_str.clone()) {
                session.set_delta_token_end(token);
            }
        }
    }

    // Deserialize errors
    let errors: Vec<SessionError> = serde_json::from_str(&errors_str).unwrap_or_default();
    for error in errors {
        session.add_error(error);
    }

    // Apply final status (must be done after setting progress)
    match status {
        SessionStatus::Completed => session.complete(),
        SessionStatus::Failed(msg) => session.fail(msg),
        SessionStatus::Cancelled => session.cancel(),
        SessionStatus::Running => {} // already in Running state from with_id
    }

    Ok(session)
}

/// Reconstruct an AuditEntry from a database row
///
/// Uses serde JSON deserialization to reconstruct with the correct stored
/// timestamp (rather than the current time that AuditEntry::new() would use).
fn audit_entry_from_row(row: &SqliteRow) -> Result<AuditEntry, CacheError> {
    let id: i64 = row.get("id");
    let timestamp_str: String = row.get("timestamp");
    let session_id_str: Option<String> = row.get("session_id");
    let item_id_str: Option<String> = row.get("item_id");
    let action_str: String = row.get("action");
    let result_str: String = row.get("result");
    let details_str: String = row.get("details");
    let duration_ms: Option<i64> = row.get("duration_ms");

    let timestamp = parse_datetime(&timestamp_str)?;

    // Parse action and result for JSON reconstruction
    let action: AuditAction =
        serde_json::from_str(&format!("\"{}\"", action_str)).map_err(|e| {
            CacheError::SerializationError(format!("Invalid AuditAction '{}': {}", action_str, e))
        })?;

    let result: AuditResult = serde_json::from_str(&result_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid AuditResult '{}': {}", result_str, e))
    })?;

    let details: serde_json::Value = serde_json::from_str(&details_str).unwrap_or_default();

    // Convert optional fields
    let session_id_val = match session_id_str {
        Some(ref s) if !s.is_empty() => serde_json::Value::String(s.clone()),
        _ => serde_json::Value::Null,
    };

    let item_id_val = match item_id_str {
        Some(ref s) if !s.is_empty() => serde_json::Value::String(s.clone()),
        _ => serde_json::Value::Null,
    };

    let duration_val = match duration_ms {
        Some(d) => serde_json::Value::Number(serde_json::Number::from(d as u64)),
        None => serde_json::Value::Null,
    };

    // Reconstruct via JSON deserialization to preserve the stored timestamp
    let entry_json = serde_json::json!({
        "id": id,
        "timestamp": timestamp.to_rfc3339(),
        "session_id": session_id_val,
        "item_id": item_id_val,
        "action": action,
        "result": result,
        "details": details,
        "duration_ms": duration_val,
    });

    let entry: AuditEntry = serde_json::from_value(entry_json).map_err(|e| {
        CacheError::SerializationError(format!("Failed to reconstruct AuditEntry from row: {}", e))
    })?;

    Ok(entry)
}

/// Reconstruct a Conflict from a database row
fn conflict_from_row(row: &SqliteRow) -> Result<Conflict, CacheError> {
    let id_str: String = row.get("id");
    let item_id_str: String = row.get("item_id");
    let detected_at_str: String = row.get("detected_at");
    let local_version_str: String = row.get("local_version");
    let remote_version_str: String = row.get("remote_version");
    let resolution_str: Option<String> = row.get("resolution");
    let resolved_at_str: Option<String> = row.get("resolved_at");
    let resolved_by_str: Option<String> = row.get("resolved_by");

    let _id = ConflictId::from_str(&id_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid ConflictId '{}': {}", id_str, e))
    })?;

    let _item_id = UniqueId::from_str(&item_id_str).map_err(|e| {
        CacheError::SerializationError(format!("Invalid UniqueId '{}': {}", item_id_str, e))
    })?;

    let _detected_at = parse_datetime(&detected_at_str)?;

    let local_version: VersionInfo = serde_json::from_str(&local_version_str)
        .map_err(|e| CacheError::SerializationError(format!("Invalid VersionInfo JSON: {}", e)))?;

    let remote_version: VersionInfo = serde_json::from_str(&remote_version_str)
        .map_err(|e| CacheError::SerializationError(format!("Invalid VersionInfo JSON: {}", e)))?;

    let resolution_val = match &resolution_str {
        Some(s) if !s.is_empty() => {
            let r: Resolution = serde_json::from_str(&format!("\"{}\"", s)).map_err(|e| {
                CacheError::SerializationError(format!("Invalid Resolution '{}': {}", s, e))
            })?;
            Some(serde_json::to_value(&r).unwrap())
        }
        _ => None,
    };

    let resolved_at = parse_optional_datetime(resolved_at_str)?;

    let resolved_by_val = match &resolved_by_str {
        Some(s) if !s.is_empty() => {
            let rb: ResolutionSource =
                serde_json::from_str(&format!("\"{}\"", s)).map_err(|e| {
                    CacheError::SerializationError(format!(
                        "Invalid ResolutionSource '{}': {}",
                        s, e
                    ))
                })?;
            Some(serde_json::to_value(&rb).unwrap())
        }
        _ => None,
    };

    // Reconstruct via serde
    let conflict_json = serde_json::json!({
        "id": id_str,
        "item_id": item_id_str,
        "detected_at": _detected_at.to_rfc3339(),
        "local_version": local_version,
        "remote_version": remote_version,
        "resolution": resolution_val,
        "resolved_at": resolved_at.map(|dt| dt.to_rfc3339()),
        "resolved_by": resolved_by_val,
    });

    let conflict: Conflict = serde_json::from_value(conflict_json).map_err(|e| {
        CacheError::SerializationError(format!("Failed to reconstruct Conflict from row: {}", e))
    })?;

    Ok(conflict)
}

// ============================================================================
// IStateRepository implementation
// ============================================================================

#[async_trait::async_trait]
impl IStateRepository for SqliteStateRepository {
    // --- SyncItem operations ---

    async fn save_item(&self, item: &SyncItem) -> anyhow::Result<()> {
        let id = item.id().to_string();
        // We need the account_id from the sync_items table context.
        // SyncItem doesn't carry account_id directly - it's part of the DB schema.
        // For UPSERT, we'll need to handle this. Let's check if there's an existing
        // row to get the account_id, or we'll store the item without it initially.
        //
        // Looking at the schema, account_id is NOT NULL.
        // Since SyncItem doesn't have an account_id field, we need to handle this
        // through the query_items filter. For save_item, the account_id should
        // already exist in the DB row (update case) or be provided.
        //
        // For now, we'll use a sub-query approach: if the item exists, keep the
        // existing account_id. For new items, we'll use the default account.
        // However, this is a limitation. Let's use a pragmatic approach:
        // attempt to get the existing account_id, or fall back to the first account.

        let local_path = item.local_path().to_string();
        let remote_id = item.remote_id().map(|r| r.as_str().to_string());
        let remote_path = item.remote_path().as_str().to_string();
        let state = item_state_to_string(item.state());
        let content_hash = item.content_hash().map(|h| h.as_str().to_string());
        let local_hash = item.local_hash().map(|h| h.as_str().to_string());
        let size_bytes = item.size_bytes() as i64;
        let last_sync = item.last_sync().map(|dt| dt.to_rfc3339());
        let last_modified_local = item.last_modified_local().map(|dt| dt.to_rfc3339());
        let last_modified_remote = item.last_modified_remote().map(|dt| dt.to_rfc3339());
        let metadata = serde_json::to_string(item.metadata())
            .map_err(|e| anyhow::anyhow!("Failed to serialize metadata: {}", e))?;
        let error_info = match item.error_info() {
            Some(ei) => Some(
                serde_json::to_string(ei)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize error_info: {}", e))?,
            ),
            None => None,
        };

        // Try to get existing account_id for this item, or use first account
        let existing_account_id: Option<String> =
            sqlx::query_scalar("SELECT account_id FROM sync_items WHERE id = ?")
                .bind(&id)
                .fetch_optional(&self.pool)
                .await?;

        let account_id = match existing_account_id {
            Some(aid) => aid,
            None => {
                // Get the first/default account
                let default_aid: Option<String> =
                    sqlx::query_scalar("SELECT id FROM accounts ORDER BY created_at ASC LIMIT 1")
                        .fetch_optional(&self.pool)
                        .await?;
                default_aid.ok_or_else(|| {
                    anyhow::anyhow!("No account found to associate with sync item")
                })?
            }
        };

        sqlx::query(
            "INSERT OR REPLACE INTO sync_items \
             (id, account_id, local_path, remote_id, remote_path, state, \
              content_hash, local_hash, size_bytes, last_sync, \
              last_modified_local, last_modified_remote, metadata, error_info) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&account_id)
        .bind(&local_path)
        .bind(&remote_id)
        .bind(&remote_path)
        .bind(&state)
        .bind(&content_hash)
        .bind(&local_hash)
        .bind(size_bytes)
        .bind(&last_sync)
        .bind(&last_modified_local)
        .bind(&last_modified_remote)
        .bind(&metadata)
        .bind(&error_info)
        .execute(&self.pool)
        .await?;

        tracing::trace!(item_id = %id, "Saved sync item");
        Ok(())
    }

    async fn get_item(&self, id: &UniqueId) -> anyhow::Result<Option<SyncItem>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM sync_items WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(sync_item_from_row(r)?)),
            None => Ok(None),
        }
    }

    async fn get_item_by_path(&self, path: &SyncPath) -> anyhow::Result<Option<SyncItem>> {
        let path_str = path.to_string();

        let row = sqlx::query("SELECT * FROM sync_items WHERE local_path = ?")
            .bind(&path_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(sync_item_from_row(r)?)),
            None => Ok(None),
        }
    }

    async fn get_item_by_remote_id(
        &self,
        remote_id: &RemoteId,
    ) -> anyhow::Result<Option<SyncItem>> {
        let remote_id_str = remote_id.as_str();

        let row = sqlx::query("SELECT * FROM sync_items WHERE remote_id = ?")
            .bind(remote_id_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(sync_item_from_row(r)?)),
            None => Ok(None),
        }
    }

    async fn query_items(&self, filter: &ItemFilter) -> anyhow::Result<Vec<SyncItem>> {
        let mut sql = String::from("SELECT * FROM sync_items WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();

        if let Some(ref account_id) = filter.account_id {
            sql.push_str(" AND account_id = ?");
            binds.push(account_id.to_string());
        }

        if let Some(ref state) = filter.state {
            sql.push_str(" AND state = ?");
            binds.push(item_state_to_string(state));
        }

        if let Some(ref path_prefix) = filter.path_prefix {
            sql.push_str(" AND local_path LIKE ?");
            // Use LIKE with escaped % for prefix matching
            let prefix = format!("{path_prefix}%");
            binds.push(prefix);
        }

        if let Some(ref modified_since) = filter.modified_since {
            sql.push_str(" AND last_modified_local > ?");
            binds.push(modified_since.to_rfc3339());
        }

        // Build the query dynamically
        let mut query = sqlx::query(&sql);
        for bind in &binds {
            query = query.bind(bind);
        }

        let rows = query.fetch_all(&self.pool).await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in &rows {
            items.push(sync_item_from_row(row)?);
        }

        Ok(items)
    }

    async fn delete_item(&self, id: &UniqueId) -> anyhow::Result<()> {
        let id_str = id.to_string();

        sqlx::query("DELETE FROM sync_items WHERE id = ?")
            .bind(&id_str)
            .execute(&self.pool)
            .await?;

        tracing::trace!(item_id = %id_str, "Deleted sync item");
        Ok(())
    }

    async fn count_items_by_state(
        &self,
        account_id: &AccountId,
    ) -> anyhow::Result<HashMap<String, u64>> {
        let account_id_str = account_id.to_string();

        let rows = sqlx::query(
            "SELECT state, COUNT(*) as count FROM sync_items \
             WHERE account_id = ? GROUP BY state",
        )
        .bind(&account_id_str)
        .fetch_all(&self.pool)
        .await?;

        let mut counts = HashMap::new();
        for row in &rows {
            let state_str: String = row.get("state");
            let count: i64 = row.get("count");

            // Convert the stored state string to the domain state name
            let state = item_state_from_string(&state_str)?;
            counts.insert(state.name().to_string(), count as u64);
        }

        Ok(counts)
    }

    // --- Account operations ---

    async fn save_account(&self, account: &Account) -> anyhow::Result<()> {
        let id = account.id().to_string();
        let email = account.email().as_str().to_string();
        let display_name = account.display_name().to_string();
        let onedrive_id = account.onedrive_id().to_string();
        let sync_root = account.sync_root().to_string();
        let quota_used = account.quota_used() as i64;
        let quota_total = account.quota_total() as i64;
        let delta_token = account.delta_token().map(|t| t.as_str().to_string());
        let last_sync = account.last_sync().map(|dt| dt.to_rfc3339());
        let state = account_state_to_string(account.state());
        let created_at = account.created_at().to_rfc3339();

        sqlx::query(
            "INSERT OR REPLACE INTO accounts \
             (id, email, display_name, onedrive_id, sync_root, \
              quota_used, quota_total, delta_token, last_sync, state, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&email)
        .bind(&display_name)
        .bind(&onedrive_id)
        .bind(&sync_root)
        .bind(quota_used)
        .bind(quota_total)
        .bind(&delta_token)
        .bind(&last_sync)
        .bind(&state)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        tracing::trace!(account_id = %id, "Saved account");
        Ok(())
    }

    async fn get_account(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM accounts WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(account_from_row(r)?)),
            None => Ok(None),
        }
    }

    async fn get_default_account(&self) -> anyhow::Result<Option<Account>> {
        let row = sqlx::query("SELECT * FROM accounts ORDER BY created_at ASC LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(account_from_row(r)?)),
            None => Ok(None),
        }
    }

    // --- Session operations ---

    async fn save_session(&self, session: &SyncSession) -> anyhow::Result<()> {
        let id = session.id().to_string();
        let account_id = session.account_id().to_string();
        let started_at = session.started_at().to_rfc3339();
        let completed_at = session.completed_at().map(|dt| dt.to_rfc3339());
        let status = session_status_to_string(session.status());
        let items_total = session.items_total() as i64;
        let items_processed = session.items_processed() as i64;
        let items_succeeded = session.items_succeeded() as i64;
        let items_failed = session.items_failed() as i64;
        let bytes_uploaded = session.bytes_uploaded() as i64;
        let bytes_downloaded = session.bytes_downloaded() as i64;
        let delta_token_start = session.delta_token_start().map(|t| t.as_str().to_string());
        let delta_token_end = session.delta_token_end().map(|t| t.as_str().to_string());
        let errors = serde_json::to_string(session.errors())
            .map_err(|e| anyhow::anyhow!("Failed to serialize session errors: {}", e))?;

        sqlx::query(
            "INSERT OR REPLACE INTO sync_sessions \
             (id, account_id, started_at, completed_at, status, \
              items_total, items_processed, items_succeeded, items_failed, \
              bytes_uploaded, bytes_downloaded, delta_token_start, delta_token_end, errors) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&account_id)
        .bind(&started_at)
        .bind(&completed_at)
        .bind(&status)
        .bind(items_total)
        .bind(items_processed)
        .bind(items_succeeded)
        .bind(items_failed)
        .bind(bytes_uploaded)
        .bind(bytes_downloaded)
        .bind(&delta_token_start)
        .bind(&delta_token_end)
        .bind(&errors)
        .execute(&self.pool)
        .await?;

        tracing::trace!(session_id = %id, "Saved sync session");
        Ok(())
    }

    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<SyncSession>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM sync_sessions WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(ref r) => Ok(Some(session_from_row(r)?)),
            None => Ok(None),
        }
    }

    // --- Audit operations ---

    async fn save_audit(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        let timestamp = entry.timestamp().to_rfc3339();
        let session_id = entry.session_id().map(|s| s.to_string());
        let item_id = entry.item_id().map(|i| i.to_string());
        let action = entry.action().to_string();
        let result = serde_json::to_string(entry.result())
            .map_err(|e| anyhow::anyhow!("Failed to serialize audit result: {}", e))?;
        let details = serde_json::to_string(entry.details())
            .map_err(|e| anyhow::anyhow!("Failed to serialize audit details: {}", e))?;
        let duration_ms = entry.duration_ms().map(|d| d as i64);

        sqlx::query(
            "INSERT INTO audit_log \
             (timestamp, session_id, item_id, action, result, details, duration_ms) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&timestamp)
        .bind(&session_id)
        .bind(&item_id)
        .bind(&action)
        .bind(&result)
        .bind(&details)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;

        tracing::trace!(action = %action, "Saved audit entry");
        Ok(())
    }

    async fn get_audit_trail(&self, item_id: &UniqueId) -> anyhow::Result<Vec<AuditEntry>> {
        let item_id_str = item_id.to_string();

        let rows = sqlx::query("SELECT * FROM audit_log WHERE item_id = ? ORDER BY timestamp ASC")
            .bind(&item_id_str)
            .fetch_all(&self.pool)
            .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            entries.push(audit_entry_from_row(row)?);
        }

        Ok(entries)
    }

    async fn get_audit_since(
        &self,
        since: DateTime<Utc>,
        limit: u32,
    ) -> anyhow::Result<Vec<AuditEntry>> {
        let since_str = since.to_rfc3339();

        let rows = sqlx::query(
            "SELECT * FROM audit_log WHERE timestamp > ? \
             ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(&since_str)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            entries.push(audit_entry_from_row(row)?);
        }

        Ok(entries)
    }

    // --- Conflict operations ---

    async fn save_conflict(&self, conflict: &Conflict) -> anyhow::Result<()> {
        let id = conflict.id().to_string();
        let item_id = conflict.item_id().to_string();
        let detected_at = conflict.detected_at().to_rfc3339();
        let local_version = serde_json::to_string(conflict.local_version())
            .map_err(|e| anyhow::anyhow!("Failed to serialize local_version: {}", e))?;
        let remote_version = serde_json::to_string(conflict.remote_version())
            .map_err(|e| anyhow::anyhow!("Failed to serialize remote_version: {}", e))?;

        let resolution = conflict.resolution().map(|r| {
            // Serialize the Resolution enum to its string form
            serde_json::to_string(r)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string()
        });

        let resolved_at = conflict.resolved_at().map(|dt| dt.to_rfc3339());

        let resolved_by = conflict.resolved_by().map(|rb| {
            serde_json::to_string(rb)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string()
        });

        sqlx::query(
            "INSERT OR REPLACE INTO conflicts \
             (id, item_id, detected_at, local_version, remote_version, \
              resolution, resolved_at, resolved_by) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&item_id)
        .bind(&detected_at)
        .bind(&local_version)
        .bind(&remote_version)
        .bind(&resolution)
        .bind(&resolved_at)
        .bind(&resolved_by)
        .execute(&self.pool)
        .await?;

        tracing::trace!(conflict_id = %id, "Saved conflict");
        Ok(())
    }

    async fn get_unresolved_conflicts(&self) -> anyhow::Result<Vec<Conflict>> {
        let rows = sqlx::query(
            "SELECT * FROM conflicts WHERE resolution IS NULL \
             ORDER BY detected_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut conflicts = Vec::with_capacity(rows.len());
        for row in &rows {
            conflicts.push(conflict_from_row(row)?);
        }

        Ok(conflicts)
    }
}
