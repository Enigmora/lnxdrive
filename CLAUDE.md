# DevTrail - Documentation Guidelines

> **This file is automatically loaded at the start of each session.**
> It contains the essential rules of the DevTrail for development traceability.

---

## 1. Language Configuration

Check `.devtrail/config.yml` for the project's language setting:

```yaml
language: en  # Options: en, es (default: en)
```

**Template paths based on language:**

| Language | Template Path |
|----------|---------------|
| `en` (default) | `.devtrail/templates/TEMPLATE-*.md` |
| `es` | `.devtrail/templates/i18n/es/TEMPLATE-*.md` |

If the config file doesn't exist or `language` is not set, use English (`en`) as default.

---

## 2. Documentation Reporting

At the end of each task, you MUST report your DevTrail documentation status:

**If you created documentation:**
```
DevTrail: Created AILOG-2025-01-27-001-implement-auth.md
```

**If documentation was not needed:**
```
DevTrail: No documentation required (minor change / <10 lines)
```

**If you should have documented but didn't:**
```
DevTrail: Documentation pending - use /devtrail-status to review
```

This transparency helps users verify compliance with DevTrail rules.

---

## 3. Fundamental Principle

> **"No significant change without a documented trace."**

---

## 4. Your Identity as an Agent

When working on this project:

- **Identify yourself** as: `claude-code-v1.0` (or your version)
- **Declare** your confidence level in decisions: `high | medium | low`
- **Record** your identification in the `agent:` field of the metadata

---

## 5. Git Operations

> **CRITICAL: Never commit directly to `main` branch.**

All changes must go through feature/fix branches and Pull Requests.

### Branch Prefixes

| Prefix | Purpose |
|--------|---------|
| `feature/` or `feat/` | New features |
| `fix/` | Bug fixes |
| `hotfix/` | Urgent production fixes |
| `docs/` | Documentation only |
| `refactor/` | Code refactoring |
| `test/` | Test changes |

### Conventional Commits

| Prefix | Use Case |
|--------|----------|
| `feat:` | New feature |
| `fix:` | Bug fix |
| `docs:` | Documentation only |
| `refactor:` | No behavior change |
| `chore:` | Maintenance |

### Quick Workflow

```bash
git checkout main && git pull origin main
git checkout -b fix/descriptive-name
# ... make changes and commits ...
git push -u origin fix/descriptive-name
gh pr create --title "fix: description" --body "..."
```

> **Full details:** `.devtrail/00-governance/GIT-BRANCHING-STRATEGY.md`

---

## 6. When to Document

### MANDATORY (create document)

| Situation | Action |
|-----------|--------|
| >10 lines of code in business logic | Create AILOG |
| Decision between technical alternatives | Create AIDEC |
| Changes in security/authentication | Create AILOG + mark `risk_level: high` |
| Personal data (GDPR/PII) | Create AILOG + request ETH |
| Integration with external service | Create AILOG |
| Change in public API or DB schema | Create AILOG |

### DO NOT DOCUMENT

- Trivial changes (whitespace, typos, formatting)
- Sensitive information (credentials, tokens, API keys)

---

## 7. File Naming Convention

```
[TYPE]-[YYYY-MM-DD]-[NNN]-[description].md
```

**Example**: `AILOG-2025-01-27-001-implement-oauth.md`

---

## 8. Minimum Metadata

```yaml
---
id: AILOG-2025-01-27-001
title: Brief description
status: accepted
created: 2025-01-27
agent: claude-code-v1.0
confidence: high | medium | low
review_required: true | false
risk_level: low | medium | high | critical
---
```

---

## 9. Autonomy Limits

| Type | I can do | Requires human |
|------|----------|----------------|
| **AILOG** | Create freely | - |
| **AIDEC** | Create freely | - |
| **ETH** | Create draft | Approval |
| **ADR** | Create draft | Review |
| **REQ** | Propose | Validation |
| **INC** | Contribute analysis | Conclusions |
| **TDE** | Identify | Prioritize |

