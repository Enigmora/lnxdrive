//! LNXDrive CLI - Command-line interface for LNXDrive
//!
//! Provides commands for:
//! - Authentication with OneDrive
//! - Viewing sync status
//! - Managing conflicts
//! - Controlling the daemon
//! - Explaining file states

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod output;

use commands::audit::AuditCommand;
use commands::auth::AuthCommand;
use commands::completions::CompletionsCommand;
use commands::config::ConfigCommand;
use commands::conflicts::ConflictsCommand;
use commands::daemon::DaemonCommand;
use commands::explain::ExplainCommand;
use commands::status::StatusCommand;
use commands::sync::SyncCommand;
use output::OutputFormat;

#[derive(Debug, Parser)]
#[command(name = "lnxdrive", version, about = "Native OneDrive client for Linux")]
pub struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Verbose output (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Use alternate config file
    #[arg(long, global = true)]
    config: Option<String>,

    /// Minimal output
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Authentication commands
    #[command(subcommand)]
    Auth(AuthCommand),
    /// Synchronize files with OneDrive
    Sync(SyncCommand),
    /// Show synchronization status
    Status(StatusCommand),
    /// Explain why a file is in its current state
    Explain(ExplainCommand),
    /// View audit log entries
    Audit(AuditCommand),
    /// Manage the LNXDrive background daemon
    #[command(subcommand)]
    Daemon(DaemonCommand),
    /// View and manage configuration
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Manage synchronization conflicts
    #[command(subcommand)]
    Conflicts(ConflictsCommand),
    /// Generate shell completions
    Completions(CompletionsCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup tracing
    let filter = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    let format = if cli.json {
        OutputFormat::Json
    } else {
        OutputFormat::Human
    };

    match cli.command {
        Commands::Auth(cmd) => cmd.execute(format).await,
        Commands::Sync(cmd) => cmd.execute(format).await,
        Commands::Status(cmd) => cmd.execute(format).await,
        Commands::Explain(cmd) => cmd.execute(format).await,
        Commands::Audit(cmd) => cmd.execute(format).await,
        Commands::Daemon(cmd) => cmd.execute(format).await,
        Commands::Config(cmd) => cmd.execute(format).await,
        Commands::Conflicts(cmd) => cmd.execute(format).await,
        Commands::Completions(cmd) => cmd.execute(format).await,
    }
}
