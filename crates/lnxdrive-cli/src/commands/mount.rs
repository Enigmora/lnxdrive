//! Mount command - Mount the LNXDrive Files-on-Demand FUSE filesystem
//!
//! Provides the `lnxdrive mount` and `lnxdrive unmount` CLI commands which:
//! 1. Load configuration and validate prerequisites
//! 2. Open the database pool
//! 3. Verify authenticated account exists
//! 4. Validate mount point (create if needed, check emptiness)
//! 5. Check FUSE availability via `/dev/fuse`
//! 6. Mount the FUSE filesystem and optionally wait for Ctrl+C

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Args;
use tokio::signal;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

// ============================================================================
// T042: MountCommand with clap options
// ============================================================================

/// Mount the LNXDrive Files-on-Demand FUSE filesystem
#[derive(Debug, Args)]
pub struct MountCommand {
    /// Override the default mount point path
    #[arg(long, short = 'p', value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Run in foreground (don't detach, wait for Ctrl+C)
    #[arg(long, short = 'f')]
    pub foreground: bool,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl MountCommand {
    /// Execute the mount command
    ///
    /// Steps:
    /// 1. Load configuration
    /// 2. Open database pool
    /// 3. Validate prerequisites (account, mount point, FUSE)
    /// 4. Mount the FUSE filesystem
    /// 5. If foreground: wait for Ctrl+C signal
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
        use lnxdrive_core::{config::Config, ports::state_repository::IStateRepository};
        use lnxdrive_fuse::{cache::ContentCache, filesystem::LnxDriveFs};

        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        // Step 1: Load configuration
        let config_path = Config::default_path();
        let config = Config::load_or_default(&config_path);

        info!(config_path = %config_path.display(), "Loaded configuration");

        // Step 2: Determine mount point (command flag overrides config)
        let mount_point = self
            .path
            .clone()
            .unwrap_or_else(|| expand_tilde(&config.fuse.mount_point));

        info!(mount_point = %mount_point.display(), "Using mount point");

        // Step 3: Open database pool
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        // Step 4: Validate authenticated account exists
        let account = state_repo
            .get_default_account()
            .await
            .context("Failed to query default account")?;

        let account = match account {
            Some(a) => a,
            None => {
                formatter.error("No account configured. Run 'lnxdrive auth login' first.");
                if use_json {
                    formatter.print_json(&serde_json::json!({
                        "success": false,
                        "error": "no_account",
                        "message": "No account configured. Run 'lnxdrive auth login' first."
                    }));
                }
                return Ok(());
            }
        };

        info!(email = %account.email(), "Found authenticated account");

        // Step 5: Validate mount point exists (create if needed)
        if !mount_point.exists() {
            formatter.info(&format!(
                "Creating mount point directory: {}",
                mount_point.display()
            ));
            tokio::fs::create_dir_all(&mount_point)
                .await
                .context("Failed to create mount point directory")?;
        }

        // Step 6: Check mount point is empty or has only hidden files
        if !is_mount_point_suitable(&mount_point).await? {
            formatter.error(&format!(
                "Mount point '{}' is not empty. Please use an empty directory.",
                mount_point.display()
            ));
            if use_json {
                formatter.print_json(&serde_json::json!({
                    "success": false,
                    "error": "mount_point_not_empty",
                    "mount_point": mount_point.display().to_string()
                }));
            }
            return Ok(());
        }

        // Step 7: Check FUSE availability
        if !Path::new("/dev/fuse").exists() {
            formatter.error("FUSE is not available. /dev/fuse does not exist.");
            formatter.info(
                "Hint: Install FUSE with 'sudo apt install fuse3' or 'sudo dnf install fuse3'",
            );
            formatter
                .info("Hint: Make sure the FUSE kernel module is loaded: 'sudo modprobe fuse'");
            if use_json {
                formatter.print_json(&serde_json::json!({
                    "success": false,
                    "error": "fuse_not_available",
                    "message": "FUSE is not available. /dev/fuse does not exist."
                }));
            }
            return Ok(());
        }

        info!("FUSE is available at /dev/fuse");

        // Step 8: Set up cache directory
        let cache_dir = expand_tilde(&config.fuse.cache_dir);
        if !cache_dir.exists() {
            formatter.info(&format!(
                "Creating cache directory: {}",
                cache_dir.display()
            ));
            tokio::fs::create_dir_all(&cache_dir)
                .await
                .context("Failed to create cache directory")?;
        }

        let cache = Arc::new(
            ContentCache::new(cache_dir.clone()).context("Failed to initialize content cache")?,
        );

        // Step 9: Create the FUSE filesystem
        let rt_handle = tokio::runtime::Handle::current();
        let fs = LnxDriveFs::new(rt_handle.clone(), pool.clone(), config.fuse.clone(), cache);

        // Step 10: Mount the filesystem using fuser::spawn_mount2
        formatter.info(&format!("Mounting filesystem at {}", mount_point.display()));

        let mount_options = vec![
            fuser::MountOption::FSName("lnxdrive".to_string()),
            fuser::MountOption::AutoUnmount,
            fuser::MountOption::AllowOther,
        ];

        let session = fuser::spawn_mount2(fs, &mount_point, &mount_options)
            .context("Failed to mount FUSE filesystem")?;

        // Step 11: Report success
        formatter.success(&format!("LNXDrive mounted at {}", mount_point.display()));

        if use_json {
            formatter.print_json(&serde_json::json!({
                "success": true,
                "mount_point": mount_point.display().to_string(),
                "cache_dir": cache_dir.display().to_string(),
                "account": account.email(),
                "foreground": self.foreground
            }));
        }

        // Step 12: Handle foreground mode
        if self.foreground {
            formatter.info("Running in foreground mode. Press Ctrl+C to unmount and exit.");

            // Wait for Ctrl+C signal
            signal::ctrl_c()
                .await
                .context("Failed to listen for Ctrl+C signal")?;

            formatter.info("Received Ctrl+C, unmounting...");

            // Join the session to trigger unmount
            session.join();

            formatter.success("Filesystem unmounted successfully");
        } else {
            formatter.info("Filesystem mounted in background.");
            formatter.info(&format!(
                "To unmount, run: lnxdrive unmount --path {}",
                mount_point.display()
            ));

            // Drop session handle without joining - filesystem continues in background
            // Note: The session handle being dropped without join() means the mount
            // will stay active as long as the spawned thread is running
            std::mem::forget(session);
        }

        Ok(())
    }
}

