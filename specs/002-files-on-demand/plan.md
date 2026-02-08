# Implementation Plan: Files-on-Demand (FUSE Virtual Filesystem)

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/002-files-on-demand/spec.md`

## Summary

Implement a FUSE-based virtual filesystem (`lnxdrive-fuse` crate) that presents OneDrive files as local entries without downloading content upfront. Files appear with their real metadata (names, sizes, timestamps) but content is downloaded on-demand when accessed ("hydration") and reclaimed automatically when disk space is needed ("dehydration"). Users can pin files for permanent offline access.

**Technical approach**: Use the `fuser` crate (v0.16) directly, with `spawn_mount2()` running FUSE callbacks in a background thread. A `tokio::runtime::Handle` stored in the `LnxDriveFs` struct enables calling async code from synchronous FUSE callbacks via `handle.block_on()`. SQLite writes are serialized through a dedicated writer task consuming `WriteOp` messages from an mpsc channel.

## Technical Context

**Language/Version**: Rust 1.75+ (MSRV)
**Primary Dependencies**: `fuser` 0.16 (FUSE protocol), `tokio` 1.35 (async runtime), `sqlx` 0.7 (SQLite), `reqwest` 0.12 (HTTP downloads), `dashmap` (concurrent inode table)
**Storage**: SQLite 3.35+ (state repository, inode mapping) + filesystem cache (`~/.local/share/lnxdrive/cache/content/`)
**Testing**: `cargo test` + `wiremock` 0.6 (Graph API mocks) + container-based E2E (FUSE requires `/dev/fuse`)
**Target Platform**: Linux (x86_64, aarch64) with FUSE3 support
**Project Type**: Rust workspace crate (`crates/lnxdrive-fuse`)
**Performance Goals**: `getattr` <1ms, `readdir` <10ms (1000 entries), first byte <2s (1MB file), streaming for >100MB
**Constraints**: <50MB idle memory (10k files), no root privileges, single-account (multi-account deferred to Fase 6)
**Scale/Scope**: 10,000+ tracked files, 50 concurrent file accesses without corruption or deadlocks

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Hexagonal Architecture | PASS | FUSE layer is an adapter; domain types (`ItemState`, `SyncItem`) stay in `lnxdrive-core`; ports define contracts |
| II. Idiomatic Rust | PASS | Newtype (`InodeNumber`), Builder (`FuseConfig`), RAII (mount handle), `thiserror` for library errors |
| III. Testing by Layers | PASS | Unit: core state machine with mocks; Integration: FUSE ops in container with `/dev/fuse`; E2E: full mount+hydrate |
| IV. DevTrail Documentation | PASS | AILOG for implementation, AIDEC for research decisions (R1-R9 already documented) |
| V. Design Guide Compliance | PASS | `04-Componentes/01-files-on-demand-fuse.md` consulted for architecture, risks, and patterns |
| VI. Git Workflow | PASS | Feature branch `feat/002-files-on-demand`, conventional commits, PR required |
| VII. Security First | PASS | No credential changes; cache files inherit user permissions; no path traversal via FUSE (paths validated against inode table) |
| VIII. Performance Requirements | PASS | Targets match constitution: `getattr` <1ms, `readdir` <10ms, streaming reads, <50MB idle |
| IX. Accessibility | N/A | No UI components in this phase (CLI only, desktop integration deferred to Fase 3) |

## Design Artifacts

All Phase 0 (Research) and Phase 1 (Design) artifacts have been generated:

| Artifact | Path | Description |
|----------|------|-------------|
| Research | [research.md](research.md) | 9 research decisions (R1-R9): crate selection, async bridge, SQLite serialization, cache storage, hydration strategy, inode management, Pinned state, dehydration policy, config schema |
| Data Model | [data-model.md](data-model.md) | Entity changes (ItemState + Pinned, SyncItem new fields), new entities (InodeEntry, HydrationRequest, DehydrationPolicy, FuseConfig), SQLite schema changes, state machine diagram, cache directory structure |
| CLI Contracts | [contracts/cli-commands.md](contracts/cli-commands.md) | 6 new commands (mount, unmount, pin, unpin, hydrate, dehydrate) + modified `status` and `daemon start`, with human/JSON output formats and error messages |
| FUSE Operations | [contracts/fuse-operations.md](contracts/fuse-operations.md) | 22 FUSE operations (metadata, directory, file, xattr), error mapping, mount options |
| Quickstart | [quickstart.md](quickstart.md) | User-facing guide: prerequisites, mounting, file access, pinning, dehydration, validation |

## Project Structure

### Documentation (this feature)

```text
specs/002-files-on-demand/
├── plan.md              # This file
├── spec.md              # Feature specification (44 FRs, 6 user stories, 10 edge cases)
├── research.md          # Phase 0: 9 research decisions (R1-R9)
├── data-model.md        # Phase 1: entities, state machine, SQLite schema
├── quickstart.md        # Phase 1: user-facing usage guide
├── contracts/
│   ├── cli-commands.md  # Phase 1: CLI command contracts
│   └── fuse-operations.md # Phase 1: FUSE operations contract
├── checklists/
│   └── requirements.md  # Quality checklist
└── tasks.md             # Phase 2: task breakdown (generated by /speckit.tasks)
```

### Source Code (repository root)

```text
crates/lnxdrive-fuse/
├── Cargo.toml                  # Dependencies: fuser, tokio, tracing, thiserror + new deps
└── src/
    ├── lib.rs                  # Public API: mount(), unmount(), FuseConfig re-export
    ├── error.rs                # FuseError enum (thiserror)
    ├── filesystem.rs           # LnxDriveFs struct implementing fuser::Filesystem
    ├── inode.rs                # InodeTable: bidirectional inode ↔ item_id mapping
    ├── inode_entry.rs          # InodeEntry struct (in-memory inode representation)
    ├── hydration.rs            # HydrationManager: queue, dedup, progress, download
    ├── dehydration.rs          # DehydrationManager: LRU sweep, policy enforcement
    ├── cache.rs                # ContentCache: hash-based file storage, disk usage tracking
    ├── write_serializer.rs     # WriteSerializer: mpsc-based SQLite write serialization
    └── xattr.rs                # Extended attributes handler (user.lnxdrive.* namespace)

