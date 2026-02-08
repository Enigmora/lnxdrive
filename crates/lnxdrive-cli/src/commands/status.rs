//! Status command - Display synchronization status
//!
//! Provides the `lnxdrive status` CLI command which:
//! 1. Shows global sync status (item counts by state, last sync time)
//! 2. Shows per-file status when a path is given
//! 3. Lists pending (Modified/Hydrating) items
//! 4. Lists items in Error state with error details
//! 5. Shows FUSE filesystem status (mount state, cache usage, file counts)

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Args;
use lnxdrive_core::config::Config;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

/// T189: Status command with optional path argument
#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Optional path to check status of a specific file
    pub path: Option<String>,
}

impl StatusCommand {
    /// T190-T193: Execute the status command
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
        use lnxdrive_core::ports::state_repository::IStateRepository;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        // Open database
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        if !db_path.exists() {
            formatter
                .error("No database found. Run 'lnxdrive auth login' and 'lnxdrive sync' first.");
            return Ok(());
        }

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        // Get default account
        let account = state_repo
            .get_default_account()
            .await
            .context("Failed to query default account")?;

        let account = match account {
            Some(a) => a,
            None => {
                formatter.error("No account configured. Run 'lnxdrive auth login' first.");
                return Ok(());
            }
        };

        if let Some(ref path_str) = self.path {
            // T191: Per-file status
            self.show_file_status(&*state_repo, path_str, &format, &*formatter)
                .await
        } else {
            // T190, T192, T193: Global status
            self.show_global_status(&*state_repo, &account, &format, &*formatter)
                .await
        }
    }

    /// T190: Display global synchronization status
    /// T094: Extended with FUSE status section
    async fn show_global_status(
        &self,
        state_repo: &dyn lnxdrive_core::ports::IStateRepository,
        account: &lnxdrive_core::domain::Account,
        format: &OutputFormat,
        formatter: &dyn crate::output::OutputFormatter,
    ) -> Result<()> {
        use lnxdrive_core::{domain::sync_item::ItemState, ports::state_repository::ItemFilter};

        info!(email = %account.email(), "Showing status for account");

        // Query counts by state
        let counts = state_repo
            .count_items_by_state(account.id())
            .await
            .context("Failed to count items by state")?;

        let total: u64 = counts.values().sum();

        // T094: Get FUSE status
        let fuse_status = get_fuse_status(&counts);

        if matches!(format, OutputFormat::Json) {
            let last_sync_str = account
                .last_sync()
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "never".to_string());

            let json = serde_json::json!({
                "account": account.email().as_str(),
                "last_sync": last_sync_str,
                "total_items": total,
                "items_by_state": counts,
                "fuse": fuse_status.to_json(),
            });
            formatter.print_json(&json);
            return Ok(());
        }

        // Human-readable output
        formatter.success(&format!("LNXDrive Status - {}", account.email()));
        formatter.info("");

        // Last sync time
        match account.last_sync() {
            Some(time) => {
                formatter.info(&format!(
                    "Last sync: {}",
                    time.format("%Y-%m-%d %H:%M:%S UTC")
                ));
            }
            None => {
                formatter.info("Last sync: Never");
            }
        }

        formatter.info(&format!("Total items: {}", total));
        formatter.info("");

        // State counts table
        let state_order = [
            "Online",
            "Hydrating",
            "Hydrated",
            "Modified",
            "Conflicted",
            "Error",
            "Deleted",
        ];
        formatter.info("State         Count");
        formatter.info("------------- -----");
        for state_name in &state_order {
            let count = counts.get(*state_name).copied().unwrap_or(0);
            if count > 0 {
                formatter.info(&format!("{:<13} {}", state_name, count));
            }
        }

        // T192: Show pending items (Modified/Hydrating)
        let modified_items = state_repo
            .query_items(&ItemFilter::new().with_state(ItemState::Modified))
            .await
            .context("Failed to query modified items")?;

        let hydrating_items = state_repo
            .query_items(&ItemFilter::new().with_state(ItemState::Hydrating))
            .await
            .context("Failed to query hydrating items")?;

        if !modified_items.is_empty() || !hydrating_items.is_empty() {
            formatter.info("");
            formatter.info("Pending items:");

            for item in &hydrating_items {
                let path_str = truncate_path(item.local_path().to_string(), 60);
                formatter.info(&format!("  [Hydrating] {}", path_str));
            }

            for item in &modified_items {
                let path_str = truncate_path(item.local_path().to_string(), 60);
                formatter.info(&format!("  [Modified]  {}", path_str));
            }
        }

        // T193: Show error items
        let error_items = state_repo
            .query_items(&ItemFilter::new().with_state(ItemState::Error(String::new())))
            .await
            .context("Failed to query error items")?;

        if !error_items.is_empty() {
            formatter.info("");
            formatter.error(&format!("{} file(s) with errors:", error_items.len()));

            for item in &error_items {
                let path_str = truncate_path(item.local_path().to_string(), 50);
                let reason = match item.error_info() {
                    Some(err) => format!("[{}] {}", err.code(), err.message()),
                    None => match item.state() {
                        ItemState::Error(reason) => reason.clone(),
                        _ => "Unknown error".to_string(),
                    },
                };
                formatter.info(&format!("  {} - {}", path_str, reason));
            }
        }

        // T094: Show FUSE status
        formatter.info("");
        formatter.info("FUSE:");
        fuse_status.display_human(formatter);

        Ok(())
    }

    /// T191: Display status for a specific file
    async fn show_file_status(
        &self,
        state_repo: &dyn lnxdrive_core::ports::IStateRepository,
        path_str: &str,
        format: &OutputFormat,
        formatter: &dyn crate::output::OutputFormatter,
    ) -> Result<()> {
        use lnxdrive_core::domain::newtypes::SyncPath;

        // Resolve to absolute path
        let abs_path = if PathBuf::from(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(path_str)
        };

        let sync_path = SyncPath::new(abs_path.clone()).context("Invalid path")?;

        let item = state_repo
            .get_item_by_path(&sync_path)
            .await
            .context("Failed to query item by path")?;

        match item {
            Some(item) => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "path": item.local_path().to_string(),
                        "remote_path": item.remote_path().to_string(),
                        "remote_id": item.remote_id().map(|r| r.to_string()),
                        "state": item.state().to_string(),
                        "size_bytes": item.size_bytes(),
                        "content_hash": item.content_hash().map(|h| h.to_string()),
                        "local_hash": item.local_hash().map(|h| h.to_string()),
                        "hashes_match": item.hashes_match(),
                        "last_modified_local": item.last_modified_local().map(|t| t.to_rfc3339()),
                        "last_modified_remote": item.last_modified_remote().map(|t| t.to_rfc3339()),
                        "last_sync": item.last_sync().map(|t| t.to_rfc3339()),
                        "error_info": item.error_info().map(|e| e.to_string()),
                    });
                    formatter.print_json(&json);
                    return Ok(());
                }

                formatter.success(&format!("File status: {}", item.local_path()));
                formatter.info("");
                formatter.info(&format!("State:          {}", item.state()));
                formatter.info(&format!("Local path:     {}", item.local_path()));
                formatter.info(&format!("Remote path:    {}", item.remote_path()));
                formatter.info(&format!(
                    "Remote ID:      {}",
                    item.remote_id()
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "(not assigned)".to_string())
                ));
                formatter.info(&format!("Size:           {} bytes", item.size_bytes()));
                formatter.info(&format!(
                    "Content hash:   {}",
                    item.content_hash()
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| "(none)".to_string())
                ));
                formatter.info("");

                // Timestamps
                formatter.info(&format!(
                    "Local modified:  {}",
                    item.last_modified_local()
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "(unknown)".to_string())
                ));
                formatter.info(&format!(
                    "Remote modified: {}",
                    item.last_modified_remote()
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "(unknown)".to_string())
                ));
                formatter.info(&format!(
                    "Last sync:       {}",
                    item.last_sync()
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "(never)".to_string())
                ));
                formatter.info("");

                // Hash match status
                let hash_status = if item.hashes_match() {
                    "Hashes match (file is in sync)"
                } else if item.content_hash().is_some() && item.local_hash().is_some() {
                    "Hashes DO NOT match (file has diverged)"
                } else {
                    "Hash comparison not available (one or both hashes missing)"
                };
                formatter.info(&format!("Hash status:    {}", hash_status));

                // Error info
                if let Some(error) = item.error_info() {
                    formatter.info("");
                    formatter.error(&format!("Error: {}", error));
                }
            }
            None => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "path": abs_path.display().to_string(),
                        "state": "not_tracked",
                        "message": "File is not tracked by LNXDrive",
                    });
                    formatter.print_json(&json);
                    return Ok(());
                }

                formatter.info(&format!(
                    "File '{}' is not tracked by LNXDrive.",
                    abs_path.display()
                ));
                formatter.info("It may be outside the sync folder or excluded by sync rules.");
            }
        }

        Ok(())
    }
}