// ============================================================================
// T042: UnmountCommand with clap options
// ============================================================================

/// Unmount the LNXDrive FUSE filesystem
#[derive(Debug, Args)]
pub struct UnmountCommand {
    /// Force unmount even if the filesystem is busy
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Override the default mount point path
    #[arg(long, short = 'p', value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl UnmountCommand {
    /// Execute the unmount command
    ///
    /// Uses `fusermount -u <path>` (or `fusermount3 -u <path>`) to unmount.
    /// With --force, uses `fusermount -uz` for lazy unmount.
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::config::Config;

        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        // Load configuration to get default mount point
        let config_path = Config::default_path();
        let config = Config::load_or_default(&config_path);

        // Determine mount point (command flag overrides config)
        let mount_point = self
            .path
            .clone()
            .unwrap_or_else(|| expand_tilde(&config.fuse.mount_point));

        info!(mount_point = %mount_point.display(), "Unmounting filesystem");

        // Check if mount point exists
        if !mount_point.exists() {
            formatter.error(&format!(
                "Mount point '{}' does not exist",
                mount_point.display()
            ));
            if use_json {
                formatter.print_json(&serde_json::json!({
                    "success": false,
                    "error": "mount_point_not_found",
                    "mount_point": mount_point.display().to_string()
                }));
            }
            return Ok(());
        }