---

## 10. Documentation Map (DevTrail)

> **IMPORTANT**: This is the complete project structure.
> Not everything is loaded in this session, but I can access any document when needed.

```
.devtrail/
├── 00-governance/              ← POLICIES AND RULES
│   ├── PRINCIPLES.md           # Project guiding principles
│   ├── DOCUMENTATION-POLICY.md # Complete documentation policy
│   ├── AGENT-RULES.md          # Detailed rules for AI agents
│   └── exceptions/             # Documented exceptions
│
├── 01-requirements/            ← REQUIREMENTS (REQ)
│   └── [REQ-*.md]              # System requirements
│
├── 02-design/                  ← DESIGN
│   └── decisions/              # ADRs (Architecture Decision Records)
│       └── [ADR-*.md]
│
├── 03-implementation/          ← IMPLEMENTATION GUIDES
│   └── [technical guides]
│
├── 04-testing/                 ← TESTING (TES)
│   └── [TES-*.md]              # Test strategies and plans
│
├── 05-operations/              ← OPERATIONS
│   ├── [runbooks]
│   └── incidents/              # Post-mortems (INC)
│       └── [INC-*.md]
│
├── 06-evolution/               ← EVOLUTION
│   └── technical-debt/         # Technical debt (TDE)
│       └── [TDE-*.md]
│
├── 07-ai-audit/                ← AI AGENT AUDIT
│   ├── agent-logs/             # Action logs (AILOG)
│   │   └── [AILOG-*.md]
│   ├── decisions/              # Agent decisions (AIDEC)
│   │   └── [AIDEC-*.md]
│   └── ethical-reviews/        # Ethical reviews (ETH)
│       └── [ETH-*.md]
│
├── templates/                  ← TEMPLATES
│   ├── TEMPLATE-AILOG.md
│   ├── TEMPLATE-AIDEC.md
│   ├── TEMPLATE-ADR.md
│   ├── TEMPLATE-ETH.md
│   ├── TEMPLATE-REQ.md
│   ├── TEMPLATE-INC.md
│   └── TEMPLATE-TDE.md
│
└── QUICK-REFERENCE.md          ← 1-page quick reference
```

---

## 11. When to Load Additional Documents

| Situation | Document to load |
|-----------|------------------|
| Going to create an AILOG | `.devtrail/templates/TEMPLATE-AILOG.md` |
| Going to create an AIDEC | `.devtrail/templates/TEMPLATE-AIDEC.md` |
| Going to create an ADR | `.devtrail/templates/TEMPLATE-ADR.md` |
| Going to create a REQ | `.devtrail/templates/TEMPLATE-REQ.md` |
| Questions about naming or metadata | `.devtrail/00-governance/DOCUMENTATION-POLICY.md` |
| Questions about my autonomy limits | `.devtrail/00-governance/AGENT-RULES.md` |
| Need to see existing requirements | List `.devtrail/01-requirements/` |
| Need to see existing ADRs | List `.devtrail/02-design/decisions/` |
| Need to see technical debt | List `.devtrail/06-evolution/technical-debt/` |

---

