---
id: AILOG-2026-02-03-005
title: Implement CLI auth commands and output formatting
status: accepted
created: 2026-02-03
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [cli, auth, output, clap]
related: [T127, T128, T129, T130, T131, T132, T133, T134]
---

# AILOG: Implement CLI auth commands and output formatting

## Summary

Implemented the CLI output formatting system and authentication subcommands for lnxdrive-cli. This establishes the command structure using clap's derive API, a trait-based output formatting layer supporting both human-readable and JSON output, and stub auth commands (login, logout, status) ready to be wired to the core authentication use cases.

## Context

The lnxdrive-cli crate had only a minimal main.rs entry point that printed the version. Tasks T127-T135 required building out the CLI infrastructure: output formatting (T131-T133), auth commands (T127-T130), and the main entry point update (T134).

## Actions Performed

1. Created `output.rs` with `OutputFormat` enum, `OutputFormatter` trait, `HumanFormatter` (checkmarks/indentation), and `JsonFormatter` (structured JSON output)
2. Created `commands/mod.rs` module declaration
3. Created `commands/auth.rs` with `AuthCommand` enum (Login, Logout, Status) using clap derive API, each with placeholder implementations
4. Updated `main.rs` with full clap-based CLI structure including global flags (--json, --verbose, --config, --quiet) and auth subcommand dispatch
5. Fixed dyn-compatibility issue: changed `print_json` from generic `T: Serialize` parameter to `&serde_json::Value` to allow trait object usage via `Box<dyn OutputFormatter>`

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-cli/Cargo.toml` | Already had all needed deps; no net change |
| `crates/lnxdrive-cli/src/output.rs` | New file: output formatting trait and implementations |
| `crates/lnxdrive-cli/src/commands/mod.rs` | New file: module declaration for auth |
| `crates/lnxdrive-cli/src/commands/auth.rs` | New file: AuthCommand with Login/Logout/Status subcommands |
| `crates/lnxdrive-cli/src/main.rs` | Rewritten: full clap Parser with global flags, Commands enum, tracing setup |

## Decisions Made

- **Changed `print_json` signature from generic to `serde_json::Value`**: The original spec used `fn print_json<T: Serialize>(&self, value: &T)` which makes the trait not dyn-compatible (cannot use `Box<dyn OutputFormatter>`). Changed to `fn print_json(&self, value: &serde_json::Value)` so callers convert their types with `serde_json::to_value()` before calling. This preserves the same functionality while enabling dynamic dispatch.

## Impact

- **Functionality**: Establishes the CLI command infrastructure. Auth commands are stubs that print status messages; they will be wired to real use cases when GraphCloudProvider is complete.
- **Performance**: N/A
- **Security**: N/A (no credentials handled yet; auth commands are placeholders)

## Verification

- [x] Code compiles without errors
- [ ] Tests pass (no tests added yet for CLI)
- [x] Manual review performed

## Additional Notes

The only compiler warnings are about `error` and `print_json` methods being unused, which is expected since they will be used as more commands are added.

---

<!-- Template: DevTrail | https://enigmora.com -->
