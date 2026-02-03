# DevTrail - Quick Reference

> One-page reference for AI agents and developers.

---

## Language Configuration

**File**: `.devtrail/config.yml`

```yaml
language: en  # Options: en, es (default: en)
```

| Language | Template Path |
|----------|---------------|
| `en` | `.devtrail/templates/TEMPLATE-*.md` |
| `es` | `.devtrail/templates/i18n/es/TEMPLATE-*.md` |

---

## Naming Convention

```
[TYPE]-[YYYY-MM-DD]-[NNN]-[description].md
```

**Example**: `AILOG-2025-01-27-001-implement-oauth.md`

---

## Document Types

| Type | Name | Location | Agent Autonomy |
|------|------|----------|----------------|
| `AILOG` | AI Action Log | `07-ai-audit/agent-logs/` | Create freely |
| `AIDEC` | AI Decision | `07-ai-audit/decisions/` | Create freely |
| `ETH` | Ethical Review | `07-ai-audit/ethical-reviews/` | Draft only |
| `ADR` | Architecture Decision | `02-design/decisions/` | Requires review |
| `REQ` | Requirement | `01-requirements/` | Propose |
| `TES` | Test Plan | `04-testing/` | Propose |
| `INC` | Incident Post-mortem | `05-operations/incidents/` | Contribute |
| `TDE` | Technical Debt | `06-evolution/technical-debt/` | Identify |

---

## When to Document

| Situation | Action |
|-----------|--------|
| >10 lines business logic | AILOG |
| Decision between alternatives | AIDEC |
| Security/authentication | AILOG + `risk_level: high` |
| Personal data (PII) | AILOG + ETH |
| External integration | AILOG |
| API/DB change | AILOG |

**DO NOT document**: whitespace, typos, credentials.

---

## Minimum Metadata

```yaml
---
id: AILOG-2025-01-27-001
title: Brief description
status: accepted
created: 2025-01-27
agent: agent-name-v1.0
confidence: high | medium | low
review_required: true | false
risk_level: low | medium | high | critical
---
```

---

## Human Review Required

Mark `review_required: true` when:

- `confidence: low`
- `risk_level: high | critical`
- Security decisions
- Irreversible changes
- ETH, ADR, REQ documents

---

## Folder Structure

```
.devtrail/
├── 00-governance/           ← Policies
├── 01-requirements/         ← REQ
├── 02-design/decisions/     ← ADR
├── 03-implementation/       ← Guides
├── 04-testing/              ← TES
├── 05-operations/incidents/ ← INC
├── 06-evolution/technical-debt/ ← TDE
├── 07-ai-audit/
│   ├── agent-logs/          ← AILOG
│   ├── decisions/           ← AIDEC
│   └── ethical-reviews/     ← ETH
└── templates/               ← Templates
```

---

## Workflow

```
1. EVALUATE → Does it require documentation?
       ↓
2. LOAD → Corresponding template
       ↓
3. CREATE → With correct naming
       ↓
4. MARK → review_required if applicable
```

---

## Levels

### Confidence
| Level | Action |
|-------|--------|
| `high` | Proceed |
| `medium` | Document alternatives |
| `low` | `review_required: true` |

### Risk
| Level | Examples |
|-------|----------|
| `low` | Docs, formatting |
| `medium` | New functionality |
| `high` | Security, APIs |
| `critical` | Production, irreversible |

---

## Skills

| Command | Purpose |
|---------|---------|
| `/devtrail-status` | Check documentation status and compliance |
| `/devtrail-new` | Create any document type (AI suggests) |
| `/devtrail-ailog` | Quick AILOG creation |
| `/devtrail-aidec` | Quick AIDEC creation |
| `/devtrail-adr` | Quick ADR creation |

---

## Quick Scenarios

| I just... | Create |
|-----------|--------|
| Implemented >10 lines | AILOG |
| Chose between options | AIDEC |
| Fixed security issue | AILOG + `risk_level: high` |
| Found tech debt | TDE |
| Handled PII data | AILOG + ETH |

📖 **See [docs/cookbook.md](../docs/cookbook.md) for detailed examples.**

---

*DevTrail v2.0.0 | [Handbook](https://enigmora.github.io/devtrail/) | [Enigmora](https://enigmora.com)*