/// Truncate a path string to a maximum length, showing the end of the path
fn truncate_path(path: String, max_len: usize) -> String {
    if path.len() <= max_len {
        path
    } else {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    }
}

// ============================================================================
// T094: FUSE Status Section
// ============================================================================

/// FUSE filesystem status information.
struct FuseStatus {
    mounted: bool,
    mount_point: String,
    cache_used_bytes: u64,
    cache_max_bytes: u64,
    files_hydrated: u64,
    files_pinned: u64,
    files_online: u64,
    files_hydrating: u64,
}

impl FuseStatus {
    /// Display FUSE status in human-readable format.
    fn display_human(&self, formatter: &dyn crate::output::OutputFormatter) {
        // Mount status
        let mount_status = if self.mounted { "mounted" } else { "not mounted" };
        formatter.info(&format!("  Mount: {} ({})", self.mount_point, mount_status));

        // Cache usage
        let cache_percent = if self.cache_max_bytes > 0 {
            (self.cache_used_bytes as f64 / self.cache_max_bytes as f64 * 100.0) as u8
        } else {
            0
        };
        formatter.info(&format!(
            "  Cache: {} / {} ({}%)",
            format_bytes(self.cache_used_bytes),
            format_bytes(self.cache_max_bytes),
            cache_percent
        ));

        // File counts
        formatter.info(&format!(
            "  Files: {} hydrated, {} pinned, {} online-only",
            self.files_hydrated, self.files_pinned, self.files_online
        ));

        // Hydrating count (only show if > 0)
        if self.files_hydrating > 0 {
            formatter.info(&format!(
                "  Hydrating: {} file(s) in progress",
                self.files_hydrating
            ));
        }
    }

