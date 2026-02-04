# Research: Core + CLI (Fase 1)

**Branch**: `001-core-cli` | **Date**: 2026-02-03

## Technology Decisions

### 1. OAuth2 Library Selection

**Decision**: `oauth2-rs` crate

**Rationale**:
- Native Rust implementation with async support
- Built-in PKCE support (required for public clients)
- Actively maintained with good documentation
- Integrates cleanly with tokio/reqwest ecosystem
- Supports custom token storage via traits

**Alternatives Considered**:
- `openidconnect`: More complex, includes OIDC which we don't need
- `yup-oauth2`: Google-focused, less suitable for Microsoft Graph
- Manual implementation: Higher risk, maintenance burden

### 2. HTTP Client

**Decision**: `reqwest` with `rustls`

**Rationale**:
- De-facto standard for async HTTP in Rust
- Native TLS via rustls (no OpenSSL dependency)
- Connection pooling built-in
- Integrates with tokio runtime
- Supports streaming for large file uploads/downloads

**Alternatives Considered**:
- `hyper` directly: Lower level, more boilerplate
- `ureq`: Sync only, not suitable for async daemon
- `surf`: Less mature ecosystem

### 3. SQLite Access

**Decision**: `sqlx` with compile-time query verification

**Rationale**:
- Compile-time SQL verification catches errors early
- Native async support with tokio
- Built-in migration system
- Connection pooling
- No ORM overhead, direct SQL control

**Alternatives Considered**:
- `rusqlite`: Sync only, would need tokio::spawn_blocking
- `diesel`: ORM overhead, compile times
- `sea-orm`: Additional abstraction layer not needed

### 4. D-Bus Integration

**Decision**: `zbus` (async)

**Rationale**:
- Pure Rust, async-native
- Procedural macros for interface definition
- Active development, good documentation
- Recommended by GNOME/freedesktop community
- Works with both system and session bus

**Alternatives Considered**:
- `dbus-rs`: Requires libdbus C library
- `dbus-native`: Less mature

### 5. CLI Framework

**Decision**: `clap` v4 with derive macros

**Rationale**:
- Industry standard for Rust CLIs
- Derive macros reduce boilerplate
- Built-in help generation
- Supports subcommands, flags, arguments
- Shell completions generation

**Alternatives Considered**:
- `structopt`: Merged into clap v3+
- `argh`: Simpler but less features
- Manual parsing: Error prone

### 6. Keyring Access

**Decision**: `keyring` crate with libsecret backend

**Rationale**:
- Cross-platform abstraction
- Uses libsecret on Linux (GNOME Keyring, KWallet)
- Simple API for credential storage
- Follows XDG Secret Service specification

**Alternatives Considered**:
- `secret-service`: Lower level, more complex
- `libsecret-rs`: Direct bindings, less portable
- File-based with encryption: Less secure

### 7. File System Watching

**Decision**: `notify` crate with inotify backend

**Rationale**:
- Cross-platform abstraction
- Uses inotify on Linux (kernel-level efficiency)
- Debouncing support
- Active maintenance
- Handles recursive watching

**Alternatives Considered**:
- `inotify` directly: Platform-specific, more work
- Polling: Inefficient for large directories

### 8. Serialization

**Decision**: `serde` with `serde_json` and `serde_yaml`

**Rationale**:
- Industry standard for Rust serialization
- Derive macros for automatic implementation
- JSON for API responses and CLI output
- YAML for configuration files
- Zero-copy deserialization when needed

**Alternatives Considered**:
- Manual parsing: Error prone, maintenance burden
- `simd-json`: Marginal performance gain not worth complexity

## Best Practices Research

### Microsoft Graph API Patterns

**Delta Query Pattern**:
1. Initial call to `/me/drive/root/delta` without token
2. Process all items in response
3. Follow `@odata.nextLink` for pagination
4. Store `@odata.deltaLink` for next sync
5. On subsequent syncs, use stored deltaLink
6. Handle 410 Gone by restarting from scratch

**Rate Limiting Strategy**:
- Start at 50% of theoretical limit
- Increase 10% per 5-minute window without 429s
- Decrease 50% on any 429, hold for 10 minutes
- Respect `Retry-After` header absolutely
- Separate buckets per endpoint type

**Upload Session Pattern**:
- Files <4MB: Direct PUT
- Files 4MB-60MB: Upload session with 10MB chunks
- Files >60MB: Upload session with progress tracking
- Always verify with hash after upload
- Resume interrupted uploads via session URL

### Tokio Async Patterns

**Structured Concurrency**:
- Use `tokio::select!` for cancellation
- Implement graceful shutdown with `CancellationToken`
- Limit concurrency with `Semaphore`
- Use channels for inter-task communication

**Error Handling**:
- Core/libraries use `thiserror` for typed errors
- Application/CLI uses `anyhow` for context
- Always wrap external errors with context
- Log errors at appropriate level

### SQLite Patterns

**Connection Pooling**:
- Use `sqlx::Pool` with reasonable pool size (4-8)
- Enable WAL mode for concurrent reads
- Use transactions for multi-step operations
- Migrations run at startup

**Schema Design**:
- Use INTEGER PRIMARY KEY for auto-increment
- Store timestamps as ISO 8601 strings
- Use JSON columns for flexible metadata
- Index on frequently queried columns (path, state)

## Integration Patterns

### Microsoft Graph API v1.0

**Authentication Endpoints**:
- Authorization: `https://login.microsoftonline.com/common/oauth2/v2.0/authorize`
- Token: `https://login.microsoftonline.com/common/oauth2/v2.0/token`
- Scopes: `Files.ReadWrite offline_access User.Read`

**Delta API Endpoint**:
- `GET https://graph.microsoft.com/v1.0/me/drive/root/delta`
- Returns: `{ value: [...items], @odata.nextLink?, @odata.deltaLink? }`

**File Operations**:
- Download: `GET /me/drive/items/{id}/content`
- Upload <4MB: `PUT /me/drive/root:/{path}:/content`
- Upload session: `POST /me/drive/root:/{path}:/createUploadSession`
- Metadata: `GET /me/drive/items/{id}`

### Error Handling Matrix

| Error Type | Detection | Action |
|------------|-----------|--------|
| 401 Unauthorized | HTTP status | Refresh token, if fails re-auth |
| 403 Forbidden | HTTP status | Log, mark item as error |
| 404 Not Found | HTTP status | Remove from local state |
| 409 Conflict | HTTP status | Mark as conflict, defer to user |
| 429 Too Many Requests | HTTP status + header | Backoff per Retry-After |
| 5xx Server Error | HTTP status | Retry with exponential backoff |
| Network Error | reqwest error | Queue for retry, update offline status |
| Disk Full | std::io error | Notify user, pause downloads |
| Permission Denied | std::io error | Log, mark item as error |

## Risk Mitigations (from Design Guide)

### B1: Error Recovery Transitions
- State machine includes explicit `Error` → `Retry` → `Previous` transitions
- Maximum retry count with exponential backoff
- User notification on persistent failures

### A1: D-Bus Single Point of Failure
- Daemon continues sync operations if D-Bus disconnected
- Automatic reconnection with backoff
- CLI can communicate directly via socket as fallback

### A3: Microsoft Graph Single Point of Failure
- Offline queue for pending changes
- Circuit breaker pattern for API failures
- Resume from queue when connectivity restored

### A4: Delta Token Expiry
- Handle 410 Gone gracefully
- Full resync with progress notification
- Store last known good state
