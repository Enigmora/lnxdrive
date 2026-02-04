# CLI Commands Contract: Core + CLI (Fase 1)

**Branch**: `001-core-cli` | **Date**: 2026-02-03

## Command Structure

```
lnxdrive <COMMAND> [OPTIONS]
```

**Global Options** (apply to all commands):
- `--json` - Output in JSON format
- `--quiet` / `-q` - Minimal output
- `--verbose` / `-v` - Verbose output (can be repeated: -vv, -vvv)
- `--config <PATH>` - Use alternate config file
- `--help` / `-h` - Show help
- `--version` / `-V` - Show version

---

## `auth` - Authentication Commands

### `auth login`

Authenticate with OneDrive.

```bash
lnxdrive auth login [OPTIONS]
```

**Options**:
- `--app-id <ID>` - Use custom Azure App ID

**Output (human)**:
```
Opening browser for Microsoft login...
Waiting for authentication...
✓ Authenticated as user@example.com
  Storage: 5.2 GB / 15 GB used
```

**Output (JSON)**:
```json
{
  "success": true,
  "email": "user@example.com",
  "display_name": "User Name",
  "quota_used": 5583457280,
  "quota_total": 16106127360
}
```

**Exit codes**:
- `0` - Success
- `1` - Authentication failed
- `2` - User cancelled

---

### `auth logout`

Remove stored credentials.

```bash
lnxdrive auth logout
```

**Output (human)**:
```
✓ Logged out successfully
  Credentials removed from keyring
```

**Output (JSON)**:
```json
{
  "success": true,
  "message": "Credentials removed"
}
```

**Exit codes**:
- `0` - Success
- `1` - No credentials to remove

---

### `auth status`

Check authentication status.

```bash
lnxdrive auth status
```

**Output (human)**:
```
Account: user@example.com
Status: Authenticated
Storage: 5.2 GB / 15 GB (34.7%)
Last sync: 2026-02-03 14:30:00 UTC
```

**Output (JSON)**:
```json
{
  "authenticated": true,
  "email": "user@example.com",
  "display_name": "User Name",
  "quota_used": 5583457280,
  "quota_total": 16106127360,
  "quota_percent": 34.7,
  "last_sync": "2026-02-03T14:30:00Z",
  "token_expires": "2026-02-03T15:30:00Z"
}
```

**Exit codes**:
- `0` - Authenticated
- `1` - Not authenticated

---

## `sync` - Synchronization Commands

### `sync` (default)

Run synchronization.

```bash
lnxdrive sync [OPTIONS]
```

**Options**:
- `--full` - Force full sync (ignore delta token)
- `--dry-run` - Show what would be synced without syncing

**Output (human)**:
```
Starting sync...
↓ Downloading: document.pdf (2.3 MB)
↑ Uploading: notes.txt (1.2 KB)
✓ Sync complete
  Downloaded: 3 files (5.2 MB)
  Uploaded: 2 files (45 KB)
  Errors: 0
  Duration: 12.3s
```

**Output (JSON)**:
```json
{
  "success": true,
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "downloaded": {
    "count": 3,
    "bytes": 5452595
  },
  "uploaded": {
    "count": 2,
    "bytes": 46080
  },
  "errors": [],
  "duration_ms": 12345
}
```

**Exit codes**:
- `0` - Success
- `1` - Sync failed
- `2` - Partial success (some items failed)

---

## `status` - Status Commands

### `status` (global)

Show overall sync status.

```bash
lnxdrive status [PATH]
```

**Arguments**:
- `[PATH]` - Optional path to check specific file/folder

**Output (human, global)**:
```
Sync Status: Idle
Last sync: 2026-02-03 14:30:00 UTC (5 minutes ago)

Items:
  Synced: 1,234
  Pending: 3
  Errors: 1
  Conflicts: 0

Pending:
  ↑ ~/OneDrive/document.pdf (uploading)
  ↓ ~/OneDrive/photos/img001.jpg (queued)
  ↓ ~/OneDrive/photos/img002.jpg (queued)

Errors:
  ✗ ~/OneDrive/locked-file.xlsx - File in use by another application
```

