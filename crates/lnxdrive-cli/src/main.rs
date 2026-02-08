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

use commands::{
    audit::AuditCommand,
    auth::AuthCommand,
    completions::CompletionsCommand,
    config::ConfigCommand,
    conflicts::ConflictsCommand,
    daemon::DaemonCommand,
    explain::ExplainCommand,
    hydrate::{DehydrateCommand, HydrateCommand},
    mount::{MountCommand, UnmountCommand},
    pin::{PinCommand, UnpinCommand},
    status::StatusCommand,
    sync::SyncCommand,
};
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
    /// Mount the Files-on-Demand FUSE filesystem
    Mount(MountCommand),
    /// Unmount the FUSE filesystem
    Unmount(UnmountCommand),
    /// Pin files or directories for permanent offline access
    Pin(PinCommand),
    /// Unpin files or directories, allowing automatic dehydration
    Unpin(UnpinCommand),
    /// Hydrate files to download their content locally
    Hydrate(HydrateCommand),
    /// Dehydrate files to free local disk space
    Dehydrate(DehydrateCommand),
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
        Commands::Mount(cmd) => cmd.execute(format).await,
        Commands::Unmount(cmd) => cmd.execute(format).await,
        Commands::Pin(cmd) => cmd.execute(format).await,
        Commands::Unpin(cmd) => cmd.execute(format).await,
        Commands::Hydrate(cmd) => cmd.execute(format).await,
        Commands::Dehydrate(cmd) => cmd.execute(format).await,
    }
}
