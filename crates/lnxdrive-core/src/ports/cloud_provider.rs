//! Cloud provider port (driven/secondary port)
//!
//! This module defines the interface for interacting with cloud storage
//! providers. The primary implementation targets Microsoft OneDrive via
//! the Microsoft Graph API, but the trait is designed to be provider-agnostic
//! to support future multi-provider scenarios.
//!
//! ## Design Notes
//!
//! - Uses `anyhow::Result` because errors at port boundaries are adapter-specific
//!   and don't need domain-level classification.
//! - Uses `#[async_trait]` for async trait methods.
//! - The `DeltaItem` struct is a port-level DTO, not a domain entity;
//!   use cases are responsible for mapping it to `SyncItem`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::newtypes::{DeltaToken, RemoteId, RemotePath};

// ============================================================================
// T048: AuthFlow enum
// ============================================================================

/// OAuth authentication flow configuration
///
/// Defines how the application should authenticate with the cloud provider.
/// Currently supports the Authorization Code flow with PKCE, which is the
/// recommended flow for native/desktop applications.
#[derive(Debug, Clone)]
pub enum AuthFlow {
    /// OAuth 2.0 Authorization Code flow with PKCE (RFC 7636)
    ///
    /// This is the recommended flow for native applications that cannot
    /// securely store a client secret.
    AuthorizationCodePKCE {
        /// Application (client) ID registered with the provider
        app_id: String,
        /// Redirect URI for receiving the authorization code
        redirect_uri: String,
        /// OAuth scopes to request (e.g., "Files.ReadWrite.All", "offline_access")
        scopes: Vec<String>,
    },
}

// ============================================================================
// T049: Tokens struct
// ============================================================================

/// OAuth tokens received from the cloud provider
///
/// Contains the access token for API requests, an optional refresh token
/// for obtaining new access tokens, and the expiration time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    /// Bearer token for authenticating API requests
    pub access_token: String,
    /// Token for refreshing the access token without user interaction
    /// (requires `offline_access` scope)
    pub refresh_token: Option<String>,
    /// When the access token expires
    pub expires_at: DateTime<Utc>,
}

impl Tokens {
    /// Returns true if the access token has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Returns true if the access token will expire within the given duration
    pub fn expires_within(&self, duration: chrono::Duration) -> bool {
        Utc::now() + duration >= self.expires_at
    }
}

// ============================================================================
// T050: DeltaResponse and DeltaItem structs
// ============================================================================

/// Response from a delta (incremental changes) query
///
/// Contains the list of changed items and pagination/continuation tokens.
/// The delta mechanism enables efficient incremental synchronization by
/// only returning items that have changed since the last query.
#[derive(Debug, Clone)]
pub struct DeltaResponse {
    /// List of items that have changed since the last delta query
    pub items: Vec<DeltaItem>,
    /// URL for fetching the next page of results (None if this is the last page)
    pub next_link: Option<String>,
    /// Token for the next delta query (present only on the last page)
    pub delta_link: Option<String>,
}

/// A single item from a delta query response
///
/// This is a port-level DTO that represents raw data from the cloud provider.
/// Use cases are responsible for mapping `DeltaItem` instances to domain
/// `SyncItem` entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaItem {
    /// Provider-specific item identifier
    pub id: String,
    /// Item name (file or folder name)
    pub name: String,
    /// Full path of the item in the cloud (None for deleted items)
    pub path: Option<String>,
    /// File size in bytes (None for folders or deleted items)
    pub size: Option<u64>,
    /// Content hash for integrity verification (None for folders)
    pub hash: Option<String>,
    /// Last modified timestamp (None for deleted items)
    pub modified: Option<DateTime<Utc>>,
    /// Whether this item has been deleted since the last delta
    pub is_deleted: bool,
    /// Whether this item is a directory/folder
    pub is_directory: bool,
    /// Parent folder ID (None for root items)
    pub parent_id: Option<String>,
}

// ============================================================================
// T051: UserInfo struct
// ============================================================================

/// Information about the authenticated user
///
/// Retrieved from the cloud provider's user profile endpoint.
/// Used during account setup and for displaying account details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User's email address
    pub email: String,
    /// User's display name
    pub display_name: String,
    /// Provider-specific user/drive identifier
    pub id: String,
    /// Storage quota used in bytes
    pub quota_used: u64,
    /// Total storage quota in bytes
    pub quota_total: u64,
}

