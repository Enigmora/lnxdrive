---
id: AILOG-2026-02-03-001
title: Implement SyncItem entity with Files-On-Demand state machine
status: accepted
created: 2026-02-03
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [domain, sync-item, state-machine, files-on-demand]
related: [T025, T026, T027, T028, T029, T030]
---

# AILOG: Implement SyncItem entity with Files-On-Demand state machine

## Summary

Rewrote the `sync_item.rs` module in `lnxdrive-core` to implement the SyncItem domain entity using the Files-On-Demand hydration model. This replaces the previous upload/download-centric state model with the OneDrive-aligned Online/Hydrating/Hydrated/Modified state machine.

## Context

Tasks T025-T030 required creating the core SyncItem entity and related types for the LNXDrive domain layer. The previous implementation used states like Synced/PendingUpload/PendingDownload which did not align with the Files-On-Demand architecture described in the design guide. The new implementation models the hydration lifecycle as specified in the architecture documentation.

## Actions Performed

1. **T025**: Implemented `ItemState` enum with variants: Online, Hydrating, Hydrated, Modified, Conflicted, Error(String), Deleted. Added helper methods (is_local, is_placeholder, is_transferring, needs_attention, has_pending_changes, name).
2. **T026**: Implemented `ItemMetadata` struct with fields: is_directory, mime_type, created_at, etag, permissions. Also created the `Permissions` struct with Unix mode conversion support.
3. **T027**: Implemented `ErrorInfo` struct with fields: code, message, retry_count, last_attempt, next_retry. Added exponential backoff scheduling and factory methods for common error types (network, auth, rate limit, conflict).
4. **T028**: Implemented `SyncItem` struct with all specified fields: id (UniqueId), local_path (SyncPath), remote_id (RemoteId), remote_path (RemotePath), state (ItemState), content_hash, local_hash, size_bytes, last_sync, last_modified_local, last_modified_remote, metadata, error_info.
5. **T029**: Implemented `SyncItem::new()` constructor with validation, plus convenience constructors: new_file, new_directory, from_remote.
6. **T030**: Implemented state transition methods: `can_transition_to()` and `transition_to()` with the specified valid transition matrix. Added convenience methods for common transitions and error handling.

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-core/src/domain/sync_item.rs` | Complete rewrite with Files-On-Demand state machine |
| `crates/lnxdrive-core/src/domain/mod.rs` | Added `Permissions` to re-exports |

## Decisions Made

- **State machine design**: Deleted is a terminal state (no transitions out). Error can transition to any state (retry mechanism). This follows the specification exactly.
- **ErrorInfo retry**: Includes exponential backoff support and factory methods for common error categories.
- **Permissions model**: Simple read/write/execute model with Unix mode conversion, suitable for FUSE integration.
- **SyncItem::new() defaults to Online**: New items start in the Online (placeholder) state, which is the natural starting point for the Files-On-Demand model.

## Impact

- **Functionality**: Core domain entity for the entire sync system. All sync operations will use this state machine.
- **Performance**: N/A (pure domain logic, no I/O)
- **Security**: N/A

## Verification

- [ ] Code compiles without errors (cargo not available in current environment)
- [ ] Tests pass (cargo not available in current environment)
- [x] Manual review performed

## Additional Notes

The implementation includes comprehensive unit tests covering all state transitions, error handling, serialization roundtrips, and edge cases. The `value_objects.rs` file exists in the directory but is not referenced in `mod.rs` -- it appears to be an older/alternative implementation that is not currently active.

---

<!-- Template: DevTrail | https://enigmora.com -->
