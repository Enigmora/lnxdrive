---
id: AILOG-2026-02-05-003
title: Implement mount() and unmount() functions for FUSE filesystem
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, mount, files-on-demand, T040, T041]
related: [AILOG-2026-02-05-001, AILOG-2026-02-05-002]
---

# AILOG: Implement mount() and unmount() functions for FUSE filesystem

## Summary

Implemented the public `mount()` and `unmount()` functions in `lnxdrive-fuse/src/lib.rs` as specified in tasks T040 and T041. These functions provide the public API for mounting and unmounting the LNXDrive FUSE filesystem for the Files-on-Demand feature.

## Context

The LNXDrive project requires a FUSE filesystem implementation to provide Files-on-Demand functionality. Tasks T040 and T041 specify the implementation of the `mount()` and `unmount()` functions that will be used by the daemon to control the FUSE filesystem lifecycle.

## Actions Performed

1. Added `dirs = "5.0"` dependency to `lnxdrive-fuse/Cargo.toml` for tilde expansion in paths
2. Implemented `expand_tilde()` helper function to expand `~/` prefix to user's home directory
3. Implemented `mount()` function with:
   - Mount point validation (exists, is directory, is empty)
   - ContentCache creation from config.cache_dir
   - LnxDriveFs instance creation
   - FUSE mount with specified options (AutoUnmount, FSName, Subtype, DefaultPermissions, NoAtime, Async)
4. Implemented `unmount()` function that drops the BackgroundSession to trigger unmount
5. Added comprehensive documentation with examples for all public functions
6. Uncommented `pub use filesystem::LnxDriveFs;` re-export

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-fuse/Cargo.toml` | Added `dirs = "5.0"` dependency |
| `crates/lnxdrive-fuse/src/lib.rs` | Added mount(), unmount(), expand_tilde() functions and required imports |

## Decisions Made

1. **Tilde expansion**: Used `strip_prefix()` method instead of manual string slicing to satisfy clippy lint
2. **Mount options**: Used the exact options specified in T040 requirements
3. **Error handling**: Used existing `FuseError` variants (NotFound, NotADirectory, NotEmpty, IoError) for validation errors

## Impact

- **Functionality**: Enables mounting/unmounting of the FUSE filesystem via public API
- **Performance**: N/A - mount/unmount are one-time operations
- **Security**: Mount point validation ensures the filesystem is mounted on valid, empty directories

## Verification

- [x] Code compiles without errors
- [x] Tests pass (37 tests in lnxdrive-fuse)
- [x] Clippy passes with no warnings
- [x] Formatting passes for lib.rs

## Additional Notes

The `unmount()` function is intentionally simple since dropping the `BackgroundSession` handle triggers the actual unmount. The function exists for API clarity and explicit intent.

---

<!-- Template: DevTrail | https://enigmora.com -->
