//! Daemon management commands
//!
//! Provides the `lnxdrive daemon` CLI subcommands for controlling the
//! LNXDrive background synchronization service via systemd user units.
//!
//! # Subcommands
//!
//! - `start`   - Start the daemon service
//! - `stop`    - Stop the daemon service
//! - `status`  - Show daemon status
//! - `restart` - Restart the daemon service

use std::process::Command;

use anyhow::{Context, Result};
use clap::Subcommand;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

/// Service unit name for the LNXDrive daemon
const SYSTEMD_UNIT: &str = "lnxdrive";

// ============================================================================
// T226: DaemonCommand with subcommands
// ============================================================================

/// Manage the LNXDrive background daemon
#[derive(Debug, Subcommand)]
pub enum DaemonCommand {
    /// Start the LNXDrive daemon
    Start,
    /// Stop the LNXDrive daemon
    Stop,
    /// Show daemon status
    Status,
    /// Restart the LNXDrive daemon
    Restart,
}

impl DaemonCommand {
    /// Execute the selected daemon subcommand
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        match self {
            DaemonCommand::Start => daemon_start(format),
            DaemonCommand::Stop => daemon_stop(format),
            DaemonCommand::Status => daemon_status(format),
            DaemonCommand::Restart => daemon_restart(format),
        }
    }
}

// ============================================================================
// T227: daemon start
// ============================================================================

/// Starts the LNXDrive daemon via systemctl
///
/// Runs `systemctl --user start lnxdrive` and reports the result.
fn daemon_start(format: OutputFormat) -> Result<()> {
    let formatter = get_formatter(matches!(format, OutputFormat::Json));

    info!("Starting LNXDrive daemon via systemctl");

    let output = Command::new("systemctl")
        .args(["--user", "start", SYSTEMD_UNIT])
        .output()
        .context("Failed to execute systemctl. Is systemd available?")?;

    if output.status.success() {
        formatter.success("LNXDrive daemon started");
        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "start",
                "success": true,
            }));
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!("Failed to start daemon: {}", stderr.trim());
        formatter.error(&msg);

        if stderr.contains("not found") || stderr.contains("No such file") {
            formatter.info("Hint: The systemd unit file may not be installed.");
            formatter
                .info("Copy config/lnxdrive.service to ~/.config/systemd/user/lnxdrive.service");
            formatter.info("Then run: systemctl --user daemon-reload");
        }

        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "start",
                "success": false,
                "error": stderr.trim(),
            }));
        }
    }

    Ok(())
}

// ============================================================================
// T228: daemon stop
// ============================================================================

/// Stops the LNXDrive daemon via systemctl
///
/// Runs `systemctl --user stop lnxdrive` and reports the result.
fn daemon_stop(format: OutputFormat) -> Result<()> {
    let formatter = get_formatter(matches!(format, OutputFormat::Json));

    info!("Stopping LNXDrive daemon via systemctl");

    let output = Command::new("systemctl")
        .args(["--user", "stop", SYSTEMD_UNIT])
        .output()
        .context("Failed to execute systemctl. Is systemd available?")?;

    if output.status.success() {
        formatter.success("LNXDrive daemon stopped");
        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "stop",
                "success": true,
            }));
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        formatter.error(&format!("Failed to stop daemon: {}", stderr.trim()));

        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "stop",
                "success": false,
                "error": stderr.trim(),
            }));
        }
    }

    Ok(())
}

// ============================================================================
// T229: daemon status
// ============================================================================

/// Shows the LNXDrive daemon status
///
/// Runs `systemctl --user status lnxdrive` and displays the output.
fn daemon_status(format: OutputFormat) -> Result<()> {
    let formatter = get_formatter(matches!(format, OutputFormat::Json));

    info!("Querying LNXDrive daemon status via systemctl");

    let output = Command::new("systemctl")
        .args(["--user", "status", SYSTEMD_UNIT])
        .output()
        .context("Failed to execute systemctl. Is systemd available?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse some basic info from systemctl output
    let is_active = stdout.contains("active (running)");
    let is_inactive = stdout.contains("inactive (dead)");
    let is_failed = stdout.contains("failed");

    let status_str = if is_active {
        "running"
    } else if is_failed {
        "failed"
    } else if is_inactive {
        "stopped"
    } else {
        "unknown"
    };

    if matches!(format, OutputFormat::Json) {
        formatter.print_json(&serde_json::json!({
            "action": "status",
            "status": status_str,
            "active": is_active,
            "details": stdout.trim(),
        }));
        return Ok(());
    }

    // Human-readable output
    if is_active {
        formatter.success("LNXDrive daemon is running");
    } else if is_failed {
        formatter.error("LNXDrive daemon has failed");
    } else if is_inactive {
        formatter.info("LNXDrive daemon is stopped");
    } else {
        formatter.info("LNXDrive daemon status is unknown");
    }

    // Show full systemctl output for details
    if !stdout.is_empty() {
        formatter.info("");
        for line in stdout.lines() {
            formatter.info(line);
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() && !output.status.success() {
        // systemctl status exits with non-zero for inactive services, so
        // only show stderr if it contains meaningful error text
        if stderr.contains("not found") || stderr.contains("No such file") {
            formatter.info("");
            formatter.info("Hint: The systemd unit file may not be installed.");
            formatter
                .info("Copy config/lnxdrive.service to ~/.config/systemd/user/lnxdrive.service");
            formatter.info("Then run: systemctl --user daemon-reload");
        }
    }

    Ok(())
}

// ============================================================================
// T230: daemon restart
// ============================================================================

/// Restarts the LNXDrive daemon via systemctl
///
/// Runs `systemctl --user restart lnxdrive` and reports the result.
fn daemon_restart(format: OutputFormat) -> Result<()> {
    let formatter = get_formatter(matches!(format, OutputFormat::Json));

    info!("Restarting LNXDrive daemon via systemctl");

    let output = Command::new("systemctl")
        .args(["--user", "restart", SYSTEMD_UNIT])
        .output()
        .context("Failed to execute systemctl. Is systemd available?")?;

    if output.status.success() {
        formatter.success("LNXDrive daemon restarted");
        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "restart",
                "success": true,
            }));
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        formatter.error(&format!("Failed to restart daemon: {}", stderr.trim()));

        if matches!(format, OutputFormat::Json) {
            formatter.print_json(&serde_json::json!({
                "action": "restart",
                "success": false,
                "error": stderr.trim(),
            }));
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_systemd_unit_name() {
        assert_eq!(SYSTEMD_UNIT, "lnxdrive");
    }

    #[test]
    fn test_daemon_command_variants() {
        // Verify all variants exist and can be constructed
        let _start = DaemonCommand::Start;
        let _stop = DaemonCommand::Stop;
        let _status = DaemonCommand::Status;
        let _restart = DaemonCommand::Restart;
    }

    #[test]
    fn test_daemon_command_debug() {
        let cmd = DaemonCommand::Start;
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("Start"));
    }
}
