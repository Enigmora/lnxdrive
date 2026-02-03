# DevTrail - Guidelines for GitHub Copilot

> This file configures GitHub Copilot CLI under DevTrail.

## Language Configuration

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

## Documentation Reporting

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
DevTrail: Documentation pending - review required
```

This transparency helps users verify compliance with DevTrail rules.

## Fundamental Principle

> **"No significant change without a documented trace."**

## Your Identity as an Agent

When working on this project:

- **Identify yourself** as: `copilot-cli-v1.0`
- **Declare** your confidence level in decisions: `high | medium | low`
- **Record** your identification in the `agent:` field of the metadata

## When to Document

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

## File Naming Convention

```
[TYPE]-[YYYY-MM-DD]-[NNN]-[description].md
```

**Example**: `AILOG-2025-01-27-001-implement-oauth.md`

## Required Metadata

```yaml
---
id: AILOG-2025-01-27-001
title: Brief description
status: accepted
created: 2025-01-27
agent: copilot-cli-v1.0
confidence: high | medium | low
review_required: true | false
risk_level: low | medium | high | critical
---
```

## Autonomy Limits

| Type | Autonomy |
|------|----------|
| AILOG | Create freely |
| AIDEC | Create freely |
| ETH | Draft only → human approves |
| ADR | Create → requires review |
| REQ | Propose → human validates |
| TDE | Identify yes, prioritize no |

## Documentation Map (DevTrail)

```
.devtrail/
├── 00-governance/          ← Policies (load if in doubt)
│   ├── AGENT-RULES.md      # Detailed rules
│   └── DOCUMENTATION-POLICY.md
├── 01-requirements/        ← REQ-*.md
├── 02-design/decisions/    ← ADR-*.md
├── 04-testing/             ← TES-*.md
├── 05-operations/incidents/← INC-*.md
├── 06-evolution/technical-debt/ ← TDE-*.md
├── 07-ai-audit/
│   ├── agent-logs/         ← AILOG-*.md (create here)
│   ├── decisions/          ← AIDEC-*.md (create here)
│   └── ethical-reviews/    ← ETH-*.md
└── templates/              ← Templates (load when creating)
```

## When to Load Templates

| Need to | Load |
|---------|------|
| Create AILOG | `.devtrail/templates/TEMPLATE-AILOG.md` |
| Create AIDEC | `.devtrail/templates/TEMPLATE-AIDEC.md` |
| Create ADR | `.devtrail/templates/TEMPLATE-ADR.md` |
| Naming questions | `.devtrail/00-governance/DOCUMENTATION-POLICY.md` |
| Autonomy questions | `.devtrail/00-governance/AGENT-RULES.md` |

## Quick Type Reference

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

## Human Review Required

Mark `review_required: true` when:
- `confidence: low`
- `risk_level: high | critical`
- Security decisions
- Irreversible changes

---

*DevTrail v1.0.0 | [Enigmora](https://enigmora.com)*