crates/lnxdrive-core/src/
├── domain/
│   └── sync_item.rs            # MODIFIED: Add Pinned variant to ItemState, new fields on SyncItem
└── config.rs                   # MODIFIED: Add FuseConfig section to Config struct

crates/lnxdrive-cache/src/
└── migrations/                 # NEW: SQL migrations for inode_counter table + sync_items columns

crates/lnxdrive-cli/src/commands/
├── mount.rs                    # NEW: mount/unmount CLI commands
├── pin.rs                      # NEW: pin/unpin CLI commands
├── hydrate.rs                  # NEW: hydrate/dehydrate CLI commands
└── mod.rs                      # MODIFIED: Register new command modules

config/
└── default-config.yaml         # MODIFIED: Add fuse section with defaults

tests/
└── integration/
    └── fuse/                   # Integration tests requiring /dev/fuse (container)
        ├── test_mount.rs       # Mount/unmount lifecycle
        ├── test_readdir.rs     # Directory listing
        ├── test_hydration.rs   # On-demand download
        ├── test_dehydration.rs # Space reclamation
        ├── test_write.rs       # Write operations
        ├── test_pin.rs         # Pin/unpin operations
        └── test_xattr.rs       # Extended attributes
```

**Structure Decision**: The FUSE implementation lives in the existing `crates/lnxdrive-fuse` skeleton crate, following the established hexagonal architecture. Domain changes (ItemState, Config) go in `lnxdrive-core`. CLI commands go in `lnxdrive-cli`. Database migrations go in `lnxdrive-cache`. This maintains the inward dependency direction: `lnxdrive-fuse` depends on `lnxdrive-core` (ports/domain) and `lnxdrive-cache` (state repository), never the reverse.

## Key Design Decisions

| Decision | Choice | Reference |
|----------|--------|-----------|
| FUSE crate | `fuser` 0.16 (pure Rust, no C libfuse dependency) | R1 |
| Sync-to-async bridge | `Handle::block_on()` inside FUSE callbacks (safe: `spawn_mount2` uses non-tokio thread) | R2 |
| SQLite write serialization | Dedicated tokio task + mpsc channel + oneshot results | R3 |
| Content storage | `~/.local/share/lnxdrive/cache/content/` with SHA-256 hash subdirectories | R4 |
| Hydration strategy | Full download <100MB, HTTP Range chunked ≥100MB | R5 |
| Inode management | Monotonic u64 counter persisted in SQLite, bidirectional mapping | R6 |
| Pinned state | New `Pinned` variant in `ItemState` enum (not a separate bool flag) | R7 |
| Dehydration policy | LRU eviction with configurable threshold + max age, periodic sweep | R8 |
| Configuration | New `fuse` section in existing YAML config (8 knobs) | R9 |

## Risk Mitigations

| Risk | Mitigation | Reference |
|------|------------|-----------|
| A1: FUSE callbacks block tokio workers | `spawn_mount2()` runs FUSE in dedicated non-tokio thread | Design Guide §Risks |
| A2: SQLITE_BUSY from concurrent writes | WriteSerializer pattern (single writer task) | R3, Design Guide §A2 |
| B1: Hydration latency for large files | Range-based streaming for files ≥100MB | R5 |
| C1: Disk space exhaustion during hydration | Check available space before download, report `ENOSPC` | FR-014, Edge Case |
| C2: Dehydrating open files | Track open handles via atomic counters in open/release | FR-016, R8 |
| D1: Crash recovery | Detect partial downloads on startup, resume or clean up | FR-012, Edge Case |

## Complexity Tracking

No constitution violations. All design choices follow established patterns:
- Single crate addition (`lnxdrive-fuse` already exists as skeleton)
- Domain changes are minimal (1 new enum variant, 3 new fields)
- No new architectural patterns beyond what the design guide prescribes
