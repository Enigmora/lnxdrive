//! Report command - Manage crash and error reports
//!
//! Provides the `lnxdrive report` CLI command with subcommands:
//! - `list`: Show all saved reports
//! - `view <id>`: Display a specific report
//! - `send`: Print what would be sent (OTLP endpoint deferred)
//! - `delete`: Remove reports from local storage

use anyhow::Result;
use clap::Subcommand;

use crate::output::{get_formatter, OutputFormat};

/// Report management subcommands
#[derive(Debug, Subcommand)]
pub enum ReportCommand {
    /// List all saved crash and error reports
    List,
    /// View a specific report
    View {
        /// Report ID or filename fragment
        id: String,
        /// Show raw JSON instead of pretty-print
        #[arg(long)]
        json: bool,
    },
    /// Show what would be sent (actual OTLP submission is deferred)
    Send {
        /// Specific report ID (omit for all unsent)
        id: Option<String>,
        /// Send all unsent reports
        #[arg(long)]
        all: bool,
    },
    /// Delete reports from local storage
    Delete {
        /// Specific report ID to delete
        id: Option<String>,
        /// Delete all reports
        #[arg(long)]
        all: bool,
    },
}

impl ReportCommand {
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_telemetry::LocalReportStore;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));
        let store = LocalReportStore::new(LocalReportStore::default_dir());

        match self {
            ReportCommand::List => {
                let entries = store.list()?;
                if entries.is_empty() {
                    formatter.info("No reports found.");
                    return Ok(());
                }

                if matches!(format, OutputFormat::Json) {
                    let json: Vec<serde_json::Value> = entries
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "id": e.id,
                                "type": e.report_type,
                                "date": e.date,
                                "size_bytes": e.size_bytes,
                            })
                        })
                        .collect();
                    formatter.print_json(&serde_json::json!(json));
                } else {
                    println!(
                        "{:<12} {:<8} {:<12} {:>10}",
                        "ID", "Type", "Date", "Size"
                    );
                    println!("{}", "-".repeat(46));
                    for entry in &entries {
                        println!(
                            "{:<12} {:<8} {:<12} {:>10}",
                            entry.id,
                            entry.report_type,
                            entry.date,
                            format_size(entry.size_bytes),
                        );
                    }
                    println!();
                    println!("Total: {} report(s)", entries.len());
                }
            }

            ReportCommand::View { id, json } => {
                match store.read(id)? {
                    Some(value) => {
                        if *json || matches!(format, OutputFormat::Json) {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&value).unwrap_or_default()
                            );
                        } else {
                            // Pretty-print key fields
                            if let Some(obj) = value.as_object() {
                                for (key, val) in obj {
                                    match val {
                                        serde_json::Value::String(s) => {
                                            println!("{}: {}", key, s);
                                        }
                                        serde_json::Value::Object(_) => {
                                            println!(
                                                "{}: {}",
                                                key,
                                                serde_json::to_string_pretty(val)
                                                    .unwrap_or_default()
                                            );
                                        }
                                        other => {
                                            println!("{}: {}", key, other);
                                        }
                                    }
                                }
                            } else {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&value).unwrap_or_default()
                                );
                            }
                        }
                    }
                    None => {
                        formatter.error(&format!("Report '{}' not found", id));
                    }
                }
            }

            ReportCommand::Send { id, all } => {
                let entries = store.list()?;
                let to_send: Vec<_> = if *all {
                    entries
                } else if let Some(ref report_id) = id {
                    entries.into_iter().filter(|e| e.id == *report_id).collect()
                } else {
                    entries
                };

                if to_send.is_empty() {
                    formatter.info("No reports to send.");
                    return Ok(());
                }

                formatter.info(&format!(
                    "Would send {} report(s) to telemetry endpoint:",
                    to_send.len()
                ));
                for entry in &to_send {
                    formatter.info(&format!(
                        "  {} ({}, {})",
                        entry.id,
                        entry.report_type,
                        format_size(entry.size_bytes)
                    ));
                }
                formatter
                    .info("(OTLP submission is not yet implemented; reports are stored locally)");
            }

            ReportCommand::Delete { id, all } => {
                if *all {
                    let count = store.delete_all()?;
                    formatter.success(&format!("Deleted {} report(s)", count));
                } else if let Some(ref report_id) = id {
                    if store.delete(report_id)? {
                        formatter.success(&format!("Deleted report '{}'", report_id));
                    } else {
                        formatter.error(&format!("Report '{}' not found", report_id));
                    }
                } else {
                    formatter.error("Specify a report ID or use --all");
                }
            }
        }

        Ok(())
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
