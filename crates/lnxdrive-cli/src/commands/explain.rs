//! Explain command - Explain why a file is in its current state
//!
//! Provides the `lnxdrive explain <path>` CLI command which:
//! 1. Looks up a file in the sync state database
//! 2. Generates a human-readable explanation of its current state
//! 3. Provides actionable suggestions based on the state/error
//! 4. Shows recent audit history for the file

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

/// T194: Explain command with required path argument
#[derive(Debug, Args)]
pub struct ExplainCommand {
    /// Path to the file to explain
    pub path: String,
}

impl ExplainCommand {
    /// T195-T198: Execute the explain command
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_cache::pool::DatabasePool;
        use lnxdrive_cache::SqliteStateRepository;
        use lnxdrive_core::domain::newtypes::SyncPath;
        use lnxdrive_core::usecases::ExplainFailureUseCase;

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

        // T195: Resolve the path to absolute
        let abs_path = if PathBuf::from(&self.path).is_absolute() {
            PathBuf::from(&self.path)
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(&self.path)
        };

        let sync_path =
            SyncPath::new(abs_path).context("Invalid path - must be an absolute path")?;

        info!(path = %sync_path, "Explaining file state");

        // Use the ExplainFailureUseCase from core
        let use_case = ExplainFailureUseCase::new(state_repo);
        let explanation = use_case
            .explain(&sync_path)
            .await
            .context("Failed to generate explanation")?;

        // Display the explanation
        if matches!(format, OutputFormat::Json) {
            let history_json: Vec<serde_json::Value> = explanation
                .history
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "timestamp": entry.timestamp().to_rfc3339(),
                        "action": entry.action().to_string(),
                        "result": format!("{:?}", entry.result()),
                        "details": entry.details(),
                        "duration_ms": entry.duration_ms(),
                    })
                })
                .collect();

            let json = serde_json::json!({
                "path": explanation.path.to_string(),
                "state": explanation.state,
                "message": explanation.message,
                "suggestions": explanation.suggestions,
                "history": history_json,
            });
            formatter.print_json(&json);
            return Ok(());
        }

        // T196: Human-readable explanation display
        formatter.success(&format!("Explanation for: {}", explanation.path));
        formatter.info("");
        formatter.info(&format!("State:   {}", explanation.state));
        formatter.info(&format!("Message: {}", explanation.message));

        // T197: Suggestions
        if !explanation.suggestions.is_empty() {
            formatter.info("");
            formatter.info("Suggestions:");
            for suggestion in &explanation.suggestions {
                formatter.info(&format!("  - {}", suggestion));
            }
        }

        // T198: History display
        if !explanation.history.is_empty() {
            formatter.info("");
            formatter.info("Recent history:");
            formatter.info("  Timestamp                Action           Result");
            formatter.info("  ----------------------- ---------------- -------");

            // Show up to 10 most recent entries
            let entries_to_show = if explanation.history.len() > 10 {
                &explanation.history[explanation.history.len() - 10..]
            } else {
                &explanation.history
            };

            for entry in entries_to_show {
                let timestamp = entry.timestamp().format("%Y-%m-%d %H:%M:%S");
                let action = entry.action().to_string();
                let result = if entry.result().is_success() {
                    "OK"
                } else {
                    "FAILED"
                };

                formatter.info(&format!("  {} {:<16} {}", timestamp, action, result));
            }

            if explanation.history.len() > 10 {
                formatter.info(&format!(
                    "  ... and {} more entries (use 'lnxdrive audit --path <path>' for full history)",
                    explanation.history.len() - 10
                ));
            }
        } else {
            formatter.info("");
            formatter.info("No audit history available for this file.");
        }

        Ok(())
    }
}
