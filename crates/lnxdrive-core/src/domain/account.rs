//! Account domain entity
//!
//! This module defines the Account entity which represents a user's
//! OneDrive account and its synchronization state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    errors::DomainError,
    newtypes::{AccountId, DeltaToken, Email, SyncPath},
};

/// T031: Represents the current state of an account
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    /// Account is active and can sync
    #[default]
    Active,
    /// OAuth token has expired, needs refresh
    TokenExpired,
    /// Account has been suspended (e.g., by Microsoft)
    Suspended,
    /// Account is in an error state with a description
    Error(String),
}

impl AccountState {
    /// Returns true if the account can perform sync operations
    pub fn can_sync(&self) -> bool {
        matches!(self, AccountState::Active)
    }

    /// Returns true if the account needs token refresh
    pub fn needs_token_refresh(&self) -> bool {
        matches!(self, AccountState::TokenExpired)
    }
}

impl std::fmt::Display for AccountState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountState::Active => write!(f, "active"),
            AccountState::TokenExpired => write!(f, "token_expired"),
            AccountState::Suspended => write!(f, "suspended"),
            AccountState::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

/// T032: Represents a user's OneDrive account
///
/// An Account entity contains all information needed to identify and
/// manage a user's OneDrive connection, including authentication state,
/// quota information, and sync configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Unique identifier for this account
    id: AccountId,
    /// User's email address (Microsoft account)
    email: Email,
    /// Display name from Microsoft profile
    display_name: String,
    /// OneDrive drive ID from Microsoft Graph API
    onedrive_id: String,
    /// Local path where files are synchronized
    sync_root: SyncPath,
    /// Bytes used in OneDrive storage
    quota_used: u64,
    /// Total bytes available in OneDrive storage
    quota_total: u64,
    /// Delta token for incremental sync (None for initial sync)
    delta_token: Option<DeltaToken>,
    /// Timestamp of last successful sync (None if never synced)
    last_sync: Option<DateTime<Utc>>,
    /// Current account state
    state: AccountState,
    /// When this account was created
    created_at: DateTime<Utc>,
}

impl Account {
    /// T033: Creates a new Account with the provided details
    ///
    /// # Arguments
    /// * `email` - User's Microsoft account email
    /// * `display_name` - User's display name
    /// * `onedrive_id` - OneDrive drive ID from Graph API
    /// * `sync_root` - Local path for file synchronization
    ///
    /// # Returns
    /// A new Account in Active state with zero quota usage
    pub fn new(
        email: Email,
        display_name: impl Into<String>,
        onedrive_id: impl Into<String>,
        sync_root: SyncPath,
    ) -> Self {
        Self {
            id: AccountId::new(),
            email,
            display_name: display_name.into(),
            onedrive_id: onedrive_id.into(),
            sync_root,
            quota_used: 0,
            quota_total: 0,
            delta_token: None,
            last_sync: None,
            state: AccountState::Active,
            created_at: Utc::now(),
        }
    }