## 12. Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  1. EVALUATE if the change requires documentation          │
│     (see section 5)                                         │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  2. LOAD the corresponding template                         │
│     (see section 10)                                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  3. CREATE the document with correct naming                 │
│     [TYPE]-[YYYY-MM-DD]-[NNN]-[description].md             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  4. If risk_level: high/critical or confidence: low         │
│     → Mark review_required: true                            │
└─────────────────────────────────────────────────────────────┘
```

---

## 13. Quick Type Reference

| Prefix | Name | Location |
|--------|------|----------|
| `AILOG` | AI Action Log | `.devtrail/07-ai-audit/agent-logs/` |
| `AIDEC` | AI Decision | `.devtrail/07-ai-audit/decisions/` |
| `ETH` | Ethical Review | `.devtrail/07-ai-audit/ethical-reviews/` |
| `ADR` | Architecture Decision Record | `.devtrail/02-design/decisions/` |
| `REQ` | Requirement | `.devtrail/01-requirements/` |
| `TES` | Test Plan | `.devtrail/04-testing/` |
| `INC` | Incident Post-mortem | `.devtrail/05-operations/incidents/` |
| `TDE` | Technical Debt | `.devtrail/06-evolution/technical-debt/` |

---

*DevTrail v1.0.0 | Last updated: 2025-01-27*
*For complete rules: `.devtrail/00-governance/AGENT-RULES.md`*
*[Enigmora](https://enigmora.com) — Because every change tells a story.*


# LNXDrive Design Guide Reference

> This project is part of the **LNXDrive** ecosystem.
> A comprehensive design and development guide exists at: `lnxdrive-guide/`

---

## When to Consult the Guide

**MANDATORY** - Consult the guide before:

| Situation | Action |
|-----------|--------|
| Planning new features or components | Load relevant architecture docs |
| Making design decisions | Check existing ADRs and principles |
| Implementing a component | Load component specification |
| Writing tests | Load testing strategy docs |
| Uncertain about patterns or conventions | Load implementation guidelines |

---

## Quick Document Reference

### Understanding the Project
```
lnxdrive-guide/01-Vision/01-resumen-ejecutivo.md
lnxdrive-guide/01-Vision/02-principios-rectores.md
```

### Architecture & Design
```
lnxdrive-guide/03-Arquitectura/01-arquitectura-hexagonal.md
lnxdrive-guide/03-Arquitectura/02-capas-y-puertos.md
```

### Component Specifications
```
lnxdrive-guide/04-Componentes/01-files-on-demand-fuse.md    # FUSE
lnxdrive-guide/04-Componentes/02-ui-gnome.md                # GNOME UI
lnxdrive-guide/04-Componentes/03-ui-kde-plasma.md           # KDE UI
lnxdrive-guide/04-Componentes/04-ui-gtk3.md                 # XFCE/MATE UI
lnxdrive-guide/04-Componentes/05-ui-cosmic.md               # Cosmic UI
lnxdrive-guide/04-Componentes/06-cli.md                     # CLI
lnxdrive-guide/04-Componentes/07-motor-sincronizacion.md    # Sync engine
lnxdrive-guide/04-Componentes/08-microsoft-graph.md         # OneDrive API
lnxdrive-guide/04-Componentes/09-rate-limiting.md           # Throttling
lnxdrive-guide/04-Componentes/10-file-watching-inotify.md   # File watching
lnxdrive-guide/04-Componentes/11-conflictos.md              # Conflicts
lnxdrive-guide/04-Componentes/12-auditoria.md               # Audit system
lnxdrive-guide/04-Componentes/13-telemetria.md              # Telemetry
```

### Implementation Guidelines
```
lnxdrive-guide/05-Implementacion/01-stack-tecnologico.md
lnxdrive-guide/05-Implementacion/03-convenciones-nomenclatura.md
lnxdrive-guide/05-Implementacion/04-patrones-rust.md
lnxdrive-guide/05-Implementacion/05-configuracion-yaml.md
```

### Testing
```
lnxdrive-guide/06-Testing/01-estrategia-testing.md
lnxdrive-guide/06-Testing/03-testing-fuse.md
lnxdrive-guide/06-Testing/05-mocking-apis.md
lnxdrive-guide/06-Testing/06-ci-cd-pipeline.md
lnxdrive-guide/06-Testing/09-testing-seguridad.md
```

### Multi-Provider & Extensibility
```
lnxdrive-guide/07-Extensibilidad/01-arquitectura-multi-proveedor.md
lnxdrive-guide/07-Extensibilidad/02-puerto-icloudprovider.md
lnxdrive-guide/07-Extensibilidad/03-multi-cuenta-namespaces.md
```

### Risk Analysis
```
lnxdrive-guide/.devtrail/02-design/risk-analysis/TRACE-risks-mitigations.md
```

---

## Navigation by Task Type

| Task | Documents to Load |
|------|-------------------|
| **Implement Core component** | `03-Arquitectura/02-capas-y-puertos.md`, `04-Componentes/07-motor-sincronizacion.md` |
| **Implement FUSE** | `04-Componentes/01-files-on-demand-fuse.md`, `06-Testing/03-testing-fuse.md` |
| **Implement GNOME UI** | `04-Componentes/02-ui-gnome.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implement KDE UI** | `04-Componentes/03-ui-kde-plasma.md`, `08-Distribucion/02-comunicacion-dbus.md` |
| **Implement CLI** | `04-Componentes/06-cli.md` |
| **Add new cloud provider** | `07-Extensibilidad/02-puerto-icloudprovider.md` |
| **Write tests** | `06-Testing/01-estrategia-testing.md` + component-specific doc |
| **Debug issues** | `06-Testing/07-logging-tracing.md`, `06-Testing/08-automatizacion-depuracion.md` |
| **Check roadmap** | `09-Referencia/02-hoja-de-ruta.md` |

