//! Auth commands - Login, Logout, and Status for OneDrive authentication
//!
//! Provides the `lnxdrive auth` CLI subcommands which:
//! 1. `login`  - Runs the OAuth2 PKCE flow via GraphAuthAdapter, stores tokens
//!    in the system keyring, fetches user info, and persists the account in SQLite.
//! 2. `logout` - Clears tokens from the keyring and suspends the account.
//! 3. `status` - Shows current account info and token validity.

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use clap::Subcommand;
use tracing::info;

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
            AuthCommand::Login { app_id } => self.execute_login(app_id.as_deref(), &*fmt).await,
            AuthCommand::Logout => self.execute_logout(&*fmt).await,
            AuthCommand::Status => self.execute_status(&*fmt, format).await,
        }
    }

    /// Execute the login flow:
    /// 1. Load config to get app_id
    /// 2. Run OAuth2 PKCE via GraphAuthAdapter
    /// 3. Store tokens in keyring
    /// 4. Fetch user info from Graph API
    /// 5. Create and persist Account in SQLite
    /// 6. Record audit entry
    async fn execute_login(
        &self,
        cli_app_id: Option<&str>,
        fmt: &dyn crate::output::OutputFormatter,
    ) -> Result<()> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
        use lnxdrive_core::{
            config::Config,
            domain::{Account, AuditAction, AuditEntry, AuditResult, Email, SyncPath},
            ports::{cloud_provider::ICloudProvider, state_repository::IStateRepository},
        };
        use lnxdrive_graph::{
            auth::{GraphAuthAdapter, KeyringTokenStorage},
            client::GraphClient,
            provider::GraphCloudProvider,
        };

        // Step 1: Load config to get app_id
        let config_path = Config::default_path();
        let config = Config::load_or_default(&config_path);

        let app_id = cli_app_id
            .map(|s| s.to_string())
            .or(config.auth.app_id.clone())
            .context("No app_id provided. Use --app-id flag or set auth.app_id in config.yaml")?;

        info!(app_id = %app_id, "Starting OAuth2 login");

        // Step 2: Run OAuth2 PKCE flow
        fmt.info("Opening browser for Microsoft login...");
        let auth_adapter = GraphAuthAdapter::with_app_id(&app_id);
        let tokens = auth_adapter.login().await.context("OAuth2 login failed")?;

        // Step 3: Fetch user info from Graph API
        fmt.info("Retrieving account information...");
        let graph_client = GraphClient::new(&tokens.access_token);
        let cloud_provider = GraphCloudProvider::new(graph_client);
        let user_info = cloud_provider
            .get_user_info()
            .await
            .context("Failed to retrieve user info from Graph API")?;

        info!(email = %user_info.email, display_name = %user_info.display_name, "Got user info");

        // Step 4: Store tokens in keyring
        KeyringTokenStorage::store(&user_info.email, &tokens)
            .context("Failed to store tokens in keyring")?;

        // Step 5: Open database and persist account
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        let email = Email::new(user_info.email.clone()).context("Invalid email from Graph API")?;

        let sync_root =
            SyncPath::new(config.sync.root.clone()).context("Invalid sync root path in config")?;

        let mut account = Account::new(email, &user_info.display_name, &user_info.id, sync_root);
        account.update_quota(user_info.quota_used, user_info.quota_total);

        state_repo
            .save_account(&account)
            .await
            .context("Failed to save account to database")?;

        // Step 6: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::AuthLogin, AuditResult::success())
            .with_details(serde_json::json!({
                "email": user_info.email,
                "display_name": user_info.display_name,
                "drive_id": user_info.id,
            }));

        state_repo
            .save_audit(&audit_entry)
            .await
            .context("Failed to save audit entry")?;

        // Step 7: Display results
        fmt.success(&format!(
            "Authenticated as {} ({})",
            user_info.display_name, user_info.email
        ));

        let quota_used_mb = user_info.quota_used as f64 / 1_048_576.0;
        let quota_total_gb = user_info.quota_total as f64 / 1_073_741_824.0;
        fmt.info(&format!(
            "Storage: {:.1} MB used / {:.1} GB total ({:.1}%)",
            quota_used_mb,
            quota_total_gb,
            account.quota_percent()
        ));
        fmt.info(&format!("Sync root: {}", config.sync.root.display()));

        Ok(())
    }

    /// Execute logout:
    /// 1. Get default account from DB
    /// 2. Clear tokens from keyring
    /// 3. Suspend account in DB
    /// 4. Record audit entry
    async fn execute_logout(&self, fmt: &dyn crate::output::OutputFormatter) -> Result<()> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
        use lnxdrive_core::{
            domain::{AuditAction, AuditEntry, AuditResult},
            ports::state_repository::IStateRepository,
        };
        use lnxdrive_graph::auth::KeyringTokenStorage;

        // Step 1: Open database and get default account
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        let account = state_repo
            .get_default_account()
            .await
            .context("Failed to query default account")?;

        let mut account = match account {
            Some(a) => a,
            None => {
                fmt.info("No account configured. Nothing to log out.");
                return Ok(());
            }
        };

        let email = account.email().as_str().to_string();
        info!(email = %email, "Logging out");

        // Step 2: Clear tokens from keyring
        KeyringTokenStorage::clear(&email).context("Failed to clear tokens from keyring")?;

        // Step 3: Suspend account
        account.suspend();
        state_repo
            .save_account(&account)
            .await
            .context("Failed to update account in database")?;

        // Step 4: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::AuthLogout, AuditResult::success())
            .with_details(serde_json::json!({
                "email": email,
            }));

        state_repo
            .save_audit(&audit_entry)
            .await
            .context("Failed to save audit entry")?;

        fmt.success("Logged out successfully");
        fmt.info("Credentials removed from keyring");

        Ok(())
    }

    /// Execute status check:
    /// 1. Get default account from DB
    /// 2. Check token state in keyring
    /// 3. Display account info and token validity
    async fn execute_status(
        &self,
        fmt: &dyn crate::output::OutputFormatter,
        format: OutputFormat,
    ) -> Result<()> {
        use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
        use lnxdrive_core::ports::state_repository::IStateRepository;
        use lnxdrive_graph::auth::KeyringTokenStorage;

        // Step 1: Open database and get default account
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        if !db_path.exists() {
            fmt.info("Authentication status: Not configured");
            fmt.info("Run 'lnxdrive auth login' to authenticate");
            return Ok(());
        }

        let pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(pool.pool().clone()));

        let account = state_repo
            .get_default_account()
            .await
            .context("Failed to query default account")?;

        let account = match account {
            Some(a) => a,
            None => {
                fmt.info("Authentication status: Not configured");
                fmt.info("Run 'lnxdrive auth login' to authenticate");
                return Ok(());
            }
        };

        // Step 2: Check tokens in keyring
        let email = account.email().as_str();
        let token_status = match KeyringTokenStorage::load(email) {
            Ok(Some(tokens)) => {
                if tokens.is_expired() {
                    "Expired"
                } else {
                    "Valid"
                }
            }
            Ok(None) => "Not found",
            Err(_) => "Error reading keyring",
        };

        // Step 3: Display results
        if matches!(format, OutputFormat::Json) {
            let json = serde_json::json!({
                "authenticated": true,
                "email": email,
                "display_name": account.display_name(),
                "drive_id": account.onedrive_id(),
                "state": format!("{}", account.state()),
                "token_status": token_status,
                "sync_root": account.sync_root().to_string(),
                "quota_used": account.quota_used(),
                "quota_total": account.quota_total(),
                "quota_percent": account.quota_percent(),
                "last_sync": account.last_sync().map(|t| t.to_rfc3339()),
            });
            fmt.print_json(&json);
        } else {
            fmt.success(&format!(
                "Authenticated as {} ({})",
                account.display_name(),
                email
            ));
            fmt.info(&format!("Account state: {}", account.state()));
            fmt.info(&format!("Token status:  {}", token_status));
            fmt.info(&format!("Drive ID:      {}", account.onedrive_id()));
            fmt.info(&format!("Sync root:     {}", account.sync_root()));

            let quota_used_mb = account.quota_used() as f64 / 1_048_576.0;
            let quota_total_gb = account.quota_total() as f64 / 1_073_741_824.0;
            fmt.info(&format!(
                "Storage:       {:.1} MB / {:.1} GB ({:.1}%)",
                quota_used_mb,
                quota_total_gb,
                account.quota_percent()
            ));

            if let Some(last_sync) = account.last_sync() {
                fmt.info(&format!(
                    "Last sync:     {}",
                    last_sync.format("%Y-%m-%d %H:%M:%S UTC")
                ));
            } else {
                fmt.info("Last sync:     Never");
            }
        }

        Ok(())
    }
}
