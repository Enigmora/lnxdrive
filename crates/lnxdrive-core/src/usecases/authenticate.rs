//! Authentication use case
//!
//! Orchestrates the OAuth2 PKCE authentication flow with Microsoft Identity,
//! including login, logout, token refresh, and account status queries.
//! Delegates actual OAuth operations to the cloud provider port and
//! persistence to the state repository port.

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Duration;
use serde_json::json;

use crate::{
    domain::{Account, AccountId, AuditAction, AuditEntry, AuditResult, Email, SyncPath},
    ports::{AuthFlow, ICloudProvider, IStateRepository, Tokens},
};

/// Default OAuth redirect URI for the desktop application
const DEFAULT_REDIRECT_URI: &str = "http://localhost:8400";

/// Default OAuth scopes for OneDrive access
const DEFAULT_SCOPES: &[&str] = &["Files.ReadWrite.All", "User.Read", "offline_access"];

/// Default Microsoft application (client) ID
const DEFAULT_APP_ID: &str = "d50ca740-c83f-4d1b-b616-12c519384f0c";

/// Use case for authentication operations
///
/// Coordinates OAuth2 PKCE flow, token management, and account lifecycle
/// between the cloud provider and state repository ports.
pub struct AuthenticateUseCase {
    cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
    state_repository: Arc<dyn IStateRepository + Send + Sync>,
}

impl AuthenticateUseCase {
    /// Creates a new AuthenticateUseCase with the required dependencies
    ///
    /// # Arguments
    ///
    /// * `cloud_provider` - Cloud storage provider for OAuth operations
    /// * `state_repository` - Persistent storage for account state and audit log
    pub fn new(
        cloud_provider: Arc<dyn ICloudProvider + Send + Sync>,
        state_repository: Arc<dyn IStateRepository + Send + Sync>,
    ) -> Self {
        Self {
            cloud_provider,
            state_repository,
        }
    }

