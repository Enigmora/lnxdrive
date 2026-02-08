---
id: AILOG-2026-02-07-001
title: Implement conflict detection and resolution engine (spec 003)
status: accepted
created: 2026-02-07
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: medium
tags: [conflicts, sync-engine, dbus, cli]
related: [AILOG-2026-02-03-008-implement-sync-engine-graph-provider]
---

# AILOG: Implement conflict detection and resolution engine

## Summary

Implemented the complete conflict detection and resolution system for LNXDrive (Fase 5). The sync engine now detects when a file has been modified locally AND remotely since the last sync, marking it as a conflict instead of silently overwriting the local version. Configurable resolution policies allow automatic or manual handling.

## Context

The `handle_remote_update()` method in `SyncEngine` was overwriting local files without verifying if they had been modified locally, which could cause data loss. The `lnxdrive-conflict` crate existed as an empty placeholder, and domain entities (`Conflict`, `VersionInfo`, `Resolution`) were already defined in `lnxdrive-core` but not wired into the sync path. The D-Bus `ConflictsInterface` had stub implementations returning static JSON.

## Actions Performed

1. **Populated `lnxdrive-conflict` crate** with 7 modules: `detector.rs`, `policy.rs`, `resolver.rs`, `namer.rs`, `diff.rs`, `error.rs`, `use_cases.rs`
2. **ConflictDetector**: Hash-based detection comparing local state vs remote delta; returns `DetectionResult::Conflicted(Box<Conflict>)` or `NoConflict`
3. **PolicyEngine**: Glob-pattern rules from YAML config with first-match-wins evaluation for auto-resolution
4. **ConflictResolver**: Applies resolution strategies — KeepLocal (upload with ETag), KeepRemote (download), KeepBoth (rename local + download remote)
5. **ConflictNamer**: Generates unique filenames like `file (conflicted copy 2026-02-07 a1b2c3d4).ext`
6. **DiffToolLauncher**: Auto-detects meld > kdiff3 > vimdiff > diff; GUI tools spawn in background
7. **Integrated detection into SyncEngine**: `handle_remote_update()` now checks if item is `Modified` state and remote hash differs, triggering conflict detection with policy auto-resolution fallback
8. **Extended CLI**: Added `conflicts diff` and `conflicts resolve-all` (with glob filter) subcommands
9. **Replaced D-Bus stubs**: `ConflictsInterface` now queries `IStateRepository` for real data; added `get_details()`, `resolve_all()`, and `ConflictDetected`/`ConflictResolved` signals
10. **Extended ports**: Added `if_match_etag: Option<&str>` to `ICloudProvider::upload_file()` for race condition protection; added `get_conflict_by_id()` to `IStateRepository`

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-conflict/Cargo.toml` | Added dependencies (glob, chrono, uuid, tokio) |
| `crates/lnxdrive-conflict/src/lib.rs` | Module declarations and re-exports |
| `crates/lnxdrive-conflict/src/detector.rs` | **New** — ConflictDetector with `check_remote_update`, `should_auto_resolve` |
| `crates/lnxdrive-conflict/src/policy.rs` | **New** — PolicyEngine with glob matching, first-match-wins |
| `crates/lnxdrive-conflict/src/resolver.rs` | **New** — ConflictResolver with 3 strategies + BatchResult |
| `crates/lnxdrive-conflict/src/namer.rs` | **New** — ConflictNamer with timestamp + UUID8 |
| `crates/lnxdrive-conflict/src/diff.rs` | **New** — DiffToolLauncher with auto-detection |
| `crates/lnxdrive-conflict/src/error.rs` | **New** — ConflictError enum |
| `crates/lnxdrive-conflict/src/use_cases.rs` | **New** — DetectConflictUseCase, ResolveConflictUseCase |
| `crates/lnxdrive-sync/src/engine.rs` | Conflict detection in `handle_remote_update()`, PolicyEngine integration |
| `crates/lnxdrive-ipc/src/service.rs` | Real ConflictsInterface with DB queries, signals, removed `conflicts_json` from DaemonState |
| `crates/lnxdrive-cli/src/commands/conflicts.rs` | Added `Diff` and `ResolveAll` subcommands |
| `crates/lnxdrive-core/src/config.rs` | Extended ConflictsConfig with `rules: Vec<ConflictRuleConfig>`, `diff_tool` |
| `crates/lnxdrive-core/src/ports/cloud_provider.rs` | Added `if_match_etag` param to `upload_file()` |
| `crates/lnxdrive-core/src/ports/state_repository.rs` | Added `get_conflict_by_id()` |
| `crates/lnxdrive-cache/src/repository.rs` | Implemented `get_conflict_by_id` for SQLite |
| `crates/lnxdrive-daemon/src/main.rs` | Passes `state_repo` to `DbusService::new()` |
| `crates/lnxdrive-graph/src/provider.rs` | Added `_if_match_etag` param to `upload_file()` |
| `crates/lnxdrive-core/src/usecases/sync_file.rs` | Updated `upload_file()` call with `None` etag |
| `config/default-config.yaml` | Added `conflicts.rules` section with examples |

## Decisions Made

- **DetectionResult::Conflicted uses `Box<Conflict>`** to avoid clippy large_enum_variant warning (Conflict is 208 bytes)
- **ETag protection added to upload_file trait** rather than a separate method, to keep the interface simple. GraphCloudProvider has a TODO for actual If-Match header implementation
- **Policy auto-resolution happens inline** in `handle_remote_update()`: if a policy matches, the conflict is resolved immediately and the download proceeds normally; otherwise it's saved as unresolved
- **D-Bus `resolve()` returns `bool`** (not a full conflict JSON) for simplicity; UI clients can re-fetch the list after resolving

## Impact

- **Functionality**: Files modified both locally and remotely are now detected as conflicts instead of being silently overwritten. Users can resolve via CLI, D-Bus, or (with lnxdrive-gnome) the GTK4 preferences UI.
- **Performance**: Negligible — conflict detection adds one hash comparison per remote update; policy evaluation is O(n) over rules.
- **Security**: N/A (no auth/credential changes)

## Verification

- [x] Code compiles without errors
- [x] 746 workspace tests pass, 0 failures
- [x] Clippy clean across workspace
- [ ] Manual review performed

## Additional Notes

This commit is on branch `feat/003-conflicts` based on `feat/002-files-on-demand`. PR: https://github.com/Enigmora/lnxdrive/pull/9

The `if_match_etag` parameter on `GraphCloudProvider::upload_file()` is accepted but not yet sent as an HTTP header (marked with TODO). This will be wired in a follow-up when the actual Microsoft Graph upload flow is enhanced.

---

<!-- Template: DevTrail | https://enigmora.com -->
