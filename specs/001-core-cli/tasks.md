# Tasks: Core + CLI (Fase 1 - Fundamentos)

**Input**: Design documents from `/specs/001-core-cli/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Constitution requires Testing by Layers (80% core, 70% adapters). Unit tests included.

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US7)
- Include exact file paths in descriptions

## Path Conventions

```text
crates/
├── lnxdrive-core/src/       # Domain core
├── lnxdrive-graph/src/      # MS Graph adapter
├── lnxdrive-sync/src/       # Sync engine
├── lnxdrive-cache/src/      # SQLite repository
├── lnxdrive-ipc/src/        # D-Bus service
├── lnxdrive-cli/src/        # CLI binary
└── lnxdrive-daemon/src/     # Daemon binary
```

---

## Stage 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and workspace structure

- [x] T001 Create Cargo.toml workspace configuration at repository root with all crate members and shared dependencies (tokio, reqwest, serde, thiserror, anyhow, tracing, sqlx, clap, zbus, oauth2)
- [x] T002 [P] Create crates/lnxdrive-core/Cargo.toml with dependencies: uuid, chrono, serde, thiserror, async-trait
- [x] T003 [P] Create crates/lnxdrive-graph/Cargo.toml with dependencies: reqwest, oauth2, tokio, serde, thiserror, keyring, tracing
- [x] T004 [P] Create crates/lnxdrive-sync/Cargo.toml with dependencies: notify, tokio, tracing, thiserror
- [x] T005 [P] Create crates/lnxdrive-cache/Cargo.toml with dependencies: sqlx (sqlite, runtime-tokio), thiserror, tracing
- [x] T006 [P] Create crates/lnxdrive-ipc/Cargo.toml with dependencies: zbus, tokio, serde, thiserror, tracing
- [x] T007 [P] Create crates/lnxdrive-cli/Cargo.toml with dependencies: clap (derive), anyhow, tokio, serde_json, tracing-subscriber
- [x] T008 [P] Create crates/lnxdrive-daemon/Cargo.toml with dependencies: tokio, anyhow, tracing-subscriber, signal-hook
- [x] T009 Create .rustfmt.toml with project formatting rules (max_width=100, edition=2021, imports_granularity=Crate)
- [x] T010 [P] Create .clippy.toml with project linting configuration
- [x] T011 [P] Create rust-toolchain.toml specifying Rust 1.75+ minimum version
- [x] T012 Create config/default-config.yaml with all configuration sections from data-model.md (sync, rate_limiting, large_files, conflicts, logging, auth)
- [x] T013 [P] Create config/lnxdrive.service systemd user service unit file with Type=simple, RestartOnFailure, WantedBy=default.target
- [x] T014 Create .github/workflows/ci.yml with cargo build, cargo test, cargo clippy, cargo fmt --check, cargo audit
- [x] T015 [P] Create tests/integration/graph_mock/Cargo.toml for wiremock MS Graph mock server

**Checkpoint**: Workspace compiles with `cargo build --workspace`

---

## Stage 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### 2.1 Domain Core - Newtypes and Base Types

- [x] T016 [P] Create crates/lnxdrive-core/src/domain/mod.rs exporting all domain modules (sync_item, account, conflict, audit, session, newtypes, errors)
- [x] T017 [P] Create crates/lnxdrive-core/src/domain/newtypes.rs with UniqueId(Uuid), AccountId(Uuid), SessionId(Uuid), ConflictId(Uuid), AuditId(i64) implementing Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Display, FromStr
- [x] T018 [P] Create SyncPath newtype in crates/lnxdrive-core/src/domain/newtypes.rs with validation: absolute path, within sync_root, no path traversal, implements AsRef<Path>, From<PathBuf> with TryFrom validation
- [x] T019 [P] Create RemotePath newtype in crates/lnxdrive-core/src/domain/newtypes.rs for OneDrive path format with validation, leading slash required
- [x] T020 [P] Create RemoteId newtype in crates/lnxdrive-core/src/domain/newtypes.rs for OneDrive item ID (alphanumeric string)
- [x] T021 [P] Create FileHash newtype in crates/lnxdrive-core/src/domain/newtypes.rs for quickXorHash format with Base64 validation
- [x] T022 [P] Create DeltaToken newtype in crates/lnxdrive-core/src/domain/newtypes.rs for MS Graph delta token (opaque string)
- [x] T023 [P] Create Email newtype in crates/lnxdrive-core/src/domain/newtypes.rs with RFC 5322 email validation
- [x] T024 Create crates/lnxdrive-core/src/domain/errors.rs with DomainError enum variants: InvalidPath, InvalidEmail, InvalidHash, InvalidState, ValidationFailed using thiserror

### 2.2 Domain Core - Entities

- [x] T025 Create ItemState enum in crates/lnxdrive-core/src/domain/sync_item.rs with variants: Online, Hydrating, Hydrated, Modified, Conflicted, Error(String), Deleted with serde rename_all="snake_case"
- [x] T026 Create ItemMetadata struct in crates/lnxdrive-core/src/domain/sync_item.rs with fields: is_directory, mime_type, created_at, etag, permissions
- [x] T027 Create ErrorInfo struct in crates/lnxdrive-core/src/domain/sync_item.rs with fields: code, message, retry_count, last_attempt, next_retry
- [x] T028 Create SyncItem struct in crates/lnxdrive-core/src/domain/sync_item.rs with all fields from data-model.md: id, local_path, remote_id, remote_path, state, content_hash, local_hash, size_bytes, last_sync, last_modified_local, last_modified_remote, metadata, error_info
- [x] T029 Implement SyncItem::new() constructor in crates/lnxdrive-core/src/domain/sync_item.rs with validation of required fields
- [x] T030 Implement SyncItem state transition methods in crates/lnxdrive-core/src/domain/sync_item.rs: can_transition_to(), transition_to() returning Result with invalid transition error
- [x] T031 Create AccountState enum in crates/lnxdrive-core/src/domain/account.rs with variants: Active, TokenExpired, Suspended, Error(String)
- [x] T032 Create Account struct in crates/lnxdrive-core/src/domain/account.rs with all fields from data-model.md: id, email, display_name, onedrive_id, sync_root, quota_used, quota_total, delta_token, last_sync, state, created_at
- [x] T033 Implement Account::new() and Account::quota_percent() in crates/lnxdrive-core/src/domain/account.rs
- [x] T034 Create SessionStatus enum in crates/lnxdrive-core/src/domain/session.rs with variants: Running, Completed, Failed(String), Cancelled
- [x] T035 Create SessionError struct in crates/lnxdrive-core/src/domain/session.rs with fields: item_id, error_code, message, timestamp
- [x] T036 Create SyncSession struct in crates/lnxdrive-core/src/domain/session.rs with all fields from data-model.md
- [x] T037 Implement SyncSession::new(), complete(), fail(), add_error(), update_progress() in crates/lnxdrive-core/src/domain/session.rs
- [x] T038 Create AuditAction enum in crates/lnxdrive-core/src/domain/audit.rs with all variants from data-model.md: AuthLogin, AuthLogout, AuthRefresh, SyncStart, SyncComplete, FileUpload, FileDownload, FileDelete, ConflictDetected, ConflictResolved, Error, ConfigChange
- [x] T039 Create AuditResult enum in crates/lnxdrive-core/src/domain/audit.rs with variants: Success, Failed(ErrorCode, String)
- [x] T040 Create AuditEntry struct in crates/lnxdrive-core/src/domain/audit.rs with all fields from data-model.md
- [x] T041 Implement AuditEntry::new() in crates/lnxdrive-core/src/domain/audit.rs
- [x] T042 Create VersionInfo struct in crates/lnxdrive-core/src/domain/conflict.rs with fields: hash, size_bytes, modified_at, etag
- [x] T043 Create Resolution enum in crates/lnxdrive-core/src/domain/conflict.rs with variants: KeepLocal, KeepRemote, KeepBoth, Manual
- [x] T044 Create ResolutionSource enum in crates/lnxdrive-core/src/domain/conflict.rs with variants: User, Policy, System
- [x] T045 Create Conflict struct in crates/lnxdrive-core/src/domain/conflict.rs with all fields from data-model.md
- [x] T046 Implement Conflict::new(), resolve() in crates/lnxdrive-core/src/domain/conflict.rs

### 2.3 Domain Core - Ports (Traits)

- [x] T047 Create crates/lnxdrive-core/src/ports/mod.rs exporting all port traits
- [x] T048 Create AuthFlow enum in crates/lnxdrive-core/src/ports/cloud_provider.rs with variants: AuthorizationCodePKCE { app_id, redirect_uri, scopes }
- [x] T049 Create Tokens struct in crates/lnxdrive-core/src/ports/cloud_provider.rs with fields: access_token, refresh_token, expires_at
- [x] T050 Create DeltaResponse struct in crates/lnxdrive-core/src/ports/cloud_provider.rs with fields: items, next_link, delta_link
- [x] T051 Create DeltaItem struct in crates/lnxdrive-core/src/ports/cloud_provider.rs with fields from MS Graph API: id, name, path, size, hash, modified, deleted flag
- [x] T052 Create ICloudProvider trait in crates/lnxdrive-core/src/ports/cloud_provider.rs with async methods: authenticate(), refresh_tokens(), get_delta(), download_file(), upload_file(), upload_file_session(), get_metadata(), get_user_info()
- [x] T053 Create ItemFilter struct in crates/lnxdrive-core/src/ports/state_repository.rs with optional fields: account_id, state, path_prefix, modified_since
- [x] T054 Create IStateRepository trait in crates/lnxdrive-core/src/ports/state_repository.rs with async methods: save_item(), get_item(), get_item_by_path(), query_items(), delete_item(), save_account(), get_account(), save_session(), get_session(), save_audit(), get_audit_trail(), get_audit_since()
- [x] T055 Create FileSystemState struct in crates/lnxdrive-core/src/ports/local_filesystem.rs with fields: exists, is_file, size, modified, is_locked
- [x] T056 Create IFileObserver trait in crates/lnxdrive-core/src/ports/local_filesystem.rs with methods: on_created(), on_modified(), on_deleted(), on_renamed()
- [x] T057 Create WatchHandle struct in crates/lnxdrive-core/src/ports/local_filesystem.rs as RAII wrapper for watch registration
- [x] T058 Create ILocalFileSystem trait in crates/lnxdrive-core/src/ports/local_filesystem.rs with async methods: create_placeholder(), read_file(), write_file(), delete_file(), get_state(), compute_hash(), watch(), unwatch()
- [x] T059 Create Notification struct in crates/lnxdrive-core/src/ports/notification.rs with fields: title, body, priority, category
- [x] T060 Create INotificationService trait in crates/lnxdrive-core/src/ports/notification.rs with methods: notify(), show_progress(), clear_progress()

### 2.4 Domain Core - Use Cases

- [x] T061 Create crates/lnxdrive-core/src/usecases/mod.rs exporting all use case modules
- [x] T062 Create AuthenticateUseCase struct in crates/lnxdrive-core/src/usecases/authenticate.rs with dependencies: cloud_provider, state_repository
- [x] T063 Implement AuthenticateUseCase::login() in crates/lnxdrive-core/src/usecases/authenticate.rs: initiate OAuth2 PKCE flow, store tokens, create Account entity, save to repository, create AuditEntry
- [x] T064 Implement AuthenticateUseCase::logout() in crates/lnxdrive-core/src/usecases/authenticate.rs: clear tokens from keyring, update Account state, create AuditEntry
- [x] T065 Implement AuthenticateUseCase::refresh_if_needed() in crates/lnxdrive-core/src/usecases/authenticate.rs: check token expiry, refresh if <5min remaining, update Account
- [x] T066 Implement AuthenticateUseCase::get_status() in crates/lnxdrive-core/src/usecases/authenticate.rs: return current auth status with account info
- [x] T067 Create SyncFileUseCase struct in crates/lnxdrive-core/src/usecases/sync_file.rs with dependencies: cloud_provider, state_repository, local_filesystem
- [x] T068 Implement SyncFileUseCase::sync_single() in crates/lnxdrive-core/src/usecases/sync_file.rs: compare hashes, determine direction, perform transfer, update state
- [x] T069 Implement SyncFileUseCase::upload() in crates/lnxdrive-core/src/usecases/sync_file.rs: read local file, choose PUT or session based on size, verify hash, update SyncItem
- [x] T070 Implement SyncFileUseCase::download() in crates/lnxdrive-core/src/usecases/sync_file.rs: stream from cloud, write to local, verify hash, update SyncItem
- [x] T071 Create QueryDeltaUseCase struct in crates/lnxdrive-core/src/usecases/query_delta.rs with dependencies: cloud_provider, state_repository
- [x] T072 Implement QueryDeltaUseCase::execute() in crates/lnxdrive-core/src/usecases/query_delta.rs: get delta from API, process items, handle pagination, save new delta token
- [x] T073 Implement QueryDeltaUseCase::handle_delta_item() in crates/lnxdrive-core/src/usecases/query_delta.rs: create/update/delete SyncItem based on delta item
- [x] T074 Create ExplainFailureUseCase struct in crates/lnxdrive-core/src/usecases/explain_failure.rs with dependencies: state_repository
- [x] T075 Implement ExplainFailureUseCase::explain() in crates/lnxdrive-core/src/usecases/explain_failure.rs: get item state, get audit trail, generate human-readable explanation with suggestions

### 2.5 Domain Core - lib.rs and Unit Tests

- [x] T076 Create crates/lnxdrive-core/src/lib.rs exporting domain, ports, usecases modules with public API
- [x] T077 [P] Create unit tests for newtypes validation in crates/lnxdrive-core/src/domain/newtypes.rs: test valid/invalid SyncPath, Email, FileHash
- [x] T078 [P] Create unit tests for SyncItem state transitions in crates/lnxdrive-core/src/domain/sync_item.rs: test valid transitions, test invalid transitions return error
- [x] T079 [P] Create unit tests for Account quota calculation in crates/lnxdrive-core/src/domain/account.rs
- [x] T080 [P] Create unit tests for SyncSession progress tracking in crates/lnxdrive-core/src/domain/session.rs

### 2.6 SQLite Repository (lnxdrive-cache)

- [x] T081 Create crates/lnxdrive-cache/src/migrations/20260203_initial.sql with all tables from data-model.md: accounts, sync_items, sync_sessions, audit_log, conflicts, config
- [x] T082 Create DatabasePool struct in crates/lnxdrive-cache/src/lib.rs wrapping sqlx::SqlitePool with connection configuration
- [x] T083 Implement DatabasePool::new() in crates/lnxdrive-cache/src/lib.rs: create pool, run migrations, enable WAL mode
- [x] T084 Create SqliteStateRepository struct in crates/lnxdrive-cache/src/repository.rs implementing IStateRepository
- [x] T085 Implement SqliteStateRepository::save_item() in crates/lnxdrive-cache/src/repository.rs with UPSERT query
- [x] T086 Implement SqliteStateRepository::get_item() in crates/lnxdrive-cache/src/repository.rs
- [x] T087 Implement SqliteStateRepository::get_item_by_path() in crates/lnxdrive-cache/src/repository.rs
- [x] T088 Implement SqliteStateRepository::query_items() in crates/lnxdrive-cache/src/repository.rs with dynamic filter building
- [x] T089 Implement SqliteStateRepository::delete_item() in crates/lnxdrive-cache/src/repository.rs
- [x] T090 Implement SqliteStateRepository::save_account() in crates/lnxdrive-cache/src/repository.rs with UPSERT
- [x] T091 Implement SqliteStateRepository::get_account() in crates/lnxdrive-cache/src/repository.rs
- [x] T092 Implement SqliteStateRepository::save_session() in crates/lnxdrive-cache/src/repository.rs
- [x] T093 Implement SqliteStateRepository::get_session() in crates/lnxdrive-cache/src/repository.rs
- [x] T094 Implement SqliteStateRepository::save_audit() in crates/lnxdrive-cache/src/repository.rs
- [x] T095 Implement SqliteStateRepository::get_audit_trail() in crates/lnxdrive-cache/src/repository.rs for specific item
- [x] T096 Implement SqliteStateRepository::get_audit_since() in crates/lnxdrive-cache/src/repository.rs with timestamp filter
- [x] T097 Create crates/lnxdrive-cache/src/lib.rs exporting DatabasePool, SqliteStateRepository
- [x] T098 Create integration tests in crates/lnxdrive-cache/tests/repository_tests.rs for all repository methods using in-memory SQLite

### 2.7 Configuration

- [x] T099 Create Config struct in crates/lnxdrive-core/src/config.rs with all sections from data-model.md: SyncConfig, RateLimitingConfig, LargeFilesConfig, ConflictsConfig, LoggingConfig, AuthConfig
- [x] T100 Implement Config::load() in crates/lnxdrive-core/src/config.rs: read YAML from path, merge with defaults, validate
- [x] T101 Implement Config::default() in crates/lnxdrive-core/src/config.rs with sensible defaults from data-model.md
- [x] T102 Implement Config::validate() in crates/lnxdrive-core/src/config.rs: check paths exist, values in range, return Vec<ValidationError>
- [x] T103 Create ConfigBuilder struct in crates/lnxdrive-core/src/config.rs following Builder pattern for programmatic configuration
- [x] T104 [P] Create unit tests for Config loading and validation in crates/lnxdrive-core/src/config.rs

### 2.8 Error Handling and Logging Infrastructure

- [x] T105 Create GraphError enum in crates/lnxdrive-graph/src/lib.rs with variants: Unauthorized, Forbidden, NotFound, Conflict, TooManyRequests{retry_after}, ServerError, NetworkError, TokenExpired
- [x] T106 Create SyncError enum in crates/lnxdrive-sync/src/lib.rs with variants: IoError, FileLocked, DiskFull, PermissionDenied, PathNotFound
- [x] T107 Create CacheError enum in crates/lnxdrive-cache/src/lib.rs with variants: ConnectionFailed, QueryFailed, MigrationFailed
- [x] T108 Create tracing configuration in crates/lnxdrive-cli/src/main.rs: setup tracing-subscriber with env_filter, file appender, JSON formatting option

**Checkpoint**: Foundation ready - `cargo build --workspace` succeeds, all ports defined, repository functional

---

## Stage 3: User Story 1 - Primera Autenticacion con OneDrive (Priority: P1) 🎯 MVP

**Goal**: User can authenticate with OneDrive via OAuth2 PKCE and store tokens securely

**Independent Test**: Run `lnxdrive auth login`, complete browser flow, verify tokens stored in keyring

### Implementation for User Story 1

- [x] T109 [US1] Create OAuth2Config struct in crates/lnxdrive-graph/src/auth.rs with fields: app_id, redirect_uri (http://127.0.0.1:8400/callback), scopes (Files.ReadWrite, offline_access, User.Read)
- [x] T110 [US1] Create KeyringTokenStorage struct in crates/lnxdrive-graph/src/auth.rs using keyring crate with service="lnxdrive", username=account_email
- [x] T111 [US1] Implement KeyringTokenStorage::store() in crates/lnxdrive-graph/src/auth.rs: serialize Tokens to JSON, store in keyring
- [x] T112 [US1] Implement KeyringTokenStorage::load() in crates/lnxdrive-graph/src/auth.rs: load from keyring, deserialize JSON
- [x] T113 [US1] Implement KeyringTokenStorage::clear() in crates/lnxdrive-graph/src/auth.rs: delete entry from keyring
- [x] T114 [US1] Create LocalCallbackServer struct in crates/lnxdrive-graph/src/auth.rs using hyper to listen on localhost:8400 for OAuth callback
- [x] T115 [US1] Implement LocalCallbackServer::start() in crates/lnxdrive-graph/src/auth.rs: bind to port, wait for callback with code parameter, return code
- [x] T116 [US1] Create PKCEFlow struct in crates/lnxdrive-graph/src/auth.rs using oauth2 crate with PKCE verifier/challenge generation
- [x] T117 [US1] Implement PKCEFlow::generate_auth_url() in crates/lnxdrive-graph/src/auth.rs: build authorization URL with PKCE challenge, state, scopes
- [x] T118 [US1] Implement PKCEFlow::exchange_code() in crates/lnxdrive-graph/src/auth.rs: exchange authorization code for tokens using PKCE verifier
- [x] T119 [US1] Implement PKCEFlow::refresh_token() in crates/lnxdrive-graph/src/auth.rs: use refresh token to get new access token
- [x] T120 [US1] Create GraphAuthAdapter struct in crates/lnxdrive-graph/src/auth.rs combining PKCEFlow, LocalCallbackServer, KeyringTokenStorage
- [x] T121 [US1] Implement full login flow in GraphAuthAdapter::login() in crates/lnxdrive-graph/src/auth.rs: generate URL, open browser (webbrowser crate), start callback server, exchange code, store tokens, get user info
- [x] T122 [US1] Create GraphClient struct in crates/lnxdrive-graph/src/client.rs with reqwest::Client, base_url, auth_adapter reference
- [x] T123 [US1] Implement GraphClient::get_user_info() in crates/lnxdrive-graph/src/client.rs: GET /me returning email, display_name, id
- [x] T124 [US1] Implement GraphClient::get_drive_quota() in crates/lnxdrive-graph/src/client.rs: GET /me/drive returning quota.used, quota.total
- [x] T125 [US1] Implement GraphClient::ensure_authenticated() in crates/lnxdrive-graph/src/client.rs: check token validity, refresh if needed, add Authorization header
- [x] T126 [US1] Create crates/lnxdrive-graph/src/lib.rs exporting GraphClient, GraphAuthAdapter, GraphError
- [x] T127 [US1] Create AuthCommand struct in crates/lnxdrive-cli/src/commands/auth.rs with clap subcommands: login, logout, status
- [x] T128 [US1] Implement auth login subcommand in crates/lnxdrive-cli/src/commands/auth.rs: parse --app-id option, call AuthenticateUseCase::login(), format output
- [x] T129 [US1] Implement auth logout subcommand in crates/lnxdrive-cli/src/commands/auth.rs: call AuthenticateUseCase::logout(), format output
- [x] T130 [US1] Implement auth status subcommand in crates/lnxdrive-cli/src/commands/auth.rs: call AuthenticateUseCase::get_status(), format output with quota info
- [x] T131 [US1] Create OutputFormatter trait in crates/lnxdrive-cli/src/output.rs with methods: success(), error(), progress(), table()
- [x] T132 [US1] Implement HumanFormatter in crates/lnxdrive-cli/src/output.rs: format with colors, checkmarks, indentation
- [x] T133 [US1] Implement JsonFormatter in crates/lnxdrive-cli/src/output.rs: serialize structs to JSON
- [x] T134 [US1] Create CLI main entry in crates/lnxdrive-cli/src/main.rs with clap App definition, global options (--json, --verbose, --config), subcommand routing
- [x] T135 [US1] Add auth integration test in tests/integration/test_auth.rs: mock OAuth server, test full login flow, verify token storage

**Checkpoint**: User can run `lnxdrive auth login`, authenticate in browser, see success message with email and quota

---

## Stage 4: User Story 2 - Sincronizacion Inicial de Archivos (Priority: P1) 🎯 MVP

**Goal**: User can sync files bidirectionally between local folder and OneDrive

**Independent Test**: Create local file, run `lnxdrive sync`, verify file appears in OneDrive web

### Implementation for User Story 2

- [x] T136 [US2] Implement GraphClient::get_delta() in crates/lnxdrive-graph/src/delta.rs: GET /me/drive/root/delta with optional token, handle pagination via @odata.nextLink
- [x] T137 [US2] Implement GraphClient::get_delta_page() in crates/lnxdrive-graph/src/delta.rs: follow nextLink for subsequent pages
- [x] T138 [US2] Create DeltaParser in crates/lnxdrive-graph/src/delta.rs: parse MS Graph response into DeltaItem structs, extract deltaLink
- [x] T139 [US2] Implement GraphClient::download_file() in crates/lnxdrive-graph/src/client.rs: GET /me/drive/items/{id}/content with streaming response
- [x] T140 [US2] Implement GraphClient::upload_small() in crates/lnxdrive-graph/src/upload.rs: PUT /me/drive/root:/{path}:/content for files <4MB
- [x] T141 [US2] Implement GraphClient::create_upload_session() in crates/lnxdrive-graph/src/upload.rs: POST /me/drive/root:/{path}:/createUploadSession
- [x] T142 [US2] Implement GraphClient::upload_chunk() in crates/lnxdrive-graph/src/upload.rs: PUT to upload URL with Content-Range header for 10MB chunks
- [x] T143 [US2] Implement GraphClient::upload_large() in crates/lnxdrive-graph/src/upload.rs: create session, upload chunks with progress callback, handle resume
- [x] T144 [US2] Create LocalFileSystemAdapter struct in crates/lnxdrive-sync/src/filesystem.rs implementing ILocalFileSystem
- [x] T145 [US2] Implement LocalFileSystemAdapter::read_file() in crates/lnxdrive-sync/src/filesystem.rs: async file read with streaming
- [x] T146 [US2] Implement LocalFileSystemAdapter::write_file() in crates/lnxdrive-sync/src/filesystem.rs: async file write with atomic rename
- [x] T147 [US2] Implement LocalFileSystemAdapter::delete_file() in crates/lnxdrive-sync/src/filesystem.rs: remove file or directory recursively
- [x] T148 [US2] Implement LocalFileSystemAdapter::get_state() in crates/lnxdrive-sync/src/filesystem.rs: stat file, check if locked (try exclusive open)
- [x] T149 [US2] Implement LocalFileSystemAdapter::compute_hash() in crates/lnxdrive-sync/src/filesystem.rs: compute quickXorHash matching OneDrive format
- [x] T150 [US2] Create GraphCloudProvider struct in crates/lnxdrive-graph/src/lib.rs implementing ICloudProvider, wrapping GraphClient
- [x] T151 [US2] Create SyncEngine struct in crates/lnxdrive-sync/src/engine.rs with dependencies: cloud_provider, state_repository, local_filesystem, config
- [x] T152 [US2] Implement SyncEngine::sync() in crates/lnxdrive-sync/src/engine.rs: create SyncSession, query delta, process items, update delta token, complete session
- [x] T153 [US2] Implement SyncEngine::process_delta_item() in crates/lnxdrive-sync/src/engine.rs: determine if create/update/delete, call appropriate handler
- [x] T154 [US2] Implement SyncEngine::handle_remote_create() in crates/lnxdrive-sync/src/engine.rs: create placeholder or download file based on config
- [x] T155 [US2] Implement SyncEngine::handle_remote_update() in crates/lnxdrive-sync/src/engine.rs: compare hashes, download if different
- [x] T156 [US2] Implement SyncEngine::handle_remote_delete() in crates/lnxdrive-sync/src/engine.rs: delete local file/directory
- [x] T157 [US2] Implement SyncEngine::scan_local_changes() in crates/lnxdrive-sync/src/engine.rs: walk sync directory, compare with stored items, find new/modified/deleted
- [x] T158 [US2] Implement SyncEngine::handle_local_create() in crates/lnxdrive-sync/src/engine.rs: upload file, create SyncItem
- [x] T159 [US2] Implement SyncEngine::handle_local_update() in crates/lnxdrive-sync/src/engine.rs: compare hashes, upload if different
- [x] T160 [US2] Implement SyncEngine::handle_local_delete() in crates/lnxdrive-sync/src/engine.rs: delete from OneDrive (move to trash)
- [x] T161 [US2] Implement retry logic in SyncEngine in crates/lnxdrive-sync/src/engine.rs: exponential backoff for transient errors, max 5 retries
- [x] T162 [US2] Create SyncCommand struct in crates/lnxdrive-cli/src/commands/sync.rs with clap options: --full, --dry-run
- [x] T163 [US2] Implement sync command in crates/lnxdrive-cli/src/commands/sync.rs: initialize SyncEngine, call sync(), display progress, format results
- [x] T164 [US2] Add progress display to sync command in crates/lnxdrive-cli/src/commands/sync.rs: show current file, progress bar, speed
- [x] T165 [US2] Create crates/lnxdrive-sync/src/lib.rs exporting SyncEngine, LocalFileSystemAdapter
- [x] T166 [US2] Add sync integration test in tests/integration/test_sync.rs: mock Graph server, test upload/download flow

**Checkpoint**: User can run `lnxdrive sync`, see files downloaded/uploaded, verify bidirectional sync works

---

## Stage 5: User Story 3 - Sincronizacion Delta Incremental (Priority: P2)

**Goal**: System efficiently syncs only changed files using delta tokens

**Independent Test**: Modify single file after initial sync, verify only that file transfers

### Implementation for User Story 3

- [x] T167 [US3] Implement delta token persistence in SyncEngine in crates/lnxdrive-sync/src/engine.rs: save to Account after successful sync
- [x] T168 [US3] Implement delta token usage in SyncEngine::sync() in crates/lnxdrive-sync/src/engine.rs: pass token to get_delta(), handle 410 Gone by resetting
- [x] T169 [US3] Implement 410 Gone handler in crates/lnxdrive-graph/src/delta.rs: detect expired token, return specific error variant
- [x] T170 [US3] Implement full resync fallback in SyncEngine in crates/lnxdrive-sync/src/engine.rs: on 410 Gone, clear token, notify user, start fresh delta
- [x] T171 [US3] Add delta efficiency metrics to SyncSession in crates/lnxdrive-core/src/domain/session.rs: items_checked vs items_synced ratio
- [x] T172 [US3] Implement local change detection optimization in SyncEngine in crates/lnxdrive-sync/src/engine.rs: only check files modified since last_sync timestamp
- [x] T173 [US3] Add delta sync integration test in tests/integration/test_delta.rs: initial sync, modify one file, verify only one transfer

**Checkpoint**: Incremental sync works efficiently, only changed files transfer

---

## Stage 6: User Story 4 - Observacion de Cambios Locales en Tiempo Real (Priority: P2)

**Goal**: System automatically detects local file changes and queues them for sync

**Independent Test**: With daemon running, create file, verify it syncs without manual command

### Implementation for User Story 4

- [x] T174 [US4] Create FileWatcher struct in crates/lnxdrive-sync/src/watcher.rs using notify crate with inotify backend
- [x] T175 [US4] Implement FileWatcher::new() in crates/lnxdrive-sync/src/watcher.rs: configure RecommendedWatcher with debounce interval from config
- [x] T176 [US4] Implement FileWatcher::watch() in crates/lnxdrive-sync/src/watcher.rs: add path to watcher recursively, return WatchHandle
- [x] T177 [US4] Implement FileWatcher::unwatch() in crates/lnxdrive-sync/src/watcher.rs: remove path from watcher
- [x] T178 [US4] Create ChangeEvent enum in crates/lnxdrive-sync/src/watcher.rs with variants: Created(path), Modified(path), Deleted(path), Renamed(old, new)
- [x] T179 [US4] Implement event mapping in FileWatcher in crates/lnxdrive-sync/src/watcher.rs: convert notify events to ChangeEvent
- [x] T180 [US4] Create DebouncedChangeQueue in crates/lnxdrive-sync/src/watcher.rs: aggregate rapid changes to same file, emit after debounce_delay
- [x] T181 [US4] Implement DebouncedChangeQueue::push() in crates/lnxdrive-sync/src/watcher.rs: reset timer for path, store latest change type
- [x] T182 [US4] Implement DebouncedChangeQueue::poll() in crates/lnxdrive-sync/src/watcher.rs: return changes older than debounce_delay
- [x] T183 [US4] Create SyncScheduler struct in crates/lnxdrive-sync/src/scheduler.rs: manages pending changes queue, triggers sync engine
- [x] T184 [US4] Implement SyncScheduler::enqueue() in crates/lnxdrive-sync/src/scheduler.rs: add change to queue, prioritize user-initiated over automatic
- [x] T185 [US4] Implement SyncScheduler::run() in crates/lnxdrive-sync/src/scheduler.rs: async loop processing queue, respecting rate limits
- [x] T186 [US4] Integrate FileWatcher with SyncEngine in crates/lnxdrive-sync/src/engine.rs: receive events from watcher, enqueue to scheduler
- [x] T187 [US4] Add file stability check in crates/lnxdrive-sync/src/watcher.rs: detect if file is still being written (size changing)
- [x] T188 [US4] Add watcher unit tests in crates/lnxdrive-sync/src/watcher.rs: test debouncing, test event coalescing

**Checkpoint**: Local changes detected automatically, debounced, and queued for sync

---

## Stage 7: User Story 5 - CLI para Estado y Diagnostico (Priority: P2)

**Goal**: User can query sync status and get explanations via CLI

**Independent Test**: Run `lnxdrive status` and `lnxdrive explain <path>` to see meaningful output

### Implementation for User Story 5

- [x] T189 [US5] Create StatusCommand struct in crates/lnxdrive-cli/src/commands/status.rs with optional path argument
- [x] T190 [US5] Implement global status display in crates/lnxdrive-cli/src/commands/status.rs: query repository for counts by state, last sync time
- [x] T191 [US5] Implement per-file status display in crates/lnxdrive-cli/src/commands/status.rs: show item state, local/remote timestamps, hash match
- [x] T192 [US5] Implement pending items list in status command in crates/lnxdrive-cli/src/commands/status.rs: show files in Modified/Hydrating state
- [x] T193 [US5] Implement error items list in status command in crates/lnxdrive-cli/src/commands/status.rs: show files in Error state with reasons
- [x] T194 [US5] Create ExplainCommand struct in crates/lnxdrive-cli/src/commands/explain.rs with required path argument
- [x] T195 [US5] Implement explain command in crates/lnxdrive-cli/src/commands/explain.rs: call ExplainFailureUseCase, format explanation with suggestions
- [x] T196 [US5] Implement explanation generation for common states in crates/lnxdrive-core/src/usecases/explain_failure.rs: Online, Hydrated, Modified, Error variants
- [x] T197 [US5] Implement suggestion generation in crates/lnxdrive-core/src/usecases/explain_failure.rs: based on error type, return actionable suggestions
- [x] T198 [US5] Implement history display in explain command in crates/lnxdrive-cli/src/commands/explain.rs: show recent audit entries for item
- [x] T199 [US5] Create AuditCommand struct in crates/lnxdrive-cli/src/commands/audit.rs with options: --since, --action, --limit, --path
- [x] T200 [US5] Implement audit command in crates/lnxdrive-cli/src/commands/audit.rs: query repository with filters, format entries in table
- [x] T201 [US5] Implement time parsing in audit command in crates/lnxdrive-cli/src/commands/audit.rs: handle "1 hour ago", "2024-01-01" formats using chrono

**Checkpoint**: User can get meaningful status and diagnostic information via CLI

---

## Stage 8: User Story 6 - Rate Limiting y Respeto de Cuotas (Priority: P3)

**Goal**: System respects API rate limits and adapts to throttling

**Independent Test**: Perform many operations, verify no 429 errors during normal operation

### Implementation for User Story 6

- [x] T202 [US6] Create TokenBucket struct in crates/lnxdrive-graph/src/rate_limit.rs with fields: capacity, tokens, refill_rate, last_refill
- [x] T203 [US6] Implement TokenBucket::try_acquire() in crates/lnxdrive-graph/src/rate_limit.rs: atomic CAS loop for thread-safe token acquisition
- [x] T204 [US6] Implement TokenBucket::refill() in crates/lnxdrive-graph/src/rate_limit.rs: calculate tokens to add based on elapsed time
- [x] T205 [US6] Create RateLimitGuard struct in crates/lnxdrive-graph/src/rate_limit.rs: RAII guard releasing token on drop
- [x] T206 [US6] Create AdaptiveRateLimiter struct in crates/lnxdrive-graph/src/rate_limit.rs with per-endpoint TokenBuckets, adaptation metrics
- [x] T207 [US6] Implement AdaptiveRateLimiter::acquire() in crates/lnxdrive-graph/src/rate_limit.rs: get bucket for endpoint, wait if needed, return guard
- [x] T208 [US6] Implement AdaptiveRateLimiter::on_success() in crates/lnxdrive-graph/src/rate_limit.rs: record success, consider increasing limits after window
- [x] T209 [US6] Implement AdaptiveRateLimiter::on_throttle() in crates/lnxdrive-graph/src/rate_limit.rs: reduce limits by 50%, record for metrics
- [x] T210 [US6] Integrate AdaptiveRateLimiter into GraphClient in crates/lnxdrive-graph/src/client.rs: wrap all API calls with rate limiter
- [x] T211 [US6] Implement 429 response handling in GraphClient in crates/lnxdrive-graph/src/client.rs: parse Retry-After header, notify rate limiter, wait, retry
- [x] T212 [US6] Create BulkMode configuration in crates/lnxdrive-sync/src/engine.rs: detect initial sync (>1000 files), reduce concurrency, increase delays
- [x] T213 [US6] Add rate limiting unit tests in crates/lnxdrive-graph/src/rate_limit.rs: test token acquisition, test adaptation, test concurrent access

**Checkpoint**: System respects rate limits, adapts to throttling, no 429 errors in normal use

---

## Stage 9: User Story 7 - Servicio Daemon Persistente (Priority: P3)

**Goal**: Sync runs as background service, auto-starts on login

**Independent Test**: Restart session, verify daemon starts and syncs automatically

### Implementation for User Story 7

- [x] T214 [US7] Create DaemonService struct in crates/lnxdrive-daemon/src/main.rs with SyncEngine, FileWatcher, SyncScheduler
- [x] T215 [US7] Implement DaemonService::run() in crates/lnxdrive-daemon/src/main.rs: async main loop, initialize components, start watching
- [x] T216 [US7] Implement periodic remote polling in DaemonService in crates/lnxdrive-daemon/src/main.rs: tokio::interval based on poll_interval config
- [x] T217 [US7] Implement graceful shutdown in DaemonService in crates/lnxdrive-daemon/src/main.rs: handle SIGTERM/SIGINT, complete pending operations, exit cleanly
- [x] T218 [US7] Implement CancellationToken propagation in crates/lnxdrive-daemon/src/main.rs: pass token to all async tasks for coordinated shutdown
- [x] T219 [US7] Create DbusService struct in crates/lnxdrive-ipc/src/service.rs implementing D-Bus interfaces from contracts/dbus-interface.xml
- [x] T220 [US7] Implement SyncController D-Bus interface in crates/lnxdrive-ipc/src/interface.rs: StartSync, PauseSync, GetStatus methods
- [x] T221 [US7] Implement Account D-Bus interface in crates/lnxdrive-ipc/src/interface.rs: GetInfo, CheckAuth methods
- [x] T222 [US7] Implement Conflicts D-Bus interface in crates/lnxdrive-ipc/src/interface.rs: List, Resolve methods
- [x] T223 [US7] Implement D-Bus signals in crates/lnxdrive-ipc/src/interface.rs: StateChanged, ItemStateChanged, SyncProgress, Error
- [x] T224 [US7] Integrate DbusService into DaemonService in crates/lnxdrive-daemon/src/main.rs: start D-Bus service, connect to engine events
- [x] T225 [US7] Create crates/lnxdrive-ipc/src/lib.rs exporting DbusService
- [x] T226 [US7] Create DaemonCommand struct in crates/lnxdrive-cli/src/commands/daemon.rs with subcommands: start, stop, status, restart
- [x] T227 [US7] Implement daemon start command in crates/lnxdrive-cli/src/commands/daemon.rs: systemctl --user start lnxdrive
- [x] T228 [US7] Implement daemon stop command in crates/lnxdrive-cli/src/commands/daemon.rs: systemctl --user stop lnxdrive
- [x] T229 [US7] Implement daemon status command in crates/lnxdrive-cli/src/commands/daemon.rs: query D-Bus for status, show PID, uptime, memory
- [x] T230 [US7] Implement daemon restart command in crates/lnxdrive-cli/src/commands/daemon.rs: systemctl --user restart lnxdrive
- [x] T231 [US7] Add single instance lock in DaemonService in crates/lnxdrive-daemon/src/main.rs: use PID file or D-Bus name registration
- [x] T232 [US7] Update config/lnxdrive.service with proper ExecStart path, Environment variables, Documentation link

**Checkpoint**: Daemon runs as systemd service, auto-starts on login, controllable via CLI

---

## Stage 10: Additional CLI Commands

**Purpose**: Complete remaining CLI commands for config and conflicts

### Implementation

- [x] T233 Create ConfigCommand struct in crates/lnxdrive-cli/src/commands/config.rs with subcommands: show, set, validate
- [x] T234 Implement config show command in crates/lnxdrive-cli/src/commands/config.rs: display current configuration
- [x] T235 Implement config set command in crates/lnxdrive-cli/src/commands/config.rs: update single key, validate, save
- [x] T236 Implement config validate command in crates/lnxdrive-cli/src/commands/config.rs: load config, run validation, report errors
- [x] T237 Create ConflictsCommand struct in crates/lnxdrive-cli/src/commands/conflicts.rs with subcommands: list, resolve, preview
- [x] T238 Implement conflicts list command in crates/lnxdrive-cli/src/commands/conflicts.rs: query unresolved conflicts, display table
- [x] T239 Implement conflicts resolve command in crates/lnxdrive-cli/src/commands/conflicts.rs: apply resolution strategy, update item state
- [x] T240 Implement conflicts preview command in crates/lnxdrive-cli/src/commands/conflicts.rs: show diff between local/remote versions
- [x] T241 Update CLI main in crates/lnxdrive-cli/src/main.rs to include all command submodules

**Checkpoint**: All CLI commands from contract implemented and functional

---

## Stage 11: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T242 Add --version flag to CLI in crates/lnxdrive-cli/src/main.rs using clap version macro
- [x] T243 Add shell completions generation in crates/lnxdrive-cli/src/main.rs using clap_complete for bash/zsh/fish
- [x] T244 Implement consistent error formatting across all CLI commands in crates/lnxdrive-cli/src/output.rs
- [x] T245 Add tracing spans to all major operations for observability in crates/lnxdrive-sync/src/engine.rs
- [x] T246 [P] Add documentation comments (///doc) to all public types and functions in lnxdrive-core
- [x] T247 [P] Add documentation comments to all public types and functions in lnxdrive-graph
- [x] T248 [P] Add documentation comments to all public types and functions in lnxdrive-sync
- [x] T249 Create README.md in crates/ directory explaining crate structure and dependencies
- [x] T250 Run cargo clippy --workspace -- -D warnings and fix all issues
- [x] T251 Run cargo fmt --all and verify formatting
- [x] T252 Run cargo audit and address any security advisories
- [x] T253 Verify all unit tests pass: cargo test --workspace
- [x] T254 Create quickstart validation script in scripts/validate-quickstart.sh to verify setup steps from quickstart.md

---

## Dependencies & Execution Order

### Stage Dependencies

```
Stage 1: Setup
    ↓