    /// Initiates the OAuth2 PKCE authentication flow
    ///
    /// This method:
    /// 1. Builds an AuthFlow configuration and authenticates via the cloud provider
    /// 2. Retrieves the user profile information
    /// 3. Creates an Account entity in the domain
    /// 4. Persists the account to the state repository
    /// 5. Records an audit entry for the login event
    ///
    /// # Arguments
    ///
    /// * `app_id` - Optional application/client ID override (uses default if None)
    ///
    /// # Returns
    ///
    /// The newly created and persisted Account entity
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The OAuth flow fails (browser launch, callback, token exchange)
    /// - User info retrieval fails
    /// - Account persistence fails
    pub async fn login(&self, app_id: Option<String>) -> Result<Account> {
        // Step 1: Build AuthFlow and initiate OAuth2 PKCE flow via cloud provider
        let auth_flow = AuthFlow::AuthorizationCodePKCE {
            app_id: app_id.unwrap_or_else(|| DEFAULT_APP_ID.to_string()),
            redirect_uri: DEFAULT_REDIRECT_URI.to_string(),
            scopes: DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
        };

        let _tokens = self
            .cloud_provider
            .authenticate(&auth_flow)
            .await
            .context("Failed to complete OAuth2 authentication flow")?;

        // Step 2: Get user profile information (cloud provider uses its internal tokens)
        let user_info = self
            .cloud_provider
            .get_user_info()
            .await
            .context("Failed to retrieve user profile from cloud provider")?;

        // Step 3: Create the Account domain entity
        let sync_root = SyncPath::new(dirs_default_sync_root(&user_info.display_name))
            .context("Failed to create default sync root path")?;

        let email = Email::new(user_info.email.clone())
            .context("Invalid email address from cloud provider")?;

        let account = Account::new(
            email,
            user_info.display_name.clone(),
            user_info.id.clone(),
            sync_root,
        );

        // Step 4: Persist account to state repository
        self.state_repository
            .save_account(&account)
            .await
            .context("Failed to persist account")?;

        // Step 5: Record audit entry for the login event
        let audit_entry = AuditEntry::new(AuditAction::AuthLogin, AuditResult::success())
            .with_details(json!({
                "account_id": account.id().to_string(),
                "email": account.email().as_str(),
                "drive_id": user_info.id,
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record login audit entry")?;

        Ok(account)
    }

    /// Logs out an account by suspending it
    ///
    /// This method:
    /// 1. Retrieves the account from the repository
    /// 2. Suspends the account
    /// 3. Persists the updated account
    /// 4. Records an audit entry for the logout event
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account to log out
    ///
    /// # Errors
    ///
    /// Returns an error if the account is not found or persistence fails
    pub async fn logout(&self, account_id: &AccountId) -> Result<()> {
        // Step 1: Retrieve account
        let mut account = self
            .state_repository
            .get_account(account_id)
            .await
            .context("Failed to retrieve account for logout")?
            .context("Account not found")?;

        // Step 2: Suspend the account
        account.suspend();

        // Step 3: Persist updated state
        self.state_repository
            .save_account(&account)
            .await
            .context("Failed to persist account state after logout")?;

        // Step 4: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::AuthLogout, AuditResult::success())
            .with_details(json!({
                "account_id": account_id.to_string(),
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record logout audit entry")?;

        Ok(())
    }

    /// Refreshes tokens if they are expiring soon (within 5 minutes)
    ///
    /// This method:
    /// 1. Checks if the access token expires within the next 5 minutes
    /// 2. If so, refreshes via the cloud provider using the refresh token
    /// 3. If the account was in TokenExpired state, reactivates it
    /// 4. Records an audit entry for the token refresh
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account whose tokens to check/refresh
    /// * `tokens` - The current tokens to evaluate and potentially refresh
    ///
    /// # Returns
    ///
    /// The current (or refreshed) tokens
    ///
    /// # Errors
    ///
    /// Returns an error if token refresh or persistence fails
    pub async fn refresh_if_needed(
        &self,
        account_id: &AccountId,
        tokens: &Tokens,
    ) -> Result<Tokens> {
        // Step 1: Check if refresh is needed (within 5 minutes of expiry)
        if !tokens.expires_within(Duration::minutes(5)) {
            return Ok(tokens.clone());
        }

        // Step 2: Refresh tokens via cloud provider
        let refresh_token_str = tokens
            .refresh_token
            .as_deref()
            .context("No refresh token available for token refresh")?;

        let new_tokens = self
            .cloud_provider
            .refresh_tokens(refresh_token_str)
            .await
            .context("Failed to refresh authentication tokens")?;

        // Step 3: Update account state to Active if it was TokenExpired
        let account = self
            .state_repository
            .get_account(account_id)
            .await
            .context("Failed to retrieve account after token refresh")?;

        if let Some(mut account) = account {
            if account.state().needs_token_refresh() {
                account.activate();
                self.state_repository
                    .save_account(&account)
                    .await
                    .context("Failed to update account state after token refresh")?;
            }
        }

        // Step 4: Record audit entry
        let audit_entry = AuditEntry::new(AuditAction::AuthRefresh, AuditResult::success())
            .with_details(json!({
                "account_id": account_id.to_string(),
            }));

        self.state_repository
            .save_audit(&audit_entry)
            .await
            .context("Failed to record token refresh audit entry")?;

        Ok(new_tokens)
    }

    /// Retrieves the current status of an account
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account to query
    ///
    /// # Returns
    ///
    /// The Account if found, or None if the account does not exist
    ///
    /// # Errors
    ///
    /// Returns an error if the repository query fails
    pub async fn get_status(&self, account_id: &AccountId) -> Result<Option<Account>> {
        self.state_repository
            .get_account(account_id)
            .await
            .context("Failed to retrieve account status")
    }
}

/// Computes the default sync root path for an account
///
/// Returns `$HOME/OneDrive` as a PathBuf. Falls back to `/tmp/OneDrive`
/// if the home directory cannot be determined.
fn dirs_default_sync_root(display_name: &str) -> std::path::PathBuf {
    let _ = display_name; // May be used for multi-account naming in the future
    match std::env::var("HOME") {
        Ok(home) => std::path::PathBuf::from(home).join("OneDrive"),
        Err(_) => std::path::PathBuf::from("/tmp/OneDrive"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirs_default_sync_root() {
        let path = dirs_default_sync_root("Test User");
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("OneDrive"));
    }
}