    /// Convert to JSON value.
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "mounted": self.mounted,
            "mount_point": self.mount_point,
            "cache_used_bytes": self.cache_used_bytes,
            "cache_max_bytes": self.cache_max_bytes,
            "files_hydrated": self.files_hydrated,
            "files_pinned": self.files_pinned,
            "files_online": self.files_online,
            "files_hydrating": self.files_hydrating
        })
    }
}

/// Get FUSE status from configuration and state counts.
fn get_fuse_status(counts: &std::collections::HashMap<String, u64>) -> FuseStatus {
    // Load configuration
    let config = Config::load_or_default(&Config::default_path());
    let fuse_config = &config.fuse;

    // Check if mounted by examining /proc/mounts
    let mounted = is_fuse_mounted(&fuse_config.mount_point);

    // Calculate cache usage
    let cache_dir = expand_tilde(&fuse_config.cache_dir);
    let cache_used_bytes = calculate_directory_size(&cache_dir);
    let cache_max_bytes = u64::from(fuse_config.cache_max_size_gb) * 1024 * 1024 * 1024;

    // Extract counts by state
    let files_hydrated = counts.get("Hydrated").copied().unwrap_or(0);
    let files_pinned = counts.get("Pinned").copied().unwrap_or(0);
    let files_online = counts.get("Online").copied().unwrap_or(0);
    let files_hydrating = counts.get("Hydrating").copied().unwrap_or(0);

    FuseStatus {
        mounted,
        mount_point: fuse_config.mount_point.clone(),
        cache_used_bytes,
        cache_max_bytes,
        files_hydrated,
        files_pinned,
        files_online,
        files_hydrating,
    }
}

/// Check if a path is a FUSE mount point by reading /proc/mounts.
fn is_fuse_mounted(mount_point: &str) -> bool {
    let expanded = expand_tilde(mount_point);

    // Try to read /proc/mounts
    if let Ok(content) = fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let mount_path = parts[1];
                let fs_type = parts[2];
                // Check if it's our mount point and is a FUSE filesystem
                if mount_path == expanded && fs_type.starts_with("fuse") {
                    return true;
                }
            }
        }
    }

    false
}

/// Calculate the total size of files in a directory recursively.
fn calculate_directory_size(path: &str) -> u64 {
    let dir_path = Path::new(path);
    if !dir_path.exists() {
        return 0;
    }

    fn recurse(dir: &Path) -> u64 {
        let mut size = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = fs::metadata(&path) {
                        size += metadata.len();
                    }
                } else if path.is_dir() {
                    size += recurse(&path);
                }
            }
        }
        size
    }

    recurse(dir_path)
}

/// Expand ~ to home directory.
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}

/// Format bytes as a human-readable string (e.g., "2.1 GB").
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
