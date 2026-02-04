# Functional Testing with Containers

This document describes how to run LNXDrive daemon functional tests using Podman containers with systemd.

## Prerequisites

- **Podman** installed and working (rootless mode)
- **Rust toolchain** (1.75+) for building the project
- **cgroups v2** enabled (default on Fedora 39+, Ubuntu 22.04+)

Verify Podman is available:

```bash
podman --version
podman info --format '{{.Host.CgroupsVersion}}'  # Should print "v2"
```

## Quick Start

```bash
# 1. Build release binaries
make build

# 2. Build the systemd container image
make container-build-systemd

# 3. Run functional tests
make container-test-daemon
```

## What Gets Tested

The functional test suite (`scripts/test-daemon-functional.sh`) runs 8 checks inside the container:

| # | Check | Description |
|---|-------|-------------|
| 1 | Service start | `systemctl --user start lnxdrive` succeeds |
| 2 | Active status | Service reports "active" |
| 3 | Process running | `lnxdrived` process exists |
| 4 | D-Bus registered | `com.enigmora.LNXDrive` name is on the session bus |
| 5 | CLI responds | `lnxdrive daemon status` returns exit code 0 |
| 6 | Database created | `~/.local/share/lnxdrive/lnxdrive.db` exists |
| 7 | Journal logs | Daemon log entries appear in systemd journal |
| 8 | Graceful stop | `systemctl --user stop lnxdrive` terminates the process cleanly |

The daemon starts without OAuth credentials and enters `WaitingForAuth` state. This is expected behavior for these tests.

## Output Format

```
LNXDrive Daemon Functional Tests
=================================

  [INFO] User: testuser
  [INFO] XDG_RUNTIME_DIR: /run/user/1000

Check 1/8: Start service
  [PASS] Service started successfully
Check 2/8: Service is active
  [PASS] Service status is 'active'
...

=========================================
Results: 8/8 checks passed
=========================================
All checks passed.
```

## Interactive Debugging

To get a shell inside the container for manual inspection:

```bash
make container-shell
```

This starts the container (if not already running) and opens an interactive bash session as `testuser`. Inside the container you can:

```bash
# Check service status
systemctl --user status lnxdrive

# View logs in real time
journalctl --user -u lnxdrive -f

# Inspect D-Bus
busctl --user list | grep enigmora
busctl --user introspect com.enigmora.LNXDrive /com/enigmora/LNXDrive

# Run the CLI
lnxdrive daemon status
lnxdrive status

# Check the database
sqlite3 ~/.local/share/lnxdrive/lnxdrive.db ".tables"

# Restart the service
systemctl --user restart lnxdrive

# Exit the container
exit
```

After exiting, stop the container:

```bash
make container-stop
```

## Cleanup

Remove the container and image:

```bash
make container-clean
```

## Troubleshooting

### "systemd --user" not starting

This usually means lingering is not enabled. The Containerfile handles this, but if you see issues:

```bash
# Inside the container as root
loginctl enable-linger testuser
```

### D-Bus session bus not available

Verify `XDG_RUNTIME_DIR` is set and the bus socket exists:

```bash
ls -la $XDG_RUNTIME_DIR/bus
```

If the socket is missing, the user session may not have fully initialized. Increase the sleep time in the Makefile's `container-test-daemon` target.

### Container fails to start with cgroups error

Ensure cgroups v2 is enabled on the host:

```bash
mount | grep cgroup2
# Should show: cgroup2 on /sys/fs/cgroup type cgroup2
```

On older distributions, you may need to add `systemd.unified_cgroup_hierarchy=1` to kernel boot parameters.

### Permission denied errors

Podman rootless requires user namespaces. Check:

```bash
sysctl user.max_user_namespaces
# Should be > 0 (typically 28633 or higher)
```

## Make Targets Reference

| Target | Description |
|--------|-------------|
| `make build` | Build release binaries (excludes stub crates) |
| `make test-unit` | Run workspace unit tests (excludes stub crates) |
| `make container-build-systemd` | Build Podman image with systemd |
| `make container-test-daemon` | Run functional tests in container |
| `make container-shell` | Open interactive shell in container |
| `make container-stop` | Stop and remove the test container |
| `make container-clean` | Remove container and image |