**Output (human, specific path)**:
```
File: ~/OneDrive/document.pdf
State: Modified (pending upload)
Local: 2026-02-03 14:25:00 (2.3 MB)
Remote: 2026-02-03 14:20:00 (2.1 MB)
Hash match: No (local newer)
```

**Output (JSON)**:
```json
{
  "state": "Idle",
  "last_sync": "2026-02-03T14:30:00Z",
  "items": {
    "synced": 1234,
    "pending": 3,
    "errors": 1,
    "conflicts": 0
  },
  "pending": [
    {
      "path": "~/OneDrive/document.pdf",
      "state": "Uploading",
      "progress": 0.45
    }
  ],
  "errors": [
    {
      "path": "~/OneDrive/locked-file.xlsx",
      "error": "File in use",
      "since": "2026-02-03T14:25:00Z"
    }
  ]
}
```

**Exit codes**:
- `0` - Status retrieved
- `1` - Error retrieving status

---

## `explain` - Explain Item State

### `explain <PATH>`

Explain why a file is in its current state.

```bash
lnxdrive explain <PATH>
```

**Output (human)**:
```
File: ~/OneDrive/document.pdf
State: Error

Explanation:
  This file could not be uploaded because it is currently open in another
  application (LibreOffice Writer). The system has been attempting to upload
  since 14:25:00 UTC (35 minutes ago).

Suggested actions:
  1. Close the application that has this file open
  2. Run 'lnxdrive sync' to retry the upload
  3. Or wait for the next automatic sync attempt (in ~25 minutes)

History:
  14:25:00 - Modified locally
  14:25:02 - Upload attempted - Failed (file locked)
  14:30:00 - Retry attempted - Failed (file locked)
  14:40:00 - Retry attempted - Failed (file locked)
```

**Output (JSON)**:
```json
{
  "path": "~/OneDrive/document.pdf",
  "state": "Error",
  "explanation": "File is locked by another application",
  "error_code": "FILE_LOCKED",
  "since": "2026-02-03T14:25:00Z",
  "retry_count": 3,
  "next_retry": "2026-02-03T15:00:00Z",
  "suggestions": [
    "Close the application that has this file open",
    "Run 'lnxdrive sync' to retry the upload"
  ],
  "history": [
    {
      "timestamp": "2026-02-03T14:25:00Z",
      "action": "FileModified"
    },
    {
      "timestamp": "2026-02-03T14:25:02Z",
      "action": "UploadAttempt",
      "result": "Failed",
      "reason": "FILE_LOCKED"
    }
  ]
}
```

**Exit codes**:
- `0` - Explanation provided
- `1` - Path not found

---

## `audit` - Audit Log Commands

### `audit`

View audit log.

```bash
lnxdrive audit [OPTIONS]
```

**Options**:
- `--since <DURATION>` - Show entries since (e.g., "1 hour ago", "2024-01-01")
- `--action <ACTION>` - Filter by action type
- `--limit <N>` - Limit number of entries (default: 50)
- `--path <PATH>` - Filter by file path

**Output (human)**:
```
Audit Log (last 50 entries)

2026-02-03 14:30:00  SyncComplete    ✓  Session completed (5 files, 12.3s)
2026-02-03 14:29:55  FileUpload      ✓  notes.txt (1.2 KB)
2026-02-03 14:29:50  FileDownload    ✓  document.pdf (2.3 MB)
2026-02-03 14:29:45  FileUpload      ✗  locked-file.xlsx (file in use)
2026-02-03 14:29:00  SyncStart       ✓  Session started
```

**Output (JSON)**:
```json
{
  "entries": [
    {
      "id": 12345,
      "timestamp": "2026-02-03T14:30:00Z",
      "action": "SyncComplete",
      "result": "Success",
      "details": {
        "session_id": "550e8400-e29b-41d4-a716-446655440000",
        "files_processed": 5,
        "duration_ms": 12345
      }
    }
  ],
  "total": 1234,
  "limit": 50,
  "offset": 0
}
```

**Exit codes**:
- `0` - Success
- `1` - Error retrieving log

---

