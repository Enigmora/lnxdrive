# Quickstart: Files-on-Demand (FUSE)

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04

---

## Prerequisites

1. **Authenticated account**: `lnxdrive auth login` (from Fase 1)
2. **Initial sync completed**: `lnxdrive sync` at least once (populates state repository)
3. **FUSE support**:
   ```bash
   # Fedora/RHEL
   sudo dnf install fuse3 fuse3-devel

   # Ubuntu/Debian
   sudo apt install fuse3 libfuse3-dev

   # Verify
   ls /dev/fuse  # must exist
   ```

## Basic Usage

### Mount

```bash
# Mount with default path (~/OneDrive)
lnxdrive mount

# Mount at custom path
lnxdrive mount --path /mnt/onedrive

# Browse — no downloads happen
ls -la ~/OneDrive/Documents/
```

### Access Files

```bash
# Opening a file triggers automatic download
cat ~/OneDrive/Documents/notes.txt

# Check file state
getfattr -n user.lnxdrive.state ~/OneDrive/Documents/notes.txt
# user.lnxdrive.state="hydrated"

# Edit files — changes sync automatically
echo "new line" >> ~/OneDrive/Documents/notes.txt
```

### Pin for Offline

```bash
# Pin important files
lnxdrive pin ~/OneDrive/Documents/critical.pdf

# Pin entire directory
lnxdrive pin ~/OneDrive/Projects/

# Check what's pinned
lnxdrive status
```

### Manage Space

```bash
# Manually free space
lnxdrive dehydrate ~/OneDrive/Videos/

# Check cache usage
lnxdrive status
```

### Unmount

```bash
lnxdrive unmount
```

## Configuration

Add to `~/.config/lnxdrive/config.yaml`:

```yaml
fuse:
  mount_point: ~/OneDrive
  auto_mount: true
  cache_dir: ~/.local/share/lnxdrive/cache
  cache_max_size_gb: 10
  dehydration_threshold_percent: 80
  dehydration_max_age_days: 30
  dehydration_interval_minutes: 60
  hydration_concurrency: 8
```

## Daemon Integration

When the daemon starts with `auto_mount: true`, the FUSE filesystem mounts automatically:

```bash
lnxdrive daemon start   # mounts FUSE automatically
lnxdrive daemon stop    # unmounts FUSE cleanly
```

## Validation

```bash
# 1. Verify mount
mount | grep lnxdrive

# 2. Verify placeholder behavior
stat ~/OneDrive/somefile.pdf    # shows real size, not 0
getfattr -d ~/OneDrive/somefile.pdf  # shows user.lnxdrive.* attrs

# 3. Verify hydration
cat ~/OneDrive/somefile.pdf > /dev/null  # triggers download
getfattr -n user.lnxdrive.state ~/OneDrive/somefile.pdf
# user.lnxdrive.state="hydrated"

# 4. Verify dehydration
lnxdrive dehydrate ~/OneDrive/somefile.pdf
getfattr -n user.lnxdrive.state ~/OneDrive/somefile.pdf
# user.lnxdrive.state="online"
```
