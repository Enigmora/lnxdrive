---
id: AILOG-2026-02-20-002
title: MVP Closure Block 4 — Distribution infrastructure (systemd, autostart, CI)
status: accepted
created: 2026-02-20
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [mvp, systemd, autostart, ci, distribution, dbus-activation]
related: [AILOG-2026-02-20-001, MVP-CLOSURE-PLAN.md]
---

# AILOG: MVP Closure Block 4 — Distribution infrastructure

## Summary

Completed the final block of the MVP Closure Plan: updated systemd service for production paths, created D-Bus activation file for on-demand startup, created XDG autostart desktop entry, and expanded CI to include all 4 previously excluded crates.

## Context

Block 4 addressed distribution infrastructure gaps: the systemd service used a development path (`%h/.cargo/bin/lnxdrived`), there was no autostart mechanism for user login, and CI excluded 4 crates (fuse, conflict, audit, telemetry) from build/test.

## Actions Performed

1. **M2: Updated systemd service** (`config/lnxdrive.service`)
   - Changed `Type=simple` to `Type=dbus` with `BusName=com.enigmora.LNXDrive`
   - Changed `ExecStart=%h/.cargo/bin/lnxdrived` to `ExecStart=/usr/bin/lnxdrive-daemon`
   - Added `PrivateTmp=true` hardening

2. **Created D-Bus activation file** (`config/com.enigmora.LNXDrive.service`)
   - New file for `/usr/share/dbus-1/services/`
   - Includes `SystemdService=lnxdrive.service` to delegate lifecycle to systemd

3. **M3: Created XDG autostart** (`config/lnxdrive-autostart.desktop`)
   - For desktops using XDG autostart (GNOME <=48, KDE, XFCE, etc.)
   - GNOME 49+ covered by systemd `WantedBy=default.target` + D-Bus activation

4. **M5: Expanded CI pipeline** (`.github/workflows/ci.yml`)
   - Removed `--exclude lnxdrive-fuse --exclude lnxdrive-conflict --exclude lnxdrive-audit --exclude lnxdrive-telemetry` from clippy, build, and test steps
   - Added `libfuse3-dev pkg-config` to system dependencies

## Modified Files

| File | Change |
|------|--------|
| `config/lnxdrive.service` | Type=dbus, BusName, ExecStart=/usr/bin/lnxdrive-daemon, PrivateTmp |
| `config/com.enigmora.LNXDrive.service` | NEW — D-Bus activation file |
| `config/lnxdrive-autostart.desktop` | NEW — XDG autostart entry |
| `.github/workflows/ci.yml` | Removed crate exclusions, added libfuse3-dev |

## Decisions Made

- **Type=dbus over Type=simple**: Aligns with the guide specification. systemd considers the service started once the bus name is acquired, providing better lifecycle management.
- **D-Bus activation + systemd**: Using `SystemdService=` prevents D-Bus from spawning duplicate daemon instances and delegates lifecycle to systemd.
- **Both XDG and systemd autostart**: XDG .desktop for broad compatibility; systemd `WantedBy=default.target` for GNOME 49+ where XDG autostart phase is ignored.

## Impact

- **Functionality**: Daemon now has proper production paths, auto-starts on login, and is activatable on-demand via D-Bus.
- **Performance**: N/A
- **Security**: Added `PrivateTmp=true` hardening to systemd service.

## Verification

- [x] Both workspaces compile without errors
- [ ] Systemd service tested in real environment
- [ ] CI pipeline tested with actual GitHub Actions run

## Additional Notes

- The packaging repo (`lnxdrive-packaging`) will reference these config files when building RPM/DEB packages
- Post-install scripts should run `systemctl --user daemon-reload` and optionally `systemctl --user enable lnxdrive.service`

---

<!-- Template: DevTrail | https://enigmora.com -->