---

## Loading Strategy

1. **Minimal load**: Only the specific task document
2. **Contextual load**: Task document + related architecture document
3. **Full load**: Only for major planning or refactoring tasks

> **Main index**: `lnxdrive-guide/Guía-de-diseño-y-desarrollo.md`

---

*LNXDrive Guide v1.0 — [Enigmora](https://enigmora.com)*


# Context7 - Up-to-date Documentation Lookup

> **Always consult Context7 for the most recent documentation** when researching or implementing with external APIs, libraries, or frameworks.

This project has access to the **Context7 MCP server**, which provides real-time access to up-to-date documentation and code examples for any programming library or framework.

### When to Use Context7

| Situation | Action |
|-----------|--------|
| Implementing with a library (e.g., `tokio`, `reqwest`, `clap`, `serde`) | Query Context7 for current API usage and examples |
| Integrating an external API (e.g., Microsoft Graph, OAuth2) | Query Context7 for latest endpoints and patterns |
| Upgrading or adding a dependency | Query Context7 to verify current API surface and breaking changes |
| Unsure about a library's current best practices | Query Context7 before defaulting to training data |

### How to Use

1. **Resolve the library ID** using `resolve-library-id` with the library name.
2. **Query the documentation** using `query-docs` with the resolved ID and a specific question.

> **Important:** Prefer Context7 documentation over training data when there may be discrepancies, especially for fast-moving libraries or recently released versions.

---


## Active Technologies
- Rust 1.75+ (MSRV) + tokio (async runtime), reqwest (HTTP), oauth2-rs (OAuth2 PKCE), zbus (D-Bus), sqlx (SQLite), clap (CLI), serde (serialization), thiserror/anyhow (errors) (001-core-cli)
- SQLite 3.35+ (estado de sincronizacion, audit log) (001-core-cli)
- Rust 1.75+ (MSRV) + `fuser` 0.16 (FUSE protocol), `tokio` 1.35 (async runtime), `sqlx` 0.7 (SQLite), `reqwest` 0.12 (HTTP downloads), `dashmap` (concurrent inode table) (002-files-on-demand)
- SQLite 3.35+ (state repository, inode mapping) + filesystem cache (`~/.local/share/lnxdrive/cache/content/`) (002-files-on-demand)

## Recent Changes
- 001-core-cli: Added Rust 1.75+ (MSRV) + tokio (async runtime), reqwest (HTTP), oauth2-rs (OAuth2 PKCE), zbus (D-Bus), sqlx (SQLite), clap (CLI), serde (serialization), thiserror/anyhow (errors)