## `config` - Configuration Commands

### `config show`

Show current configuration.

```bash
lnxdrive config show
```

**Output (human)**:
```
Configuration: ~/.config/lnxdrive/config.yaml

sync:
  root: ~/OneDrive
  poll_interval: 30s
  debounce_delay: 2s

rate_limiting:
  delta_requests_per_minute: 10
  upload_concurrent: 4
  ...

(showing 12 of 24 settings, use --all for complete list)
```

---

### `config set <KEY> <VALUE>`

Set a configuration value.

```bash
lnxdrive config set sync.poll_interval 60
```

**Output (human)**:
```
✓ Set sync.poll_interval = 60
  (was: 30)
```

---

### `config validate`

Validate configuration file.

```bash
lnxdrive config validate
```

**Output (human)**:
```
✓ Configuration is valid
```

or

```
✗ Configuration errors:
  - sync.root: Path does not exist
  - rate_limiting.upload_concurrent: Must be between 1 and 10
```

**Exit codes**:
- `0` - Valid
- `1` - Invalid

---

## `daemon` - Daemon Management Commands

### `daemon start`

Start the daemon.

```bash
lnxdrive daemon start
```

**Output (human)**:
```
Starting lnxdrive daemon...
✓ Daemon started (PID: 12345)
```

---

### `daemon stop`

Stop the daemon.

```bash
lnxdrive daemon stop
```

**Output (human)**:
```
Stopping lnxdrive daemon...
✓ Daemon stopped
```

---

### `daemon status`

Check daemon status.

```bash
lnxdrive daemon status
```

**Output (human)**:
```
Daemon Status: Running
PID: 12345
Uptime: 2h 34m
Memory: 45.2 MB
CPU: 0.1%
```

**Output (JSON)**:
```json
{
  "running": true,
  "pid": 12345,
  "uptime_seconds": 9240,
  "memory_bytes": 47395635,
  "cpu_percent": 0.1
}
```

---

### `daemon restart`

Restart the daemon.

```bash
lnxdrive daemon restart
```

---

## `conflicts` - Conflict Commands

### `conflicts`

List unresolved conflicts.

```bash
lnxdrive conflicts
```

**Output (human)**:
```
Conflicts (2 unresolved):

1. ~/OneDrive/report.docx
   Local:  2026-02-03 14:30:00 (234 KB)
   Remote: 2026-02-03 14:25:00 (230 KB)

2. ~/OneDrive/data.xlsx
   Local:  2026-02-03 13:00:00 (1.2 MB)
   Remote: 2026-02-03 14:00:00 (1.3 MB)

Use 'lnxdrive conflicts resolve <ID> --keep-local|--keep-remote|--keep-both'
```

---

### `conflicts resolve <ID>`

Resolve a conflict.

```bash
lnxdrive conflicts resolve <ID> [--keep-local|--keep-remote|--keep-both]
```

---

### `conflicts preview <ID>`

Preview conflict resolution.

```bash
lnxdrive conflicts preview <ID>
```

Shows diff between local and remote versions.

---

## Error Handling

All commands follow consistent error output:

**Human format**:
```
✗ Error: <message>
  Details: <additional context>
  Suggestion: <what to do>
```

**JSON format**:
```json
{
  "success": false,
  "error": {
    "code": "AUTH_REQUIRED",
    "message": "Not authenticated",
    "suggestion": "Run 'lnxdrive auth login' to authenticate"
  }
}
```

## Common Error Codes

| Code | Description |
|------|-------------|
| `AUTH_REQUIRED` | Not authenticated |
| `AUTH_EXPIRED` | Token expired, needs refresh |
| `NETWORK_ERROR` | Network connectivity issue |
| `FILE_NOT_FOUND` | File or path not found |
| `FILE_LOCKED` | File in use by another application |
| `PERMISSION_DENIED` | No permission to access file |
| `DISK_FULL` | No space left on device |
| `RATE_LIMITED` | API rate limit exceeded |
| `CONFLICT` | Sync conflict detected |
| `CONFIG_ERROR` | Configuration file error |
| `DAEMON_ERROR` | Daemon communication error |
