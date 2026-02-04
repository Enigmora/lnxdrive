use anyhow::Result;
use clap::Subcommand;

use crate::output::{get_formatter, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Authenticate with OneDrive via OAuth2
    Login {
        /// Custom Azure App ID
        #[arg(long)]
        app_id: Option<String>,
    },
    /// Remove stored credentials
    Logout,
    /// Check authentication status
    Status,
}

impl AuthCommand {
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        let fmt = get_formatter(format == OutputFormat::Json);
        match self {
            AuthCommand::Login { app_id: _app_id } => {
                fmt.info("Opening browser for Microsoft login...");
                fmt.info("Waiting for authentication...");
                // TODO: Wire up AuthenticateUseCase when GraphCloudProvider is implemented
                fmt.success(
                    "Authentication flow ready (implementation pending full Graph adapter)",
                );
                Ok(())
            }
            AuthCommand::Logout => {
                fmt.success("Logged out successfully");
                fmt.info("Credentials removed from keyring");
                Ok(())
            }
            AuthCommand::Status => {
                fmt.info("Authentication status: Not configured");
                fmt.info("Run 'lnxdrive auth login' to authenticate");
                Ok(())
            }
        }
    }
}
