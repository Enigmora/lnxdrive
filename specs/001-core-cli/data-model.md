# Data Model: Core + CLI (Fase 1)

**Branch**: `001-core-cli` | **Date**: 2026-02-03

## Domain Entities

### SyncItem

Represents a file or directory being synchronized.

```
SyncItem
├── id: UniqueId (newtype over UUID)
├── local_path: SyncPath (newtype, absolute path validated)
├── remote_id: RemoteId (OneDrive item ID)
├── remote_path: RemotePath (path in OneDrive)
├── state: ItemState (enum)
├── content_hash: Option<FileHash> (SHA-256 from OneDrive)
├── local_hash: Option<FileHash> (locally computed)
├── size_bytes: u64
├── last_sync: Option<DateTime<Utc>>
├── last_modified_local: DateTime<Utc>
├── last_modified_remote: DateTime<Utc>
├── metadata: ItemMetadata
└── error_info: Option<ErrorInfo>
```

**State Transitions (ItemState enum)**:
```
                    ┌──────────────────────────────────────┐
                    │                                      │
    ┌──────────┐    │     ┌───────────┐     ┌───────────┐  │
    │  Online  │────┼────►│ Hydrating │────►│ Hydrated  │──┘
    │(metadata)│    │     │(downloading)    │ (synced)  │
    └──────────┘    │     └───────────┘     └─────┬─────┘
         ▲          │                             │
         │          │                        modify
         │ dehydrate│                             │
         │          │     ┌───────────┐           │
         └──────────┼─────│ Modified  │◄──────────┘
                    │     │ (pending) │
                    │     └─────┬─────┘
                    │           │
                    │      sync │ conflict
                    │           │     │
                    │     ┌─────▼─────▼───┐
                    │     │  Conflicted   │
                    │     └───────────────┘
                    │
                    │     ┌───────────┐
                    └────►│   Error   │ (from any state)
                          │ (reason)  │
                          └───────────┘
```

**Validation Rules**:
- `local_path` MUST be absolute and within sync root
- `content_hash` MUST match OneDrive's quickXorHash format
- `size_bytes` MUST match actual file size
- `state` transitions MUST follow state machine rules

### Account

Represents a linked OneDrive account.

```
Account
├── id: AccountId (newtype over UUID)
├── email: Email (validated email format)
├── display_name: String
├── onedrive_id: String (OneDrive user ID)
├── sync_root: SyncPath (local folder path)
├── quota_used: u64 (bytes)
├── quota_total: u64 (bytes)
├── delta_token: Option<DeltaToken>
├── last_sync: Option<DateTime<Utc>>
├── state: AccountState
└── created_at: DateTime<Utc>
```

**AccountState enum**:
- `Active` - Normal operation
- `TokenExpired` - Needs re-authentication
- `Suspended` - Manually paused
- `Error(String)` - Error state with reason

**Validation Rules**:
- `email` MUST be valid email format
- `sync_root` MUST be absolute, writable directory
- `delta_token` format validated per MS Graph spec

### SyncSession

Represents an active synchronization session.

```
SyncSession
├── id: SessionId (newtype over UUID)
├── account_id: AccountId
├── started_at: DateTime<Utc>
├── completed_at: Option<DateTime<Utc>>
├── status: SessionStatus
├── items_total: u32
├── items_processed: u32
├── items_succeeded: u32
├── items_failed: u32
├── bytes_uploaded: u64
├── bytes_downloaded: u64
├── delta_token_start: Option<DeltaToken>
├── delta_token_end: Option<DeltaToken>
└── errors: Vec<SessionError>
```

**SessionStatus enum**:
- `Running` - In progress
- `Completed` - Finished successfully
- `Failed(String)` - Stopped due to error
- `Cancelled` - User cancelled

### AuditEntry

Represents an audit log entry.

```
AuditEntry
├── id: AuditId (auto-increment)
├── timestamp: DateTime<Utc>
├── session_id: Option<SessionId>
├── item_id: Option<UniqueId>
├── action: AuditAction
├── result: AuditResult
├── details: JsonValue (structured context)
└── duration_ms: Option<u32>
```

**AuditAction enum**:
- `AuthLogin`
- `AuthLogout`
- `AuthRefresh`
- `SyncStart`
- `SyncComplete`
- `FileUpload`
- `FileDownload`
- `FileDelete`
- `ConflictDetected`
- `ConflictResolved`
- `Error`
- `ConfigChange`

**AuditResult enum**:
- `Success`
- `Failed(ErrorCode, String)`

### Conflict

Represents a synchronization conflict.

```
Conflict
├── id: ConflictId (newtype over UUID)
├── item_id: UniqueId
├── detected_at: DateTime<Utc>
├── local_version: VersionInfo
├── remote_version: VersionInfo
├── resolution: Option<Resolution>
├── resolved_at: Option<DateTime<Utc>>
└── resolved_by: Option<ResolutionSource>
```

**VersionInfo**:
```
VersionInfo
├── hash: FileHash
├── size_bytes: u64
├── modified_at: DateTime<Utc>
└── etag: Option<String>
```