    /// Creates an Account with a specific ID (for reconstitution from storage)
    pub fn with_id(
        id: AccountId,
        email: Email,
        display_name: impl Into<String>,
        onedrive_id: impl Into<String>,
        sync_root: SyncPath,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            email,
            display_name: display_name.into(),
            onedrive_id: onedrive_id.into(),
            sync_root,
            quota_used: 0,
            quota_total: 0,
            delta_token: None,
            last_sync: None,
            state: AccountState::Active,
            created_at,
        }
    }

    // --- Getters ---

    /// Returns the account's unique identifier
    pub fn id(&self) -> &AccountId {
        &self.id
    }

    /// Returns the account's email address
    pub fn email(&self) -> &Email {
        &self.email
    }

    /// Returns the account's display name
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the OneDrive drive ID
    pub fn onedrive_id(&self) -> &str {
        &self.onedrive_id
    }

    /// Returns the sync root path
    pub fn sync_root(&self) -> &SyncPath {
        &self.sync_root
    }

    /// Returns bytes used in storage
    pub fn quota_used(&self) -> u64 {
        self.quota_used
    }

    /// Returns total bytes available
    pub fn quota_total(&self) -> u64 {
        self.quota_total
    }

    /// Returns the current delta token if any
    pub fn delta_token(&self) -> Option<&DeltaToken> {
        self.delta_token.as_ref()
    }

    /// Returns the last sync timestamp if any
    pub fn last_sync(&self) -> Option<DateTime<Utc>> {
        self.last_sync
    }

    /// Returns the current account state
    pub fn state(&self) -> &AccountState {
        &self.state
    }

    /// Returns when the account was created
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    // --- T033: Methods ---

    /// T033: Calculates the percentage of quota used
    ///
    /// # Returns
    /// - Percentage as f64 between 0.0 and 100.0
    /// - Returns 0.0 if quota_total is 0 to avoid division by zero
    pub fn quota_percent(&self) -> f64 {
        if self.quota_total == 0 {
            return 0.0;
        }
        (self.quota_used as f64 / self.quota_total as f64) * 100.0
    }

    // --- Setters / State Mutations ---

    /// Updates the quota information
    pub fn update_quota(&mut self, used: u64, total: u64) {
        self.quota_used = used;
        self.quota_total = total;
    }

    /// Updates the delta token after a successful sync
    pub fn update_delta_token(&mut self, token: DeltaToken) {
        self.delta_token = Some(token);
    }

    /// Clears the delta token (forces full resync)
    pub fn clear_delta_token(&mut self) {
        self.delta_token = None;
    }

    /// Records a successful sync
    pub fn record_sync(&mut self, timestamp: DateTime<Utc>) {
        self.last_sync = Some(timestamp);
    }

    /// Updates the account state
    pub fn set_state(&mut self, state: AccountState) {
        self.state = state;
    }

    /// Marks the account as active
    pub fn activate(&mut self) {
        self.state = AccountState::Active;
    }

    /// Marks the token as expired
    pub fn mark_token_expired(&mut self) {
        self.state = AccountState::TokenExpired;
    }

    /// Marks the account as suspended
    pub fn suspend(&mut self) {
        self.state = AccountState::Suspended;
    }

    /// Marks the account with an error
    pub fn mark_error(&mut self, reason: impl Into<String>) {
        self.state = AccountState::Error(reason.into());
    }

    /// Updates the sync root path
    ///
    /// # Errors
    /// Returns `DomainError::InvalidPath` if the new path is not absolute
    pub fn update_sync_root(&mut self, path: SyncPath) -> Result<(), DomainError> {
        self.sync_root = path;
        Ok(())
    }

    /// Returns true if the account can perform sync operations
    pub fn can_sync(&self) -> bool {
        self.state.can_sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_account() -> Account {
        use std::path::PathBuf;
        let email = Email::new("test@example.com".to_string()).unwrap();
        let sync_root = SyncPath::new(PathBuf::from("/home/user/OneDrive")).unwrap();
        Account::new(email, "Test User", "drive123", sync_root)
    }

    mod account_state_tests {
        use super::*;

        #[test]
        fn test_can_sync_active() {
            assert!(AccountState::Active.can_sync());
        }

        #[test]
        fn test_cannot_sync_token_expired() {
            assert!(!AccountState::TokenExpired.can_sync());
        }

        #[test]
        fn test_cannot_sync_suspended() {
            assert!(!AccountState::Suspended.can_sync());
        }

        #[test]
        fn test_cannot_sync_error() {
            assert!(!AccountState::Error("test".to_string()).can_sync());
        }

        #[test]
        fn test_needs_token_refresh() {
            assert!(AccountState::TokenExpired.needs_token_refresh());
            assert!(!AccountState::Active.needs_token_refresh());
        }

        #[test]
        fn test_display() {
            assert_eq!(format!("{}", AccountState::Active), "active");
            assert_eq!(format!("{}", AccountState::TokenExpired), "token_expired");
            assert_eq!(format!("{}", AccountState::Suspended), "suspended");
            assert_eq!(
                format!("{}", AccountState::Error("network".to_string())),
                "error: network"
            );
        }

        #[test]
        fn test_serialization() {
            let active = AccountState::Active;
            let json = serde_json::to_string(&active).unwrap();
            assert_eq!(json, "\"active\"");

            let error = AccountState::Error("test error".to_string());
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(json, "{\"error\":\"test error\"}");
        }
    }

    mod account_tests {
        use super::*;

        #[test]
        fn test_new_account() {
            let account = create_test_account();

            assert_eq!(account.email().as_str(), "test@example.com");
            assert_eq!(account.display_name(), "Test User");
            assert_eq!(account.onedrive_id(), "drive123");
            assert_eq!(account.sync_root().to_string(), "/home/user/OneDrive");
            assert_eq!(account.quota_used(), 0);
            assert_eq!(account.quota_total(), 0);
            assert!(account.delta_token().is_none());
            assert!(account.last_sync().is_none());
            assert_eq!(*account.state(), AccountState::Active);
        }

        #[test]
        fn test_quota_percent_normal() {
            let mut account = create_test_account();
            account.update_quota(50, 100);
            assert!((account.quota_percent() - 50.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_quota_percent_zero_total() {
            let account = create_test_account();
            assert!((account.quota_percent() - 0.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_quota_percent_full() {
            let mut account = create_test_account();
            account.update_quota(100, 100);
            assert!((account.quota_percent() - 100.0).abs() < f64::EPSILON);
        }

        #[test]
        fn test_quota_percent_real_values() {
            let mut account = create_test_account();
            // 5GB used of 15GB
            account.update_quota(5_368_709_120, 16_106_127_360);
            let percent = account.quota_percent();
            assert!(percent > 33.0 && percent < 34.0);
        }

        #[test]
        fn test_update_delta_token() {
            let mut account = create_test_account();
            let token = DeltaToken::new("new_token_123".to_string()).unwrap();
            account.update_delta_token(token);
            assert_eq!(account.delta_token().unwrap().as_str(), "new_token_123");
        }

        #[test]
        fn test_clear_delta_token() {
            let mut account = create_test_account();
            let token = DeltaToken::new("token".to_string()).unwrap();
            account.update_delta_token(token);
            account.clear_delta_token();
            assert!(account.delta_token().is_none());
        }

        #[test]
        fn test_record_sync() {
            let mut account = create_test_account();
            let timestamp = Utc::now();
            account.record_sync(timestamp);
            assert_eq!(account.last_sync(), Some(timestamp));
        }

        #[test]
        fn test_state_transitions() {
            let mut account = create_test_account();

            account.mark_token_expired();
            assert_eq!(*account.state(), AccountState::TokenExpired);
            assert!(!account.can_sync());

            account.activate();
            assert_eq!(*account.state(), AccountState::Active);
            assert!(account.can_sync());

            account.suspend();
            assert_eq!(*account.state(), AccountState::Suspended);
            assert!(!account.can_sync());

            account.mark_error("Network failure");
            assert!(
                matches!(account.state(), AccountState::Error(msg) if msg == "Network failure")
            );
            assert!(!account.can_sync());
        }

        #[test]
        fn test_serialization_roundtrip() {
            let account = create_test_account();
            let json = serde_json::to_string(&account).unwrap();
            let deserialized: Account = serde_json::from_str(&json).unwrap();

            assert_eq!(account.email(), deserialized.email());
            assert_eq!(account.display_name(), deserialized.display_name());
            assert_eq!(account.onedrive_id(), deserialized.onedrive_id());
            assert_eq!(*account.state(), *deserialized.state());
        }
    }
}
