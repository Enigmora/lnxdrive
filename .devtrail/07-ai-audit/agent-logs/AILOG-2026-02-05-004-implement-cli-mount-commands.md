---
id: AILOG-2026-02-05-004
title: Implement CLI mount and unmount commands
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [cli, fuse, mount, files-on-demand, T042, T043]
related: [AILOG-2026-02-05-003]
---

# AILOG: Implement CLI mount and unmount commands

## Summary

Implemented the `lnxdrive mount` and `lnxdrive unmount` CLI commands as specified in tasks T042 and T043. These commands provide user-facing interfaces for mounting and unmounting the Files-on-Demand FUSE filesystem.

## Context

Following the implementation of the core mount/unmount functions in T040/T041, the CLI needs corresponding commands for users to interact with the FUSE filesystem. T042 specifies the mount command with foreground mode and JSON output support, while T043 requires registration in the CLI command structure.

## Actions Performed

1. Created `crates/lnxdrive-cli/src/commands/mount.rs` with:
   - `MountCommand` struct with clap derive for:
     - `--path <PATH>` optional mount point override
     - `--foreground` / `-f` flag for foreground execution
     - `--json` flag for JSON output
   - `execute()` method that validates prerequisites and mounts the filesystem
   - `UnmountCommand` struct with `--force` and `--path` flags
   - Helper functions: `expand_tilde()`, `is_mount_point_suitable()`, `which_exists()`

2. Updated `crates/lnxdrive-cli/src/commands/mod.rs`:
   - Added `pub mod mount;` declaration

3. Updated `crates/lnxdrive-cli/src/main.rs`:
   - Added imports for `MountCommand` and `UnmountCommand`
   - Added `Mount(MountCommand)` and `Unmount(UnmountCommand)` variants to Commands enum
   - Added match arms in the execute function

4. Updated `crates/lnxdrive-cli/Cargo.toml`:
   - Added `fuser.workspace = true` dependency
   - Added `[dev-dependencies]` section with `tempfile.workspace = true`

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-cli/src/commands/mount.rs` | New file: MountCommand and UnmountCommand implementations |
| `crates/lnxdrive-cli/src/commands/mod.rs` | Added mount module declaration |
| `crates/lnxdrive-cli/src/main.rs` | Added Mount/Unmount command variants and match arms |
| `crates/lnxdrive-cli/Cargo.toml` | Added fuser and tempfile dependencies |

## Decisions Made

1. **Mount validation**: The mount command validates:
   - Authenticated account exists
   - Mount point exists (creates if needed)
   - Mount point is empty or has only hidden files
   - FUSE is available at `/dev/fuse`

2. **Foreground mode**: When `--foreground` is specified, the command waits for Ctrl+C and then cleanly unmounts

3. **Background mode**: Uses `std::mem::forget(session)` to keep the FUSE mount active after the CLI exits

4. **Unmount implementation**: Uses `fusermount -u` (or `fusermount3`) via Command for simplicity rather than managing session handles directly

5. **Tilde expansion**: Used `strip_prefix()` for path expansion to satisfy clippy lints

## Impact

- **Functionality**: Users can now mount/unmount the FUSE filesystem via CLI
- **Performance**: N/A - mount/unmount are one-time operations
- **Security**: Validates prerequisites before mounting; force unmount available for busy filesystems

## Verification

- [x] Code compiles without errors
- [x] All 56 tests pass
- [x] Clippy passes with no warnings
- [x] Code formatting passes
- [x] CLI help shows new commands

## Additional Notes

The mount command supports both human-readable and JSON output formats. When running in foreground mode, the filesystem remains mounted until Ctrl+C is pressed. The unmount command uses fusermount for portability across different Linux distributions and FUSE versions (FUSE 2 and FUSE 3).

---

<!-- Template: DevTrail | https://enigmora.com -->