        // Build fusermount command
        // Try fusermount3 first (FUSE 3), fall back to fusermount (FUSE 2)
        let fusermount = if which_exists("fusermount3") {
            "fusermount3"
        } else {
            "fusermount"
        };

        let mut args = vec!["-u"];
        if self.force {
            // -z for lazy unmount (unmount even if busy)
            args.push("-z");
        }

        formatter.info(&format!(
            "Executing: {} {} {}",
            fusermount,
            args.join(" "),
            mount_point.display()
        ));

        let output = Command::new(fusermount)
            .args(&args)
            .arg(&mount_point)
            .output()
            .context("Failed to execute fusermount. Is FUSE installed?")?;

        if output.status.success() {
            formatter.success(&format!(
                "Filesystem unmounted from {}",
                mount_point.display()
            ));
            if use_json {
                formatter.print_json(&serde_json::json!({
                    "success": true,
                    "mount_point": mount_point.display().to_string(),
                    "force": self.force
                }));
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = stderr.trim();

            // Check for common error conditions
            if error_msg.contains("not mounted") || error_msg.contains("no such file") {
                formatter.error(&format!(
                    "Filesystem is not mounted at {}",
                    mount_point.display()
                ));
            } else if error_msg.contains("Device or resource busy") {
                formatter.error(&format!(
                    "Filesystem is busy. Close any programs using files in {} and try again.",
                    mount_point.display()
                ));
                formatter.info("Hint: Use --force to perform a lazy unmount");
            } else {
                formatter.error(&format!("Failed to unmount: {}", error_msg));
            }

            if use_json {
                formatter.print_json(&serde_json::json!({
                    "success": false,
                    "error": "unmount_failed",
                    "mount_point": mount_point.display().to_string(),
                    "stderr": error_msg
                }));
            }
        }

        Ok(())
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Expand tilde (~) in a path string to the user's home directory
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Check if a mount point is suitable (empty or contains only hidden files)
async fn is_mount_point_suitable(path: &Path) -> Result<bool> {
    let mut entries = tokio::fs::read_dir(path)
        .await
        .context("Failed to read mount point directory")?;

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Allow hidden files (starting with .)
        if !name.starts_with('.') {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Check if a command exists in PATH
fn which_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_with_home_path() {
        let expanded = expand_tilde("~/OneDrive");
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home.join("OneDrive"));
        }
    }

    #[test]
    fn test_expand_tilde_with_absolute_path() {
        let expanded = expand_tilde("/tmp/mount");
        assert_eq!(expanded, PathBuf::from("/tmp/mount"));
    }

    #[test]
    fn test_expand_tilde_with_relative_path() {
        let expanded = expand_tilde("relative/path");
        assert_eq!(expanded, PathBuf::from("relative/path"));
    }

    #[test]
    fn test_expand_tilde_only() {
        let expanded = expand_tilde("~");
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home);
        }
    }

    #[tokio::test]
    async fn test_is_mount_point_suitable_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = is_mount_point_suitable(temp_dir.path()).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_is_mount_point_suitable_with_hidden_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(temp_dir.path().join(".hidden"), "test")
            .await
            .unwrap();
        let result = is_mount_point_suitable(temp_dir.path()).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_is_mount_point_suitable_with_regular_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        tokio::fs::write(temp_dir.path().join("regular_file"), "test")
            .await
            .unwrap();
        let result = is_mount_point_suitable(temp_dir.path()).await;
        assert!(!result.unwrap());
    }

    #[test]
    fn test_mount_command_default() {
        let cmd = MountCommand {
            path: None,
            foreground: false,
            json: false,
        };
        assert!(!cmd.foreground);
        assert!(cmd.path.is_none());
    }

    #[test]
    fn test_unmount_command_default() {
        let cmd = UnmountCommand {
            force: false,
            path: None,
            json: false,
        };
        assert!(!cmd.force);
        assert!(cmd.path.is_none());
    }
}
