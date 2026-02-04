# Implementation Plan: Core + CLI (Fase 1 - Fundamentos)

**Branch**: `001-core-cli` | **Date**: 2026-02-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-core-cli/spec.md`

## Summary

Implementar el nucleo de sincronizacion de LNXDrive: motor de sincronizacion bidireccional con OneDrive usando Microsoft Graph API, autenticacion OAuth2 PKCE, CLI completa, y daemon systemd. El sistema sigue arquitectura hexagonal con Rust como lenguaje principal, tokio como runtime async, y SQLite para persistencia de estado.

## Technical Context

**Language/Version**: Rust 1.75+ (MSRV)
**Primary Dependencies**: tokio (async runtime), reqwest (HTTP), oauth2-rs (OAuth2 PKCE), zbus (D-Bus), sqlx (SQLite), clap (CLI), serde (serialization), thiserror/anyhow (errors)
**Storage**: SQLite 3.35+ (estado de sincronizacion, audit log)
**Testing**: cargo test (unit), containers (integration), VM (E2E systemd)
**Target Platform**: Linux x86_64/aarch64 con systemd
**Project Type**: Monorepo con multiples crates (workspace)
**Performance Goals**: <50MB memoria idle, <1% CPU idle, sincronizacion de 1000 archivos en <10min
**Constraints**: <30s para reflejar cambios locales en nube, recuperacion automatica de errores de red
**Scale/Scope**: Una cuenta OneDrive, archivos hasta 250GB, miles de archivos

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Implementation Notes |
|-----------|--------|---------------------|
| **I. Hexagonal Architecture** | ✅ PASS | `domain/` → `ports/` → `adapters/` → `application/` structure |
| **II. Idiomatic Rust** | ✅ PASS | Newtype (SyncPath, FileHash), Builder (Config), Type-State (SyncItem), thiserror/anyhow |
| **III. Testing by Layers** | ✅ PASS | Unit (core 80%), Integration (adapters 70%), E2E (systemd VM) |
| **IV. DevTrail Documentation** | ✅ PASS | AILOG for changes >10 lines, AIDEC for tech decisions |
| **V. Design Guide Compliance** | ✅ PASS | Load `lnxdrive-guide/04-Componentes/07-motor-sincronizacion.md` before implementation |
| **VI. Git Workflow** | ✅ PASS | Feature branch `001-core-cli`, conventional commits |
| **VII. Security First** | ✅ PASS | Tokens in libsecret keyring, no secrets in logs, HTTPS only |
| **VIII. Performance Requirements** | ✅ PASS | <50MB idle, <1% CPU idle, chunked transfers >10MB |
| **IX. Accessibility (a11y)** | N/A | No UI in Phase 1 (CLI only) |

**Gate Result**: ✅ All applicable principles satisfied. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/001-core-cli/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (D-Bus interface definitions)
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
crates/
├── lnxdrive-core/           # Domain core (business logic)
│   └── src/
│       ├── domain/          # Entities: SyncItem, Account, Conflict, AuditEntry
│       │   ├── mod.rs
│       │   ├── sync_item.rs
│       │   ├── account.rs
│       │   ├── conflict.rs
│       │   └── audit.rs
│       ├── ports/           # Traits: ICloudProvider, IStateRepository, ILocalFileSystem
│       │   ├── mod.rs
│       │   ├── cloud_provider.rs
│       │   ├── state_repository.rs
│       │   ├── local_filesystem.rs
│       │   └── notification.rs
│       ├── usecases/        # Interactors: SyncFileUseCase, AuthenticateUseCase, etc.
│       │   ├── mod.rs
│       │   ├── sync_file.rs
│       │   ├── authenticate.rs
│       │   ├── query_delta.rs
│       │   └── explain_failure.rs
│       └── lib.rs
│
├── lnxdrive-graph/          # Microsoft Graph adapter
│   └── src/
│       ├── client.rs        # GraphClient with rate limiting
│       ├── auth.rs          # OAuth2 PKCE flow
│       ├── delta.rs         # Delta sync implementation
│       ├── upload.rs        # PUT + upload sessions
│       └── lib.rs
│
├── lnxdrive-sync/           # Sync engine adapter
│   └── src/
│       ├── engine.rs        # SyncEngine orchestrator
│       ├── state_machine.rs # Item state transitions
│       ├── watcher.rs       # inotify file watcher
│       └── lib.rs
│
├── lnxdrive-cache/          # SQLite state repository
│   └── src/
│       ├── repository.rs    # IStateRepository impl
│       ├── migrations/      # SQLx migrations
│       └── lib.rs
│
├── lnxdrive-ipc/            # D-Bus service library
│   └── src/
│       ├── service.rs       # zbus service definition
│       ├── interface.rs     # ISyncController D-Bus interface
│       └── lib.rs
│
├── lnxdrive-cli/            # CLI binary
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── auth.rs
│       │   ├── sync.rs
│       │   ├── status.rs
│       │   ├── explain.rs
│       │   ├── audit.rs
│       │   ├── config.rs
│       │   └── daemon.rs
│       └── output.rs        # JSON/human output formatting
│
└── lnxdrive-daemon/         # Daemon binary
    └── src/
        ├── main.rs
        └── systemd.rs       # Service lifecycle

tests/
├── unit/                    # Unit tests (in-crate with #[cfg(test)])
├── integration/             # Integration tests (containers)
│   ├── graph_mock/          # Wiremock MS Graph server
│   └── sqlite_tests/
└── e2e/                     # E2E tests (VM with systemd)
    └── daemon_tests/

config/
├── lnxdrive.service         # systemd user service unit
└── default-config.yaml      # Default configuration template
```

**Structure Decision**: Monorepo Rust workspace con arquitectura hexagonal. Cada crate tiene responsabilidad unica: `lnxdrive-core` contiene dominio puro sin dependencias externas, los adapters (`lnxdrive-graph`, `lnxdrive-cache`, `lnxdrive-sync`) implementan los ports, y los binarios (`lnxdrive-cli`, `lnxdrive-daemon`) orquestan todo.

## Complexity Tracking

> **No violations detected. Constitution principles are fully satisfied.**

| Aspect | Justification |
|--------|---------------|
| Multiple crates | Required by Hexagonal Architecture principle - separation of domain, ports, and adapters |
| D-Bus interface | Required for future UI integration (GNOME/KDE phases) - design guide compliance |