// ============================================================================
// T052: ICloudProvider trait
// ============================================================================

/// Port trait for cloud storage provider operations
///
/// This is the primary interface for all interactions with the cloud storage
/// backend. Implementations handle the provider-specific API calls, authentication,
/// rate limiting, and error mapping.
///
/// ## Implementation Notes
///
/// - Implementations should handle retry logic for transient errors internally
///   or propagate them as `anyhow::Error` for the use case layer to handle.
/// - The `progress` callback in `upload_file_session` is called with
///   `(bytes_sent, total_bytes)` to report upload progress.
/// - All methods assume that valid authentication tokens are available;
///   token refresh should be handled by the implementation or a wrapper.
#[async_trait::async_trait]
pub trait ICloudProvider: Send + Sync {
    /// Initiates the OAuth authentication flow
    ///
    /// # Arguments
    /// * `auth_flow` - Configuration for the authentication flow
    ///
    /// # Returns
    /// OAuth tokens on successful authentication
    async fn authenticate(&self, auth_flow: &AuthFlow) -> anyhow::Result<Tokens>;

    /// Refreshes an expired access token using a refresh token
    ///
    /// # Arguments
    /// * `refresh_token` - The refresh token from a previous authentication
    ///
    /// # Returns
    /// New OAuth tokens with a fresh access token
    async fn refresh_tokens(&self, refresh_token: &str) -> anyhow::Result<Tokens>;

    /// Queries for changes since the last delta token
    ///
    /// If `token` is `None`, returns all items (initial sync).
    /// If `token` is `Some`, returns only items changed since that token.
    ///
    /// # Arguments
    /// * `token` - Delta token from a previous query (None for initial sync)
    ///
    /// # Returns
    /// A response containing changed items and continuation/delta tokens
    async fn get_delta(&self, token: Option<&DeltaToken>) -> anyhow::Result<DeltaResponse>;

    /// Downloads a file's content by its remote ID
    ///
    /// # Arguments
    /// * `remote_id` - The provider-specific identifier for the file
    ///
    /// # Returns
    /// The file contents as a byte vector
    async fn download_file(&self, remote_id: &RemoteId) -> anyhow::Result<Vec<u8>>;

    /// Uploads a small file (< 4MB for OneDrive) in a single request
    ///
    /// # Arguments
    /// * `parent_path` - The remote path of the parent folder
    /// * `name` - The file name
    /// * `data` - The file contents
    /// * `if_match_etag` - Optional ETag for conditional upload (If-Match header).
    ///   When provided, the upload will only succeed if the remote file's current
    ///   ETag matches. This prevents overwriting concurrent changes (race condition
    ///   protection). Pass `None` for unconditional upload.
    ///
    /// # Returns
    /// Metadata of the uploaded file
    async fn upload_file(
        &self,
        parent_path: &RemotePath,
        name: &str,
        data: &[u8],
        if_match_etag: Option<&str>,
    ) -> anyhow::Result<DeltaItem>;

    /// Uploads a large file using a resumable upload session
    ///
    /// This method should be used for files larger than the provider's
    /// simple upload size limit (4MB for OneDrive).
    ///
    /// # Arguments
    /// * `parent_path` - The remote path of the parent folder
    /// * `name` - The file name
    /// * `data` - The file contents
    /// * `progress` - Optional callback reporting (bytes_sent, total_bytes)
    ///
    /// # Returns
    /// Metadata of the uploaded file
    async fn upload_file_session(
        &self,
        parent_path: &RemotePath,
        name: &str,
        data: &[u8],
        progress: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> anyhow::Result<DeltaItem>;

    /// Retrieves metadata for a specific item by its remote ID
    ///
    /// # Arguments
    /// * `remote_id` - The provider-specific identifier for the item
    ///
    /// # Returns
    /// The item's metadata
    async fn get_metadata(&self, remote_id: &RemoteId) -> anyhow::Result<DeltaItem>;

    /// Retrieves information about the authenticated user
    ///
    /// # Returns
    /// User profile and quota information
    async fn get_user_info(&self) -> anyhow::Result<UserInfo>;

    /// Deletes an item from the cloud storage
    ///
    /// # Arguments
    /// * `remote_id` - The provider-specific identifier for the item to delete
    async fn delete_item(&self, remote_id: &RemoteId) -> anyhow::Result<()>;
}
