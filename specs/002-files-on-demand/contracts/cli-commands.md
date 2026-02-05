# CLI Commands Contract: Files-on-Demand (Fase 2)

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04

---

## New Commands

### `mount` - Mount FUSE Filesystem

Mount the OneDrive virtual filesystem.

```bash
lnxdrive mount [OPTIONS]
```

**Options**:
- `--path <PATH>` - Override mount point (default: from config, `~/OneDrive`)
- `--foreground` / `-f` - Run in foreground (don't daemonize)

**Output (human)**:
```
✓ Mounted OneDrive at ~/OneDrive
  Items: 1,234 files, 56 directories
  Cache: 2.1 GB / 10 GB
```

**Output (JSON)**:
```json
{
  "success": true,
  "mount_point": "/home/user/OneDrive",
  "items_count": 1290,
  "cache_used_bytes": 2254857830,
  "cache_max_bytes": 10737418240
}
```

**Errors**:
- Mount point does not exist → `"Mount point does not exist: ~/OneDrive"`
- Mount point not empty → `"Mount point is not empty: ~/OneDrive"`
- FUSE not available → `"FUSE not available. Install libfuse3: sudo dnf install fuse3"`
- Already mounted → `"OneDrive is already mounted at ~/OneDrive"`
- No authenticated account → `"No authenticated account. Run: lnxdrive auth login"`

**Exit codes**: 0 = success, 1 = error

---

### `unmount` - Unmount FUSE Filesystem

Unmount the OneDrive virtual filesystem.

```bash
lnxdrive unmount [OPTIONS]
```

**Options**:
- `--force` - Force unmount even if files are in use

**Output (human)**:
```
✓ Unmounted OneDrive from ~/OneDrive
```

**Output (JSON)**:
```json
{
  "success": true,
  "mount_point": "/home/user/OneDrive"
}
```

**Errors**:
- Not mounted → `"OneDrive is not currently mounted"`
- Files in use → `"Cannot unmount: 3 files in use. Use --force to force unmount"`

**Exit codes**: 0 = success, 1 = error

---

### `pin` - Pin Files for Offline Access

Mark files or directories as always available offline.

```bash
lnxdrive pin <PATH>...
```

**Arguments**:
- `<PATH>` - One or more paths to pin (supports globs)

**Output (human)**:
```
✓ Pinned 3 items
  📌 ~/OneDrive/Documents/report.pdf (hydrating...)
  📌 ~/OneDrive/Documents/notes.txt (already hydrated)
  📌 ~/OneDrive/Projects/ (directory, 12 items)
```

**Output (JSON)**:
```json
{
  "success": true,
  "pinned": [
    {"path": "/home/user/OneDrive/Documents/report.pdf", "state": "hydrating"},
    {"path": "/home/user/OneDrive/Documents/notes.txt", "state": "pinned"},
    {"path": "/home/user/OneDrive/Projects/", "state": "pinned", "items": 12}
  ]
}
```

**Errors**:
- Path not in mount → `"Path is not within the OneDrive mount: /tmp/file"`
- Path not found → `"File not found: ~/OneDrive/missing.txt"`

**Exit codes**: 0 = success, 1 = error

---

### `unpin` - Unpin Files

Remove the offline pin from files or directories.

```bash
lnxdrive unpin <PATH>...
```

**Arguments**:
- `<PATH>` - One or more paths to unpin

**Output (human)**:
```
✓ Unpinned 2 items (now eligible for dehydration)
```

**Output (JSON)**:
```json
{
  "success": true,
  "unpinned": [
    {"path": "/home/user/OneDrive/Documents/report.pdf", "state": "hydrated"},
    {"path": "/home/user/OneDrive/Projects/", "state": "hydrated", "items": 12}
  ]
}
```

**Exit codes**: 0 = success, 1 = error

---

### `hydrate` - Download File Content

Manually download the content of a placeholder file.

```bash
lnxdrive hydrate <PATH>...
```

**Arguments**:
- `<PATH>` - One or more paths to hydrate

**Output (human)**:
```
⟳ Hydrating report.pdf... 45%
✓ Hydrated 1 file (15.2 MB downloaded)
```

**Output (JSON)**:
```json
{
  "success": true,
  "hydrated": [
    {"path": "/home/user/OneDrive/report.pdf", "size_bytes": 15938355}
  ]
}
```

**Errors**:
- Already hydrated → `"File is already hydrated: report.pdf"`
- Network error → `"Download failed: connection timeout. Retry with: lnxdrive hydrate report.pdf"`

**Exit codes**: 0 = success, 1 = error

---

### `dehydrate` - Remove Local Content

Manually remove local content of hydrated files, reverting them to placeholders.

```bash
lnxdrive dehydrate <PATH>... [OPTIONS]
```

**Arguments**:
- `<PATH>` - One or more paths to dehydrate

**Options**:
- `--force` - Dehydrate even if file is pinned (unpins first)

**Output (human)**:
```
✓ Dehydrated 2 files (freed 45.3 MB)
```

**Output (JSON)**:
```json
{
  "success": true,
  "dehydrated": [
    {"path": "/home/user/OneDrive/video.mp4", "freed_bytes": 47534284}
  ],
  "skipped": [
    {"path": "/home/user/OneDrive/pinned.pdf", "reason": "pinned"}
  ]
}
```

**Errors**:
- File is open → `"Cannot dehydrate: file is in use by another process"`
- File has pending changes → `"Cannot dehydrate: file has unsaved changes pending sync"`
- Already a placeholder → `"File is already a placeholder: report.pdf"`

**Exit codes**: 0 = success, 1 = error

---

## Modified Commands

### `status` (existing) - Extended Output

The existing `lnxdrive status` command gains FUSE-related information.

**Additional output (human)**:
```
FUSE:
  Mount: ~/OneDrive (mounted)
  Cache: 2.1 GB / 10 GB (21%)
  Files: 234 hydrated, 12 pinned, 988 online-only
  Hydrating: 2 files in progress
```

**Additional JSON fields**:
```json
{
  "fuse": {
    "mounted": true,
    "mount_point": "/home/user/OneDrive",
    "cache_used_bytes": 2254857830,
    "cache_max_bytes": 10737418240,
    "files_hydrated": 234,
    "files_pinned": 12,
    "files_online": 988,
    "files_hydrating": 2
  }
}
```

### `daemon start` (existing) - Auto-mount

When the daemon starts and `fuse.auto_mount` is `true` in config, the FUSE filesystem is mounted automatically.
