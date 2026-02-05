# FUSE Operations Contract: Files-on-Demand

**Branch**: `feat/002-files-on-demand` | **Date**: 2026-02-04

---

## Implemented FUSE Operations

The `LnxDriveFs` struct implements the `fuser::Filesystem` trait with the following operations:

### Metadata Operations

| Operation | Description | Behavior |
|-----------|-------------|----------|
| `init` | Filesystem initialization | Negotiate kernel capabilities, load inode table from SQLite |
| `destroy` | Filesystem shutdown | Flush pending writes, complete in-progress hydrations |
| `lookup` | Resolve directory entry by name | Query inode table, return FileAttr with TTL |
| `getattr` | Get file attributes | Return real file size for placeholders (not 0), <1ms target |
| `setattr` | Change file attributes | Update permissions, timestamps, truncate |
| `statfs` | Filesystem statistics | Report total/free space based on OneDrive quota |
| `forget` | Kernel drops inode reference | Decrement lookup_count, GC if zero |

### Directory Operations

| Operation | Description | Behavior |
|-----------|-------------|----------|
| `readdir` | List directory contents | Serve from inode table (no network), include `.` and `..` |
| `opendir` | Open directory handle | Validate directory exists, allocate fh |
| `releasedir` | Close directory handle | Release fh |
| `mkdir` | Create directory | Create entry in state repo, queue for sync |
| `rmdir` | Remove directory | Verify empty, mark deleted, queue for sync |

### File Operations

| Operation | Description | Behavior |
|-----------|-------------|----------|
| `open` | Open a file | Trigger hydration if placeholder, track open handles |
| `read` | Read file data | Serve from cache, block if hydrating requested range |
| `write` | Write file data | Require hydration first, write to cache, mark modified |
| `create` | Create new file | Create entry, allocate inode, mark modified |
| `unlink` | Delete file | Mark deleted, queue for sync |
| `rename` | Move/rename file | Update inode table + state repo, queue for sync |
| `flush` | Sync dirty data | No-op (writes go to cache immediately) |
| `release` | Close file handle | Decrement open handles, allow dehydration |

### Extended Attributes

| Operation | Description | Behavior |
|-----------|-------------|----------|
| `getxattr` | Get extended attribute | Serve `user.lnxdrive.*` attributes from inode state |
| `setxattr` | Set extended attribute | Only allow internal state changes (reject external writes) |
| `listxattr` | List extended attributes | Return list of `user.lnxdrive.*` names |
| `removexattr` | Remove extended attribute | Reject (attributes are system-managed) |

---

## Extended Attributes Namespace

| Attribute | Values | Read | Write |
|-----------|--------|------|-------|
| `user.lnxdrive.state` | `online`, `hydrating`, `hydrated`, `pinned`, `modified` | Yes | No (system-managed) |
| `user.lnxdrive.size` | File size as string (e.g., `"1048576"`) | Yes | No |
| `user.lnxdrive.remote_id` | OneDrive item ID string | Yes | No |
| `user.lnxdrive.progress` | `"0"` to `"100"` (during hydration) | Yes | No |

---

## Error Mapping

| Condition | FUSE errno |
|-----------|-----------|
| Item not found in inode table | `ENOENT` |
| Permission denied | `EACCES` |
| File already exists (create) | `EEXIST` |
| Directory not empty (rmdir) | `ENOTEMPTY` |
| Network/cloud API failure | `EIO` |
| Not a directory (readdir on file) | `ENOTDIR` |
| Is a directory (open on dir) | `EISDIR` |
| Disk full during hydration | `ENOSPC` |
| xattr not found | `ENODATA` |
| xattr buffer too small | `ERANGE` |
| Invalid argument | `EINVAL` |
| File name too long | `ENAMETOOLONG` |

---

## Mount Options

```
MountOption::AutoUnmount     # Unmount when process exits
MountOption::FSName("lnxdrive")
MountOption::Subtype("onedrive")
MountOption::DefaultPermissions  # Kernel checks permissions
MountOption::NoAtime         # Don't update atime on every read
MountOption::Async           # Async I/O
```

Note: `AllowOther` is NOT set by default (requires `/etc/fuse.conf` with `user_allow_other`).
