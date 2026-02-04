---
id: AILOG-2026-02-04-002
title: Complete phases 0-1 of spec 001-core-cli
status: accepted
created: 2026-02-04
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [auth, cli, integration-tests, cleanup]
related: [AILOG-2026-02-03-009]
---

# AILOG: Complete phases 0-1 of spec 001-core-cli

## Summary

Completed the remaining 3 incomplete tasks (T015, T166, T173) from the 254-task spec, wired up the auth CLI commands that were stubs, cleaned up obsolete TODO comments, and committed the Containerfile fix. All 254 tasks are now marked as completed.

## Context

The spec 001-core-cli had 251/254 tasks completed. The remaining gaps were:
1. Auth CLI commands (`login`/`logout`/`status`) were stubs that printed placeholder messages
2. T015: No mock server infrastructure for Graph API integration tests
3. T166: No sync integration test
4. T173: No delta integration test
5. Obsolete TODO comments in engine.rs referencing unimplemented FileWatcher (already implemented)
6. Uncommitted Containerfile.systemd fix

## Actions Performed

1. Committed the pre-existing Containerfile.systemd fix (replaced `loginctl enable-linger` and `systemctl --user` with static alternatives for container builds)
2. Wired up auth CLI commands in `crates/lnxdrive-cli/src/commands/auth.rs`:
   - `login`: OAuth2 PKCE flow via GraphAuthAdapter → token storage in keyring → user info from Graph API → account persistence in SQLite → audit entry
   - `logout`: Load account from DB → clear keyring → suspend account → audit entry
   - `status`: Load account from DB → check token validity in keyring → display account info
3. Updated obsolete TODO comments in `engine.rs` (lines 225, 361) and `filesystem.rs` (line 248)
4. Added `wiremock` as workspace dev-dependency
5. Created integration test infrastructure in `crates/lnxdrive-graph/tests/integration/` with 13 tests:
   - `common.rs`: Shared mock server helpers
   - `test_user_info.rs`: User profile and quota endpoints
   - `test_delta.rs`: Delta queries (initial, incremental, empty, deleted, mixed)
   - `test_sync_operations.rs`: Upload, download, error handling
6. Made `GraphClient::with_base_url()` public (was `#[cfg(test)]` only)
7. Updated `specs/001-core-cli/tasks.md`: marked T015, T166, T173 as completed

## Modified Files

| File | Change |
|------|--------|
| `docker/Containerfile.systemd` | Replace runtime-dependent systemd commands with static alternatives |
| `crates/lnxdrive-cli/src/commands/auth.rs` | Full implementation of login/logout/status commands |
| `crates/lnxdrive-sync/src/engine.rs` | Updated obsolete TODO comments |
| `crates/lnxdrive-sync/src/filesystem.rs` | Clarified watch stub comment |
| `Cargo.toml` | Added wiremock, tempfile to workspace deps |
| `crates/lnxdrive-graph/Cargo.toml` | Added dev-dependencies |
| `crates/lnxdrive-graph/src/client.rs` | Made with_base_url public |
| `crates/lnxdrive-graph/tests/integration/` | New: 5 test files, 13 integration tests |
| `specs/001-core-cli/tasks.md` | Marked T015, T166, T173 as completed |

## Decisions Made

- **Auth wiring architecture**: Used `GraphAuthAdapter` directly in CLI rather than going through `AuthenticateUseCase`, because `GraphCloudProvider::authenticate()` correctly `bail!`s with "Use GraphAuthAdapter". The adapter handles the OAuth PKCE flow, and the CLI orchestrates the full pipeline (auth → user info → persistence).
- **Integration tests location**: Placed in `crates/lnxdrive-graph/tests/integration/` rather than a workspace-level `tests/` directory, following Rust convention for crate-level integration tests.

## Impact

- **Functionality**: Auth commands now fully functional (login opens browser, stores tokens, persists account; logout clears credentials; status shows account info)
- **Performance**: N/A
- **Security**: Tokens stored securely in system keyring via `KeyringTokenStorage`

## Verification

- [x] Code compiles without errors (`cargo build --workspace` minus excluded crates)
- [x] Tests pass (495+ tests, 0 failures)
- [x] Clippy clean (`-D warnings`, 0 warnings)
- [x] All 254 tasks in tasks.md marked as completed

---

<!-- Template: DevTrail | https://enigmora.com -->
