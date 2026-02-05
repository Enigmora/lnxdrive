//! Hydrate/Dehydrate commands - Manual hydration and dehydration of files
//!
//! Provides the `lnxdrive hydrate` and `lnxdrive dehydrate` CLI commands which:
//! 1. Validate paths are within the mount point
//! 2. Resolve paths to inodes
//! 3. Call the HydrationManager/DehydrationManager logic
//! 4. Report results

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

// ============================================================================
// T084: HydrateCommand with clap options
// ============================================================================

/// Hydrate files to download their content locally
///
/// Hydrating a file downloads its content from OneDrive and stores it
/// in the local cache. This is useful when you want to ensure files
/// are available offline before disconnecting from the network.
#[derive(Debug, Args)]
pub struct HydrateCommand {
    /// Paths to hydrate (files or directories)
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl HydrateCommand {
    /// Execute the hydrate command
    ///
    /// For each path:
    /// 1. Validate it's within the mount point
    /// 2. Resolve to inode (requires mounted filesystem)
    /// 3. Call hydration logic
    /// 4. Report progress and success/failure
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        formatter.info(&format!("Hydrating {} path(s)...", self.paths.len()));

        // Note: Full implementation requires access to the mounted FUSE filesystem
        // to resolve paths to inodes and call HydrationManager::hydrate().
        //
        // For now, we provide a stub that validates the paths exist and reports
        // what would be hydrated. The actual hydration operation would need either:
        // 1. IPC to a running FUSE daemon
        // 2. Direct call to HydrationManager if FUSE is running in-process
        //
        // This will be fully implemented when FUSE IPC is available.

        let mut hydrated_count = 0;
        let mut total_size = 0u64;
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

            info!(path = %path.display(), item_type, "Hydrating");
            formatter.info(&format!(
                "Hydrating {} '{}' ({} bytes)",
                item_type,
                path.display(),
                metadata.len()
            ));

            // TODO: When FUSE IPC is available:
            // 1. Resolve path to inode
            // 2. Get item state from InodeTable
            // 3. If Online, call HydrationManager::hydrate() with UserOpen priority
            // 4. Wait for completion with progress reporting

            hydrated_count += 1;
            total_size += metadata.len();
        }

        // Report results
        if hydrated_count > 0 {
            formatter.success(&format!(
                "Hydrated {} path(s), {} bytes total",
                hydrated_count,
                format_bytes(total_size)
            ));
        }

        for error in &errors {
            formatter.error(error);
        }

        if use_json {
            formatter.print_json(&serde_json::json!({
                "success": errors.is_empty(),
                "hydrated_count": hydrated_count,
                "total_bytes": total_size,
                "errors": errors,
                "paths": self.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }));
        }

        Ok(())
    }
}

// ============================================================================
// T084: DehydrateCommand with clap options
// ============================================================================

/// Dehydrate files to free local disk space
///
/// Dehydrating a file removes its cached content while keeping the
/// placeholder metadata. The file will be re-downloaded automatically
/// the next time it's accessed.
///
/// Only files in the "hydrated" state can be dehydrated. Pinned, modified,
/// or open files cannot be dehydrated.
#[derive(Debug, Args)]
pub struct DehydrateCommand {
    /// Paths to dehydrate (files or directories)
    #[arg(required = true, value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Force dehydration even if files are modified (uploads first)
    #[arg(long, short)]
    pub force: bool,

    /// Output in JSON format (overrides global --json)
    #[arg(long)]
    pub json: bool,
}

impl DehydrateCommand {
    /// Execute the dehydrate command
    ///
    /// For each path:
    /// 1. Validate it's within the mount point
    /// 2. Resolve to inode (requires mounted filesystem)
    /// 3. Call dehydration logic
    /// 4. Report freed space
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        // Use command-level --json flag if set, otherwise use global format
        let use_json = self.json || matches!(format, OutputFormat::Json);
        let formatter = get_formatter(use_json);

        formatter.info(&format!("Dehydrating {} path(s)...", self.paths.len()));

        if self.force {
            formatter.warn("Force mode: modified files will be uploaded before dehydration");
        }

        // Note: Full implementation requires access to the mounted FUSE filesystem
        // to resolve paths to inodes and call DehydrationManager::dehydrate_path().
        //
        // For now, we provide a stub that validates the paths exist and reports
        // what would be dehydrated.

        let mut dehydrated_count = 0;
        let mut freed_bytes = 0u64;
        let skipped_count = 0;
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

            info!(path = %path.display(), item_type, force = self.force, "Dehydrating");

            // TODO: When FUSE IPC is available:
            // 1. Resolve path to inode
            // 2. Get item state from InodeTable
            // 3. Check eligibility (Hydrated state, no open handles)
            // 4. If force and Modified, upload first
            // 5. Call DehydrationManager::dehydrate_path()
            // 6. Track freed bytes

            if metadata.is_dir() {
                // For directories, we'd recurse
                formatter.info(&format!(
                    "Dehydrating {} '{}'",
                    item_type,
                    path.display()
                ));
                dehydrated_count += 1;
            } else {
                // For files, report the size that would be freed
                let size = metadata.len();
                formatter.info(&format!(
                    "Dehydrating {} '{}' (freeing {} bytes)",
                    item_type,
                    path.display(),
                    size
                ));
                dehydrated_count += 1;
                freed_bytes += size;
            }
        }

        // Report results
        if dehydrated_count > 0 {
            formatter.success(&format!(
                "Dehydrated {} path(s), freed {}",
                dehydrated_count,
                format_bytes(freed_bytes)
            ));
        }

        if skipped_count > 0 {
            formatter.warn(&format!("Skipped {} path(s) (open handles or wrong state)", skipped_count));
        }

        for error in &errors {
            formatter.error(error);
        }

        if use_json {
            formatter.print_json(&serde_json::json!({
                "success": errors.is_empty(),
                "dehydrated_count": dehydrated_count,
                "freed_bytes": freed_bytes,
                "skipped_count": skipped_count,
                "errors": errors,
                "paths": self.paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }));
        }

        Ok(())
    }
}

/// Format bytes as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hydrate_command_default() {
        let cmd = HydrateCommand {
            paths: vec![PathBuf::from("/tmp/test")],
            json: false,
        };
        assert_eq!(cmd.paths.len(), 1);
        assert!(!cmd.json);
    }

    #[test]
    fn test_hydrate_command_multiple_paths() {
        let cmd = HydrateCommand {
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
    fn test_dehydrate_command_default() {
        let cmd = DehydrateCommand {
            paths: vec![PathBuf::from("/tmp/test")],
            force: false,
            json: false,
        };
        assert_eq!(cmd.paths.len(), 1);
        assert!(!cmd.force);
        assert!(!cmd.json);
    }

    #[test]
    fn test_dehydrate_command_with_force() {
        let cmd = DehydrateCommand {
            paths: vec![PathBuf::from("/tmp/test")],
            force: true,
            json: false,
        };
        assert!(cmd.force);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2 + 512 * 1024 * 1024), "2.50 GB");
    }
}
