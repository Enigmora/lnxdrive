<!--
SYNC IMPACT REPORT
==================
Version change: N/A (new) → 1.0.0
Modified principles: N/A (initial creation)
Added sections:
  - I. Hexagonal Architecture
  - II. Idiomatic Rust
  - III. Testing by Layers (NON-NEGOTIABLE)
  - IV. DevTrail Documentation
  - V. Design Guide Compliance
  - VI. Git Workflow
  - VII. Security First (NON-NEGOTIABLE)
  - VIII. Performance Requirements
  - IX. Accessibility (a11y)
Removed sections: None
Templates requiring updates:
  - .specify/templates/plan-template.md: ✅ Compatible (Constitution Check section exists)
  - .specify/templates/spec-template.md: ✅ Compatible (priority-based user stories align)
  - .specify/templates/tasks-template.md: ✅ Compatible (test-first approach supported)
Follow-up TODOs: None
-->

# LNXDrive Constitution

## Core Principles

### I. Hexagonal Architecture

The project follows Clean Architecture + Ports and Adapters (Hexagonal):

- Domain core MUST be independent of UI, infrastructure, and frameworks
- Strict layer separation MUST be enforced: `domain/` → `ports/` → `adapters/` → `application/`
- Ports define contracts (traits); adapters implement them
- Dependencies MUST point inward: adapters depend on ports, ports depend on domain

**Reference:** `lnxdrive-guide/03-Arquitectura/01-arquitectura-hexagonal.md`

### II. Idiomatic Rust

All code MUST follow Rust API Guidelines for naming and conventions:

**Required Patterns:**
- **Newtype Pattern**: MUST use for type safety (SyncPath, FileHash, WatchHandle)
- **Builder Pattern**: MUST use for complex configuration with validation
- **Type-State Pattern**: MUST use for compile-time state machines
- **RAII Pattern**: MUST use for automatic resource management

**Error Handling:**
- Libraries MUST use `thiserror`
- Applications MUST use `anyhow`

**Async Runtime:** `tokio` MUST be the primary runtime

**Reference:** `lnxdrive-guide/05-Implementacion/04-patrones-rust.md`

### III. Testing by Layers (NON-NEGOTIABLE)

**Test Types:**
- **Unit tests**: Core with port mocks (direct host execution)
- **Integration tests**: Adapters in containers
- **E2E tests**: Headless VM for FUSE/systemd, GUI VM for desktop extensions

**Minimum Coverage Requirements:**
| Layer | Coverage |
|-------|----------|
| Core (business logic) | **80%** |
| Adapters | **70%** |
| Critical code (FUSE, security) | **90%** |

Tests derived from risk analysis (P0/P1) are MANDATORY.

**Reference:** `lnxdrive-guide/06-Testing/01-estrategia-testing.md`

### IV. DevTrail Documentation

Every significant technical decision MUST be documented:

| Type | When to Use | Location |
|------|-------------|----------|
| AILOG | Code changes >10 lines in business logic | `.devtrail/07-ai-audit/agent-logs/` |
| AIDEC | Decisions between technical alternatives | `.devtrail/07-ai-audit/decisions/` |
| ADR | Architectural decisions (requires human review) | `.devtrail/02-design/decisions/` |
| ETH | Changes with ethical/privacy impact (requires approval) | `.devtrail/07-ai-audit/ethical-reviews/` |

**Naming:** `[TYPE]-[YYYY-MM-DD]-[NNN]-[description].md`

**Reference:** `CLAUDE.md`, `.devtrail/00-governance/`

### V. Design Guide Compliance

Before implementing any component, the Design Guide MUST be consulted:

**The guide at `lnxdrive-guide/` contains:**
- Project guiding principles
- Component specifications
- Component-specific testing strategies
- Risk analysis and mitigations

**Requirement:** Agents MUST load the relevant document before proposing changes.

**Reference:** `lnxdrive-guide/Guía-de-diseño-y-desarrollo.md`

### VI. Git Workflow

**Branch Policy:**
- Direct commits to `main` are FORBIDDEN — always use feature branches + PR
- Branch prefixes: `feature/`, `fix/`, `docs/`, `refactor/`, `test/`

**Commit Format:** Conventional Commits
- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation only
- `refactor:` — No behavior change
- `chore:` — Maintenance

**PR Requirements:**
- Tests MUST pass
- Documentation MUST be updated

**Reference:** `.devtrail/00-governance/GIT-BRANCHING-STRATEGY.md`

### VII. Security First (NON-NEGOTIABLE)

**Tokens and Credentials:**
- MUST store in system keyring (libsecret/kwallet)
- MUST NEVER store in configuration files or logs
- OAuth handling code MUST have `risk_level: high`

**Input Validation:**
- All paths MUST be absolute and validated (SyncPath newtype)
- Paths MUST be sanitized to prevent path traversal

**Communication:**
- External APIs MUST use HTTPS only
- TLS certificates MUST be verified

**Logging and Audit:**
- Tokens, passwords, or sensitive data MUST NEVER be logged
- Authentication/security changes MUST create AILOG + ETH

**Dependencies:**
- `cargo audit` MUST run in CI
- Security updates MUST be reviewed monthly

**Reference:** `lnxdrive-guide/06-Testing/09-testing-seguridad.md`

### VIII. Performance Requirements

**FUSE Operations:**
| Operation | Latency |
|-----------|---------|
| `getattr` | <1ms |
| `readdir` | <10ms for directories with <1000 entries |
| `open/read` | Streaming without loading entire file into memory |

**Daemon:**
- Base memory MUST be <50MB idle
- CPU MUST be <1% idle, <10% during active sync

**Synchronization:**
- Concurrent uploads/downloads per rate limiting
- Chunked transfers for files >10MB
- Delta sync when possible

**UI:**
- Response time MUST be <100ms for user actions
- UI MUST NEVER block during network operations

**Reference:** `lnxdrive-guide/05-Implementacion/02-justificacion-rust.md`

### IX. Accessibility (a11y)

All UIs MUST be accessible:

**Requirements:**
- **Keyboard navigation**: All controls MUST be accessible without mouse
- **Screen readers**: Labels and ARIA/ATK roles MUST be correct
- **Contrast**: MUST meet WCAG 2.1 level AA minimum
- **Text size**: MUST respect system configuration

**Verification Tools:**
- GTK: `gtk4-widget-factory` with Accerciser
- Qt: `qt-accessibility-inspector`

Accessibility tests are MANDATORY before release.

**Reference:** WCAG 2.1 Standards, GNOME HIG, KDE HIG

## Governance

- The Constitution takes precedence over all other practices
- Changes to the Constitution require:
  1. Documentation (AIDEC or ADR)
  2. Human approval
  3. Migration plan for affected code
- Use `lnxdrive-guide/` as authoritative development reference
- All PRs/reviews MUST verify compliance with these principles
- Complexity beyond these principles MUST be justified in writing

**Version**: 1.0.0 | **Ratified**: 2026-02-03 | **Last Amended**: 2026-02-03
