---
id: AILOG-2026-02-05-009
title: Implement Files-on-Demand download URL and streaming methods (T059/T060)
status: accepted
created: 2026-02-05
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [fuse, files-on-demand, streaming, download]
related: [US2, T059, T060]
---

# AILOG: Implement Files-on-Demand download URL and streaming methods

## Summary

Implemented T059 and T060 for the Files-on-Demand feature. Added three new methods to `GraphCloudProvider`:
- `get_download_url()` - retrieves the pre-authenticated `@microsoft.graph.downloadUrl` field
- `download_file_to_disk()` - streaming download directly to disk without loading into memory
- `download_range()` - partial file download using HTTP Range header

## Context

The Files-on-Demand feature requires efficient file download capabilities for the FUSE filesystem. The existing `download_file()` method loads entire files into memory, which is unsuitable for large files. These new methods enable:
1. Getting a pre-authenticated direct download URL (bypasses Graph API)
2. Streaming downloads to disk (memory efficient)
3. Partial downloads for random access file reading

## Actions Performed

1. Added `base_url()` and `client()` accessor methods to `GraphClient` in `client.rs`
2. Added `futures-util = "0.3"` dependency to `lnxdrive-graph/Cargo.toml`
3. Added `stream` feature to reqwest in workspace `Cargo.toml`
4. Implemented `get_download_url()` method in `provider.rs`
5. Implemented `download_file_to_disk()` method with streaming response
6. Implemented `download_range()` method with HTTP Range header support
7. Added proper imports for `std::path::Path`, `futures_util::StreamExt`, and tokio I/O traits

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-graph/src/client.rs` | Added `base_url()` and `client()` public accessor methods |
| `crates/lnxdrive-graph/src/provider.rs` | Added imports and three new download methods to `GraphCloudProvider` |
| `crates/lnxdrive-graph/Cargo.toml` | Added `futures-util = "0.3"` dependency |
| `Cargo.toml` | Added `stream` feature to reqwest workspace dependency |

## Decisions Made

1. **Methods as inherent impl, not trait methods**: The new download methods are added as inherent methods on `GraphCloudProvider` rather than extending `ICloudProvider` trait. This keeps the trait focused on core operations while allowing FUSE-specific functionality.

2. **Use `truncate(false)` for range downloads**: In `download_range()`, we explicitly set `truncate(false)` when opening the destination file because we're writing to a specific offset and want to preserve the rest of the file content.

3. **Streaming via `bytes_stream()`**: Used reqwest's streaming response to avoid loading entire files into memory during download.

## Impact

- **Functionality**: Enables Files-on-Demand with efficient file access patterns
- **Performance**: Streaming downloads avoid memory pressure for large files; range downloads enable lazy loading
- **Security**: Download URLs are pre-authenticated but short-lived (~1 hour)

## Verification

- [x] Code compiles without errors (`cargo check --workspace`)
- [x] Tests pass (104 unit tests + 13 integration tests)
- [x] Clippy passes with no warnings
- [x] All existing functionality preserved

## Additional Notes

- The `@microsoft.graph.downloadUrl` is a pre-authenticated URL that bypasses the Graph API and goes directly to Azure blob storage
- Download URLs are typically valid for ~1 hour before expiring
- The `download_range()` method can be used by FUSE `read()` operations for on-demand block fetching

---

<!-- Template: DevTrail | https://enigmora.com -->
