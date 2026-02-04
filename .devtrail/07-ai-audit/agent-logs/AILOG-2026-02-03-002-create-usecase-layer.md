---
id: AILOG-2026-02-03-002
title: Create use case layer for lnxdrive-core (T061-T076)
status: accepted
created: 2026-02-03
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [usecases, hexagonal-architecture, core]
related: [AILOG-2026-02-03-001]
---

# AILOG: Create use case layer for lnxdrive-core (T061-T076)

## Summary

Created the use case (interactor) layer for lnxdrive-core, implementing four use cases that orchestrate domain entities through port interfaces following the hexagonal architecture pattern. Updated lib.rs with enhanced documentation.

## Context

The domain entities (T025-T044) and port trait definitions (T047-T060) were being created in parallel. The use case layer needed to be added to coordinate between domain logic and port interfaces, completing the hexagonal architecture core. These use cases are thin orchestrators that delegate business rules to domain methods and I/O operations to port traits.

## Actions Performed

1. Created `usecases/mod.rs` (T061) with module declarations and re-exports
2. Created `usecases/authenticate.rs` (T062-T066) with AuthenticateUseCase: login (OAuth2 PKCE), logout, refresh_if_needed, get_status
3. Created `usecases/sync_file.rs` (T067-T070) with SyncFileUseCase: sync_single, upload (PUT vs session based on size), download with hash verification
4. Created `usecases/query_delta.rs` (T071-T073) with QueryDeltaUseCase: execute (with pagination), handle_delta_item
5. Created `usecases/explain_failure.rs` (T074-T075) with ExplainFailureUseCase and Explanation struct: explain with state-specific messages and suggestions
6. Updated `lib.rs` (T076) with enhanced module documentation

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-core/src/usecases/mod.rs` | New: module declarations and re-exports for all four use cases |
| `crates/lnxdrive-core/src/usecases/authenticate.rs` | New: AuthenticateUseCase with login, logout, refresh_if_needed, get_status |
| `crates/lnxdrive-core/src/usecases/sync_file.rs` | New: SyncFileUseCase with sync_single, upload, download |
| `crates/lnxdrive-core/src/usecases/query_delta.rs` | New: QueryDeltaUseCase with execute, handle_delta_item |
| `crates/lnxdrive-core/src/usecases/explain_failure.rs` | New: ExplainFailureUseCase with Explanation struct and explain method |
| `crates/lnxdrive-core/src/lib.rs` | Updated: enhanced module documentation |

## Decisions Made

- **Arc<dyn Trait + Send + Sync> for dependency injection**: All port dependencies use Arc-wrapped trait objects for thread-safe shared ownership, consistent with async runtime requirements.
- **anyhow::Result for use case methods**: Use cases use anyhow for error handling since they are orchestrators and do not define their own error types; domain errors are context-wrapped.
- **4MB threshold for upload method**: Simple PUT for files under 4MB, resumable session upload for larger files, following OneDrive API best practices.
- **5-minute token refresh threshold**: Tokens are proactively refreshed if expiring within 5 minutes to avoid mid-operation failures.
- **Explanation struct in explain_failure.rs**: Co-located with the use case rather than in the domain since it is a presentation concern for the CLI.

## Impact

- **Functionality**: Adds the complete use case orchestration layer connecting domain entities to port interfaces
- **Performance**: N/A (no runtime code yet; use cases are thin coordinators)
- **Security**: Authentication use case handles OAuth2 PKCE flow delegation; tokens are persisted through the state repository port

## Verification

- [ ] Code compiles without errors (port trait files are being created in parallel; compilation deferred)
- [x] Unit tests added for Explanation struct and explain_failure module
- [x] All use cases follow thin-orchestrator pattern delegating to domain methods

## Additional Notes

- The port trait files (cloud_provider.rs, state_repository.rs, local_filesystem.rs) are being created in a parallel task. The use cases reference types from ports/mod.rs re-exports.
- The authenticate.rs uses a `dirs_default_sync_root` helper function that computes `$HOME/OneDrive` as the default sync root.
- The explain_failure module includes comprehensive unit tests for all ItemState variants.

---

<!-- Template: DevTrail | https://enigmora.com -->
