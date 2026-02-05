//! Conflicts command - Manage synchronization conflicts
//!
//! Provides the `lnxdrive conflicts` CLI command which:
//! 1. Lists all unresolved conflicts in a table format
//! 2. Resolves a specific conflict by ID with a chosen strategy
//! 3. Previews conflict details showing local vs remote metadata

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Subcommand;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

/// T237: Conflicts subcommands
#[derive(Debug, Subcommand)]
pub enum ConflictsCommand {
    /// List unresolved conflicts
    List,
    /// Resolve a conflict
    Resolve {
        /// Conflict ID
        id: String,
        /// Resolution strategy: local, remote, keep_both
        #[arg(long)]
        strategy: String,
    },
    /// Preview conflict details
    Preview {
        /// Conflict ID
        id: String,
    },
}

impl ConflictsCommand {
    /// Execute the conflicts command
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        match self {
            ConflictsCommand::List => self.execute_list(format).await,
            ConflictsCommand::Resolve { id, strategy } => {
                self.execute_resolve(id, strategy, format).await
            }
            ConflictsCommand::Preview { id } => self.execute_preview(id, format).await,
        }
    }

    /// Open the database and return a state repository
    async fn open_database(
        &self,
        formatter: &dyn crate::output::OutputFormatter,
    ) -> Result<Option<Arc<lnxdrive_cache::SqliteStateRepository>>> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};

        let db_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        if !db_path.exists() {
            formatter
                .error("No database found. Run 'lnxdrive auth login' and 'lnxdrive sync' first.");
            return Ok(None);
        }

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        Ok(Some(state_repo))
    }

    /// T238: List unresolved conflicts
    async fn execute_list(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::ports::state_repository::IStateRepository;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let state_repo = match self.open_database(&*formatter).await? {
            Some(repo) => repo,
            None => return Ok(()),
        };

        let conflicts = state_repo
            .get_unresolved_conflicts()
            .await
            .context("Failed to query unresolved conflicts")?;

        info!(count = conflicts.len(), "Retrieved unresolved conflicts");

        if matches!(format, OutputFormat::Json) {
            let conflicts_json: Vec<serde_json::Value> = conflicts
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id().to_string(),
                        "item_id": c.item_id().to_string(),
                        "detected_at": c.detected_at().to_rfc3339(),
                        "local_version": {
                            "hash": c.local_version().hash().to_string(),
                            "size_bytes": c.local_version().size_bytes(),
                            "modified_at": c.local_version().modified_at().to_rfc3339(),
                        },
                        "remote_version": {
                            "hash": c.remote_version().hash().to_string(),
                            "size_bytes": c.remote_version().size_bytes(),
                            "modified_at": c.remote_version().modified_at().to_rfc3339(),
                        },
                    })
                })
                .collect();

            let json = serde_json::json!({
                "count": conflicts.len(),
                "conflicts": conflicts_json,
            });
            formatter.print_json(&json);
            return Ok(());
        }

        // Human-readable table output
        if conflicts.is_empty() {
            formatter.success("No unresolved conflicts");
            return Ok(());
        }

        formatter.success(&format!(
            "{} unresolved conflict{}",
            conflicts.len(),
            if conflicts.len() == 1 { "" } else { "s" }
        ));
        formatter.info("");
        formatter.info("  ID (short)     Detected              Local Size  Remote Size");
        formatter.info("  -------------- --------------------- ----------- -----------");

        for conflict in &conflicts {
            let id_short = truncate_id(conflict.id().to_string(), 14);
            let detected = conflict
                .detected_at()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            let local_size = format_bytes(conflict.local_version().size_bytes());
            let remote_size = format_bytes(conflict.remote_version().size_bytes());

            formatter.info(&format!(
                "  {:<14} {} {:>11} {:>11}",
                id_short, detected, local_size, remote_size
            ));
        }

        formatter.info("");
        formatter.info("Use 'lnxdrive conflicts preview <id>' for details.");
        formatter.info(
            "Use 'lnxdrive conflicts resolve <id> --strategy <local|remote|keep_both>' to resolve.",
        );

        Ok(())
    }

    /// T239: Resolve a conflict by ID
    async fn execute_resolve(&self, id: &str, strategy: &str, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::{
            domain::conflict::{Resolution, ResolutionSource},
            ports::state_repository::IStateRepository,
        };

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let state_repo = match self.open_database(&*formatter).await? {
            Some(repo) => repo,
            None => return Ok(()),
        };

        // Parse the resolution strategy
        let resolution = match strategy {
            "local" | "keep_local" => Resolution::KeepLocal,
            "remote" | "keep_remote" => Resolution::KeepRemote,
            "keep_both" | "both" => Resolution::KeepBoth,
            _ => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("Unknown strategy: '{}'. Use: local, remote, keep_both", strategy),
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.error(&format!(
                        "Unknown strategy: '{}'. Valid strategies: local, remote, keep_both",
                        strategy
                    ));
                }
                return Ok(());
            }
        };

        // Find the conflict by searching unresolved conflicts
        let conflicts = state_repo
            .get_unresolved_conflicts()
            .await
            .context("Failed to query conflicts")?;

        let conflict = conflicts.into_iter().find(|c| {
            let cid = c.id().to_string();
            cid == id || cid.starts_with(id)
        });

        let conflict = match conflict {
            Some(c) => c,
            None => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("No unresolved conflict found with ID: {}", id),
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.error(&format!("No unresolved conflict found with ID: {}", id));
                    formatter.info("Use 'lnxdrive conflicts list' to see unresolved conflicts.");
                }
                return Ok(());
            }
        };

        let conflict_id_str = conflict.id().to_string();

        info!(
            conflict_id = %conflict_id_str,
            strategy = %strategy,
            "Resolving conflict"
        );

        // Resolve the conflict
        let resolved = conflict.resolve(resolution.clone(), ResolutionSource::User);

        // Save the resolved conflict
        state_repo
            .save_conflict(&resolved)
            .await
            .context("Failed to save resolved conflict")?;

        if matches!(format, OutputFormat::Json) {
            let json = serde_json::json!({
                "success": true,
                "conflict_id": conflict_id_str,
                "resolution": resolution.to_string(),
                "resolved_by": "user",
            });
            formatter.print_json(&json);
        } else {
            formatter.success(&format!(
                "Conflict {} resolved with strategy: {}",
                truncate_id(conflict_id_str, 14),
                resolution
            ));
        }

        Ok(())
    }

    /// T240: Preview conflict details
    async fn execute_preview(&self, id: &str, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::ports::state_repository::IStateRepository;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let state_repo = match self.open_database(&*formatter).await? {
            Some(repo) => repo,
            None => return Ok(()),
        };

        // Find the conflict by searching unresolved conflicts
        let conflicts = state_repo
            .get_unresolved_conflicts()
            .await
            .context("Failed to query conflicts")?;

        let conflict = conflicts.iter().find(|c| {
            let cid = c.id().to_string();
            cid == id || cid.starts_with(id)
        });

        let conflict = match conflict {
            Some(c) => c,
            None => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("No unresolved conflict found with ID: {}", id),
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.error(&format!("No unresolved conflict found with ID: {}", id));
                    formatter.info("Use 'lnxdrive conflicts list' to see unresolved conflicts.");
                }
                return Ok(());
            }
        };

        if matches!(format, OutputFormat::Json) {
            let json = serde_json::json!({
                "id": conflict.id().to_string(),
                "item_id": conflict.item_id().to_string(),
                "detected_at": conflict.detected_at().to_rfc3339(),
                "is_resolved": conflict.is_resolved(),
                "local_version": {
                    "hash": conflict.local_version().hash().to_string(),
                    "size_bytes": conflict.local_version().size_bytes(),
                    "modified_at": conflict.local_version().modified_at().to_rfc3339(),
                    "etag": conflict.local_version().etag(),
                },
                "remote_version": {
                    "hash": conflict.remote_version().hash().to_string(),
                    "size_bytes": conflict.remote_version().size_bytes(),
                    "modified_at": conflict.remote_version().modified_at().to_rfc3339(),
                    "etag": conflict.remote_version().etag(),
                },
            });
            formatter.print_json(&json);
            return Ok(());
        }

        // Human-readable detail output
        formatter.success(&format!("Conflict Details: {}", conflict.id()));
        formatter.info("");
        formatter.info(&format!("Item ID:     {}", conflict.item_id()));
        formatter.info(&format!(
            "Detected:    {}",
            conflict.detected_at().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        formatter.info(&format!(
            "Resolved:    {}",
            if conflict.is_resolved() { "Yes" } else { "No" }
        ));

        formatter.info("");
        formatter.info("Local Version:");
        formatter.info(&format!(
            "  Hash:        {}",
            conflict.local_version().hash()
        ));
        formatter.info(&format!(
            "  Size:        {}",
            format_bytes(conflict.local_version().size_bytes())
        ));
        formatter.info(&format!(
            "  Modified:    {}",
            conflict
                .local_version()
                .modified_at()
                .format("%Y-%m-%d %H:%M:%S UTC")
        ));
        if let Some(etag) = conflict.local_version().etag() {
            formatter.info(&format!("  ETag:        {}", etag));
        }

        formatter.info("");
        formatter.info("Remote Version:");
        formatter.info(&format!(
            "  Hash:        {}",
            conflict.remote_version().hash()
        ));
        formatter.info(&format!(
            "  Size:        {}",
            format_bytes(conflict.remote_version().size_bytes())
        ));
        formatter.info(&format!(
            "  Modified:    {}",
            conflict
                .remote_version()
                .modified_at()
                .format("%Y-%m-%d %H:%M:%S UTC")
        ));
        if let Some(etag) = conflict.remote_version().etag() {
            formatter.info(&format!("  ETag:        {}", etag));
        }

        // Comparison summary
        formatter.info("");
        formatter.info("Comparison:");
        let size_diff = conflict.remote_version().size_bytes() as i64
            - conflict.local_version().size_bytes() as i64;
        let size_indicator = if size_diff > 0 {
            format!(
                "Remote is {} larger",
                format_bytes(size_diff.unsigned_abs())
            )
        } else if size_diff < 0 {
            format!(
                "Local is {} larger",
                format_bytes((-size_diff).unsigned_abs())
            )
        } else {
            "Same size".to_string()
        };
        formatter.info(&format!("  Size diff:   {}", size_indicator));

        let local_newer =
            conflict.local_version().modified_at() > conflict.remote_version().modified_at();
        formatter.info(&format!(
            "  Newer:       {}",
            if local_newer { "Local" } else { "Remote" }
        ));

        let hashes_match = conflict.local_version().hash() == conflict.remote_version().hash();
        formatter.info(&format!(
            "  Hashes:      {}",
            if hashes_match {
                "Match (content is identical)"
            } else {
                "Different (content has diverged)"
            }
        ));

        formatter.info("");
        formatter.info("To resolve, run:");
        formatter.info(&format!(
            "  lnxdrive conflicts resolve {} --strategy <local|remote|keep_both>",
            truncate_id(conflict.id().to_string(), 14)
        ));

        Ok(())
    }
}

/// Truncate a UUID string for display, showing only the first N characters
fn truncate_id(id: String, max_len: usize) -> String {
    if id.len() <= max_len {
        id
    } else {
        format!("{}...", &id[..max_len - 3])
    }
}

/// Format a byte count into a human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_id_short() {
        let id = "abc123".to_string();
        assert_eq!(truncate_id(id, 14), "abc123");
    }

    #[test]
    fn test_truncate_id_long() {
        let id = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let result = truncate_id(id, 14);
        assert_eq!(result.len(), 14);
        assert!(result.ends_with("..."));
        assert_eq!(result, "550e8400-e2...");
    }

    #[test]
    fn test_truncate_id_exact() {
        let id = "12345678901234".to_string();
        assert_eq!(truncate_id(id, 14), "12345678901234");
    }

    #[test]
    fn test_format_bytes_small() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_kib() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
    }

    #[test]
    fn test_format_bytes_mib() {
        assert_eq!(format_bytes(1048576), "1.0 MiB");
        assert_eq!(format_bytes(5 * 1048576), "5.0 MiB");
    }

    #[test]
    fn test_format_bytes_gib() {
        assert_eq!(format_bytes(1073741824), "1.0 GiB");
    }
}