**Resolution enum**:
- `KeepLocal` - Use local version
- `KeepRemote` - Use remote version
- `KeepBoth` - Rename local with conflict suffix
- `Manual` - User merged manually

**ResolutionSource enum**:
- `User` - User chose resolution
- `Policy` - Automatic per configuration
- `System` - System default

## SQLite Schema

```sql
-- Accounts table
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    onedrive_id TEXT NOT NULL,
    sync_root TEXT NOT NULL,
    quota_used INTEGER NOT NULL DEFAULT 0,
    quota_total INTEGER NOT NULL DEFAULT 0,
    delta_token TEXT,
    last_sync TEXT,
    state TEXT NOT NULL DEFAULT 'Active',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync items table
CREATE TABLE sync_items (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    local_path TEXT NOT NULL,
    remote_id TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'Online',
    content_hash TEXT,
    local_hash TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_sync TEXT,
    last_modified_local TEXT NOT NULL,
    last_modified_remote TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    error_info TEXT,
    UNIQUE(account_id, local_path),
    UNIQUE(account_id, remote_id)
);

CREATE INDEX idx_sync_items_state ON sync_items(state);
CREATE INDEX idx_sync_items_path ON sync_items(local_path);

-- Sync sessions table
CREATE TABLE sync_sessions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    status TEXT NOT NULL DEFAULT 'Running',
    items_total INTEGER NOT NULL DEFAULT 0,
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_succeeded INTEGER NOT NULL DEFAULT 0,
    items_failed INTEGER NOT NULL DEFAULT 0,
    bytes_uploaded INTEGER NOT NULL DEFAULT 0,
    bytes_downloaded INTEGER NOT NULL DEFAULT 0,
    delta_token_start TEXT,
    delta_token_end TEXT,
    errors TEXT NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_sync_sessions_account ON sync_sessions(account_id);
CREATE INDEX idx_sync_sessions_status ON sync_sessions(status);

-- Audit log table
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    session_id TEXT REFERENCES sync_sessions(id),
    item_id TEXT REFERENCES sync_items(id),
    action TEXT NOT NULL,
    result TEXT NOT NULL,
    details TEXT NOT NULL DEFAULT '{}',
    duration_ms INTEGER
);

CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_action ON audit_log(action);
CREATE INDEX idx_audit_log_item ON audit_log(item_id);

-- Conflicts table
CREATE TABLE conflicts (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES sync_items(id) ON DELETE CASCADE,
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    local_version TEXT NOT NULL,
    remote_version TEXT NOT NULL,
    resolution TEXT,
    resolved_at TEXT,
    resolved_by TEXT
);

CREATE INDEX idx_conflicts_item ON conflicts(item_id);
CREATE INDEX idx_conflicts_unresolved ON conflicts(resolution) WHERE resolution IS NULL;

-- Configuration table (key-value store)
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

## Newtypes (Type Safety)

```rust
// All newtypes implement Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize

pub struct UniqueId(Uuid);
pub struct AccountId(Uuid);
pub struct SessionId(Uuid);
pub struct ConflictId(Uuid);
pub struct AuditId(i64);

pub struct SyncPath(PathBuf);  // Validated absolute path within sync root
pub struct RemotePath(String); // OneDrive path format
pub struct RemoteId(String);   // OneDrive item ID

pub struct FileHash(String);   // quickXorHash format
pub struct DeltaToken(String); // MS Graph delta token

pub struct Email(String);      // Validated email format
```

## Relationships

```
Account (1) ──────────────────────────────────┬──── (N) SyncItem
                                              │
Account (1) ──────────────────────────────────┼──── (N) SyncSession
                                              │
SyncItem (1) ─────────────────────────────────┼──── (N) AuditEntry (optional)
                                              │
SyncItem (1) ─────────────────────────────────┼──── (0..1) Conflict
                                              │
SyncSession (1) ──────────────────────────────┴──── (N) AuditEntry (optional)
```

## Configuration Schema (YAML)

```yaml
# ~/.config/lnxdrive/config.yaml

# Sync configuration
sync:
  root: ~/OneDrive               # Sync folder path
  poll_interval: 30              # Seconds between remote checks
  debounce_delay: 2              # Seconds to wait after local change

# Rate limiting
rate_limiting:
  delta_requests_per_minute: 10
  upload_concurrent: 4
  upload_requests_per_minute: 60
  download_concurrent: 8
  metadata_requests_per_minute: 100

# Large file handling
large_files:
  threshold_mb: 100
  chunk_size_mb: 10
  max_concurrent_large: 1

# Conflict resolution
conflicts:
  default_strategy: manual       # manual | keep_local | keep_remote | keep_both

# Logging
logging:
  level: info                    # trace | debug | info | warn | error
  file: ~/.local/share/lnxdrive/lnxdrive.log
  max_size_mb: 50
  max_files: 5

# Authentication (read-only, set via CLI)
auth:
  app_id: null                   # Custom app ID (optional)
```