Stage 2: Foundational (BLOCKS all user stories)
    ↓
    ├─→ Stage 3: US1 - Auth (P1) 🎯
    │       ↓
    ├─→ Stage 4: US2 - Initial Sync (P1, depends on US1 for auth)
    │       ↓
    ├─→ Stage 5: US3 - Delta Sync (P2, builds on US2)
    │       ↓
    ├─→ Stage 6: US4 - File Watching (P2, can parallel with US3)
    │       ↓
    ├─→ Stage 7: US5 - Status/Explain (P2, can parallel with US3/US4)
    │       ↓
    ├─→ Stage 8: US6 - Rate Limiting (P3, can parallel with US5)
    │       ↓
    ├─→ Stage 9: US7 - Daemon (P3, depends on US4 for watcher)
    │       ↓
    └─→ Stage 10: Additional CLI
            ↓
        Stage 11: Polish
```

### User Story Dependencies

- **US1 (Auth)**: Foundation only - can start immediately after Stage 2
- **US2 (Initial Sync)**: Requires US1 (needs authentication)
- **US3 (Delta Sync)**: Requires US2 (builds on sync infrastructure)
- **US4 (File Watching)**: Requires US2 (needs sync engine)
- **US5 (Status/Explain)**: Requires US2 (needs sync items in repository)
- **US6 (Rate Limiting)**: Requires US2 (integrates with GraphClient)
- **US7 (Daemon)**: Requires US4 (needs file watcher)

### Parallel Opportunities

**Stage 1 (all [P] tasks can run in parallel):**
```
T002-T008 - All crate Cargo.toml files
T009-T011 - Tooling config files
T013-T015 - CI and test infrastructure
```

**Stage 2 Newtypes (all [P]):**
```
T017-T023 - All newtypes can be implemented in parallel
```

**Stage 2 Entities (partial parallel):**
```
T025-T030 - SyncItem
T031-T033 - Account         } Can run in parallel
T034-T037 - SyncSession     }
T038-T041 - AuditEntry      }
T042-T046 - Conflict        }
```

**Stage 2 Unit Tests (all [P]):**
```
T077-T080 - All unit tests for domain entities
```

---

## Implementation Strategy

### MVP First (User Stories 1-2 Only)

1. Complete Stage 1: Setup
2. Complete Stage 2: Foundational (CRITICAL - blocks all stories)
3. Complete Stage 3: User Story 1 (Auth)
4. Complete Stage 4: User Story 2 (Initial Sync)
5. **STOP and VALIDATE**: Test auth + sync works end-to-end
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 (Auth) → Test independently → Demo
3. Add US2 (Initial Sync) → Test independently → Demo (MVP!)
4. Add US3 (Delta) → More efficient sync
5. Add US4 (Watching) → Real-time sync
6. Add US5 (Status/Explain) → Better UX
7. Add US6 (Rate Limiting) → Production ready
8. Add US7 (Daemon) → Full product

---

## Summary

| Stage | Tasks | User Story | Priority |
|-------|-------|------------|----------|
| 1 | T001-T015 (15) | Setup | - |
| 2 | T016-T108 (93) | Foundational | - |
| 3 | T109-T135 (27) | US1: Auth | P1 |
| 4 | T136-T166 (31) | US2: Initial Sync | P1 |
| 5 | T167-T173 (7) | US3: Delta Sync | P2 |
| 6 | T174-T188 (15) | US4: File Watching | P2 |
| 7 | T189-T201 (13) | US5: Status/Explain | P2 |
| 8 | T202-T213 (12) | US6: Rate Limiting | P3 |
| 9 | T214-T232 (19) | US7: Daemon | P3 |
| 10 | T233-T241 (9) | Additional CLI | - |
| 11 | T242-T254 (13) | Polish | - |

**Total Tasks**: 254

**MVP Scope** (Stages 1-4): 166 tasks for auth + bidirectional sync
**Full Scope** (All Stages): 254 tasks for complete feature

---

## Notes

- [P] tasks = different files, no dependencies
- [USn] label maps task to specific user story
- Each user story independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Constitution requires: 80% test coverage for core, DevTrail AILOG for >10 line changes
