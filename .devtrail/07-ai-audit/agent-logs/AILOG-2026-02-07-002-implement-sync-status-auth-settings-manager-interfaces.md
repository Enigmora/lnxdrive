---
id: AILOG-2026-02-07-002
title: Implement Sync, Status, Auth, Settings, Manager D-Bus interfaces
status: accepted
created: 2026-02-07
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [dbus, ipc, sync, status, auth, settings, manager, gnome-integration]
related: [AILOG-2026-02-07-001]
---

## Summary

Implemented the remaining 5 D-Bus interfaces in `lnxdrive-ipc` to complete the full contract defined in `dbus-gnome-contracts.md`. All 7 interfaces required by GNOME integration components (plus 2 legacy) are now implemented.

## Context

After Session 1 added the `Files` interface, the daemon still lacked 5 interfaces consumed by GNOME components: `Sync` (Shell extension), `Status` (Shell extension + Preferences), `Auth` (Onboarding wizard), `Settings` (Preferences panel), and `Manager` (Shell extension + Onboarding).

## Actions Performed

1. **Extended `DaemonState`** with 15 new fields across 5 interface domains:
   - Sync: `last_sync_time`, `pending_changes`
   - Status: `connection_status`, `quota_used`, `quota_total`
   - Auth: `is_authenticated`, `auth_url`, `auth_csrf_state`
   - Settings: `config_yaml`, `selected_folders`, `exclusion_patterns`, `remote_folder_tree`
   - Manager: `version`, `is_running`

2. **Implemented 5 new interface structs** following the existing `#[zbus::interface]` pattern:

   - **`SyncInterface`** (`com.enigmora.LNXDrive.Sync`): 3 methods (`sync_now`, `pause`, `resume`), 3 read-only properties (`SyncStatus`, `LastSyncTime`, `PendingChanges`), 4 signals. Coexists with legacy `SyncController`.

   - **`StatusInterface`** (`com.enigmora.LNXDrive.Status`): 2 methods (`get_quota` → `(u64, u64)`, `get_account_info` → `HashMap<String, OwnedValue>` for D-Bus `a{sv}`), 1 property (`ConnectionStatus`), 2 signals.

   - **`AuthInterface`** (`com.enigmora.LNXDrive.Auth`): 4 methods (`start_auth` → `(String, String)`, `complete_auth` with CSRF validation → `bool`, `is_authenticated` → `bool`, `logout`), 1 signal.

   - **`SettingsInterface`** (`com.enigmora.LNXDrive.Settings`): 7 methods (get/set config, folders, exclusion patterns, remote folder tree), 1 signal.

   - **`ManagerInterface`** (`com.enigmora.LNXDrive.Manager`): 4 methods (`start`, `stop`, `restart`, `get_status`), 2 read-only properties (`Version`, `IsRunning`).

3. **Registered all 5 interfaces** in `DbusService::start()`.
4. **Updated `lib.rs`** to re-export all new interface types.
5. **Added 36 new tests** (66 total in crate, up from 30).

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-ipc/src/service.rs` | Added 5 interfaces, extended DaemonState with 15 fields, registered in DbusService, added 36 tests |
| `crates/lnxdrive-ipc/src/lib.rs` | Re-exported 5 new interface types, updated doc comments |

## Decisions Made

- **Sync coexists with SyncController**: The new `Sync` interface uses the contract-specified name (`com.enigmora.LNXDrive.Sync`) while the legacy `SyncController` remains for backward compatibility. Both share the same `DaemonState`.
- **`a{sv}` via OwnedValue**: `StatusInterface::get_account_info()` returns `HashMap<String, OwnedValue>` to match the D-Bus `a{sv}` type in the contract, rather than JSON strings used by the legacy `Account` interface.
- **CSRF validation in CompleteAuth**: Auth flow includes CSRF state token verification to prevent cross-site request forgery, matching the OAuth2 PKCE flow pattern.
- **Empty config rejection**: `SetConfig` silently rejects empty YAML strings to prevent accidental config wipe.
- **Version from Cargo**: `DaemonState::version` defaults to `env!("CARGO_PKG_VERSION")` at compile time.

## Impact

### Functionality
- All 7 D-Bus interfaces from `dbus-gnome-contracts.md` are now implemented (9 total including 2 legacy)
- GNOME Shell extension, Preferences panel, Nautilus extension, and Onboarding wizard can now connect to the real daemon
- Unblocks Sessions 3-5 (Shell extension completion, conflict UI, integration testing)

### Performance
- No performance impact; all operations are O(1) state reads/writes behind `Arc<Mutex<>>`

### Security
- Auth interface includes CSRF state verification
- Logout clears all credential-related fields
- No actual token handling (deferred to OAuth2 integration)

## Verification

- [x] `cargo build -p lnxdrive-ipc` compiles successfully
- [x] `cargo test -p lnxdrive-ipc` — 66 tests pass (30 existing + 36 new)
- [x] `cargo test --workspace` — all 16 test suites pass (765 tests), no regressions
- [x] All interface names match contract: `.Sync`, `.Status`, `.Auth`, `.Settings`, `.Manager`
- [x] Method signatures and return types match `dbus-gnome-contracts.md`
- [x] D-Bus properties use `#[zbus(property)]` for proper PropertiesChanged signals
