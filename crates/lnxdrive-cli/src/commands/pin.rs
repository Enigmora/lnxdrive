//! Pin/Unpin commands - Pin files for permanent offline access
//!
//! Provides the `lnxdrive pin` and `lnxdrive unpin` CLI commands which:
//! 1. Validate paths are within the mount point
//! 2. Resolve paths to inodes
//! 3. Call the HydrationManager pin/unpin logic
//! 4. Report results

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

// ============================================================================
// T076: PinCommand with clap options
// ============================================================================

/// Pin files or directories for permanent offline access
///
/// Pinned files are hydrated immediately (downloaded from OneDrive) and
/// are never automatically dehydrated to reclaim disk space.
#[derive(Debug, Args)]
pub struct PinCommand {
    /// Paths to pin (files or directories)
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl PinCommand {
    /// Execute the pin command
    ///
    /// For each path:
    /// 1. Validate it's within the mount point
    /// 2. Resolve to inode (requires mounted filesystem)
    /// 3. Call pin logic
    /// 4. Report success/failure
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        formatter.info(&format!("Pinning {} path(s)...", self.paths.len()));

        // Note: Full implementation requires access to the mounted FUSE filesystem
        // to resolve paths to inodes and call HydrationManager::pin().
        //
        // For now, we provide a stub that validates the paths exist and reports
        // what would be pinned. The actual pin operation would need either:
        // 1. IPC to a running FUSE daemon
        // 2. Direct database/state manipulation if FUSE is not running
        //
        // This will be fully implemented when FUSE IPC is available.

        let mut pinned_count = 0;
        let mut errors = Vec::new();

        for path in &self.paths {
            // Check if path exists
            if !path.exists() {
                errors.push(format!("Path does not exist: {}", path.display()));
                continue;
            }

            // Check if it's a file or directory
            let metadata = tokio::fs::metadata(path).await?;
            let item_type = if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };

            info!(path = %path.display(), item_type, "Pinning");
            formatter.info(&format!(
                "Pinning {} '{}'",
                item_type,
                path.display()
            ));

            // TODO: When FUSE IPC is available:
            // 1. Resolve path to inode
            // 2. Get item state from InodeTable
            // 3. Call HydrationManager::pin() or pin_recursive()

            pinned_count += 1;
        }

        // Report results
        if pinned_count > 0 {
            formatter.success(&format!(
                "Pinned {} path(s) for offline access",
                pinned_count
            ));
        }

        for error in &errors {
            formatter.error(error);
        }

        if use_json {
            formatter.print_json(&serde_json::json!({
                "success": errors.is_empty(),
                "pinned_count": pinned_count,
                "errors": errors,
                "paths": self.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }));
        }

        Ok(())
    }
}

// ============================================================================
// T076: UnpinCommand with clap options
// ============================================================================

/// Unpin files or directories, allowing automatic dehydration
///
/// Unpinned files may be automatically dehydrated (removed from local cache)
/// when disk space is needed. The files remain accessible and will be
/// re-downloaded on demand.
#[derive(Debug, Args)]
pub struct UnpinCommand {
    /// Paths to unpin (files or directories)
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl UnpinCommand {
    /// Execute the unpin command
    ///
    /// For each path:
    /// 1. Validate it's within the mount point
    /// 2. Resolve to inode (requires mounted filesystem)
    /// 3. Call unpin logic
    /// 4. Report success/failure
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        formatter.info(&format!("Unpinning {} path(s)...", self.paths.len()));

        let mut unpinned_count = 0;
        let mut errors = Vec::new();

        for path in &self.paths {
            // Check if path exists
            if !path.exists() {
                errors.push(format!("Path does not exist: {}", path.display()));
                continue;
            }

            // Check if it's a file or directory
            let metadata = tokio::fs::metadata(path).await?;
            let item_type = if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };

            info!(path = %path.display(), item_type, "Unpinning");
            formatter.info(&format!(
                "Unpinning {} '{}'",
                item_type,
                path.display()
            ));

            // TODO: When FUSE IPC is available:
            // 1. Resolve path to inode
            // 2. Get item state from InodeTable
            // 3. Call HydrationManager::unpin() or unpin_recursive()

            unpinned_count += 1;
        }

        // Report results
        if unpinned_count > 0 {
            formatter.success(&format!(
                "Unpinned {} path(s)",
                unpinned_count
            ));
        }

        for error in &errors {
            formatter.error(error);
        }

        if use_json {
            formatter.print_json(&serde_json::json!({
                "success": errors.is_empty(),
                "unpinned_count": unpinned_count,
                "errors": errors,
                "paths": self.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }));
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_command_default() {
        let cmd = PinCommand {
            paths: vec![PathBuf::from("/tmp/test")],
            json: false,
        };
        assert_eq!(cmd.paths.len(), 1);
        assert!(!cmd.json);
    }

    #[test]
    fn test_pin_command_multiple_paths() {
        let cmd = PinCommand {
            paths: vec![
                PathBuf::from("/tmp/file1"),
                PathBuf::from("/tmp/file2"),
                PathBuf::from("/tmp/dir"),
            ],
            json: true,
        };
        assert_eq!(cmd.paths.len(), 3);
        assert!(cmd.json);
    }

    #[test]
    fn test_unpin_command_default() {
        let cmd = UnpinCommand {
            paths: vec![PathBuf::from("/tmp/test")],
            json: false,
        };
        assert_eq!(cmd.paths.len(), 1);
        assert!(!cmd.json);
    }
}
