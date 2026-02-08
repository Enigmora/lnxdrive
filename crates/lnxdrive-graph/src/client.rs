//! Microsoft Graph API client
//!
//! Provides a typed HTTP client for interacting with the Microsoft Graph API.
//! Handles authentication headers, JSON deserialization, and endpoint construction.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use lnxdrive_graph::client::GraphClient;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = GraphClient::new("access-token-here");
//! let user_info = client.get_user_info().await?;
//! println!("Hello, {}", user_info.display_name);
//! # Ok(())
//! # }
//! ```

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use lnxdrive_core::{domain::newtypes::RemoteId, ports::cloud_provider::UserInfo};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::rate_limit::{parse_retry_after, AdaptiveRateLimiter};

/// Base URL for Microsoft Graph API v1.0
const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

// ============================================================================
// Graph API response types
// ============================================================================

/// Response from the /me endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    /// User's display name
    display_name: Option<String>,
    /// User's email (mail field)
    mail: Option<String>,
    /// User's principal name (typically email)
    user_principal_name: Option<String>,
    /// User ID
    id: Option<String>,
}

/// Response from the /me/drive endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveResponse {
    /// Drive ID
    #[allow(dead_code)]
    id: Option<String>,
    /// Quota information
    quota: Option<QuotaResponse>,
}

/// Quota information from the drive response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaResponse {
    /// Total storage in bytes
    total: Option<u64>,
    /// Used storage in bytes
    used: Option<u64>,
    /// Remaining storage in bytes
    #[allow(dead_code)]
    remaining: Option<u64>,
}

// ============================================================================
// GraphClient
// ============================================================================

/// Default retry-after duration when header is missing (30 seconds)
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Maximum number of retries for 429 responses when no rate limiter is configured
const DEFAULT_MAX_RETRIES: u32 = 5;

/// HTTP client for Microsoft Graph API calls
///
/// Wraps `reqwest::Client` with authentication headers and base URL
/// construction for the Microsoft Graph API.
///
/// Optionally integrates with an [`AdaptiveRateLimiter`] for proactive
/// rate limiting and automatic 429 retry handling.
pub struct GraphClient {
    /// The underlying HTTP client
    client: Client,
    /// Base URL for API requests
    base_url: String,
    /// Current OAuth2 access token
    access_token: String,
    /// Optional adaptive rate limiter for proactive throttling
    rate_limiter: Option<Arc<AdaptiveRateLimiter>>,
}

impl GraphClient {
    /// Creates a new GraphClient with the given access token
    ///
    /// # Arguments
    /// * `access_token` - A valid OAuth2 access token for Microsoft Graph
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: GRAPH_BASE_URL.to_string(),
            access_token: access_token.into(),
            rate_limiter: None,
        }
    }

    /// Creates a new GraphClient with a custom base URL (useful for testing)
    ///
    /// # Arguments
    /// * `access_token` - A valid OAuth2 access token
    /// * `base_url` - Custom base URL for API requests
    pub fn with_base_url(access_token: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            access_token: access_token.into(),
            rate_limiter: None,
        }
    }

    /// Sets the adaptive rate limiter for this client.
    ///
    /// When a rate limiter is present, methods like [`execute_with_retry`]
    /// will acquire a token before sending requests and notify the limiter
    /// of successes and throttle events.
    ///
    /// # Arguments
    /// * `limiter` - A shared adaptive rate limiter instance
    pub fn with_rate_limiter(mut self, limiter: Arc<AdaptiveRateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Sets the rate limiter on an existing client (mutable setter variant).
    ///
    /// # Arguments
    /// * `limiter` - A shared adaptive rate limiter instance
    pub fn set_rate_limiter(&mut self, limiter: Arc<AdaptiveRateLimiter>) {
        self.rate_limiter = Some(limiter);
        debug!("Rate limiter attached to GraphClient");
    }

    /// Returns a reference to the rate limiter, if configured.
    pub fn rate_limiter(&self) -> Option<&Arc<AdaptiveRateLimiter>> {
        self.rate_limiter.as_ref()
    }

    /// Updates the access token (e.g., after a token refresh)
    ///
    /// # Arguments
    /// * `token` - The new access token
    pub fn set_access_token(&mut self, token: impl Into<String>) {
        self.access_token = token.into();
        debug!("Updated GraphClient access token");
    }

    /// Returns a reference to the current access token
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Creates an authenticated request builder for the given method and path
    ///
    /// Automatically prepends the base URL and adds the Authorization header.
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, PUT, DELETE, etc.)
    /// * `path` - API path relative to base URL (e.g., "/me" or "/me/drive")
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .request(method, &url)
            .bearer_auth(&self.access_token)
    }

    /// Retrieves information about the authenticated user
    ///
    /// Makes two API calls:
    /// 1. `GET /me` - for user profile (name, email, id)
    /// 2. `GET /me/drive` - for drive quota information
    ///
    /// # Returns
    /// A [`UserInfo`] struct with the user's profile and quota data
    pub async fn get_user_info(&self) -> Result<UserInfo> {
        debug!("Fetching user info from /me");

        // Get user profile
        let me: MeResponse = self
            .request(Method::GET, "/me")
            .send()
            .await
            .context("Failed to fetch /me")?
            .error_for_status()
            .context("GET /me returned error status")?
            .json()
            .await
            .context("Failed to parse /me response")?;

        // Get drive quota
        let (quota_used, quota_total) = self.get_drive_quota().await?;

        let email = me
            .mail
            .or(me.user_principal_name)
            .unwrap_or_else(|| "unknown@unknown.com".to_string());

        let display_name = me
            .display_name
            .unwrap_or_else(|| "Unknown User".to_string());

        let id = me.id.unwrap_or_default();

        Ok(UserInfo {
            email,
            display_name,
            id,
            quota_used,
            quota_total,
        })
    }

    /// Retrieves drive quota information
    ///
    /// # Returns
    /// A tuple of `(used_bytes, total_bytes)`
    pub async fn get_drive_quota(&self) -> Result<(u64, u64)> {
        debug!("Fetching drive quota from /me/drive");

        let drive: DriveResponse = self
            .request(Method::GET, "/me/drive")
            .send()
            .await
            .context("Failed to fetch /me/drive")?
            .error_for_status()
            .context("GET /me/drive returned error status")?
            .json()
            .await
            .context("Failed to parse /me/drive response")?;

        let used = drive.quota.as_ref().and_then(|q| q.used).unwrap_or(0);

        let total = drive.quota.as_ref().and_then(|q| q.total).unwrap_or(0);

        if total == 0 {
            warn!("Drive quota total is 0, this may indicate an API issue");
        }

        debug!("Drive quota: {} / {} bytes", used, total);
        Ok((used, total))
    }

    /// Downloads a file by its remote item ID
    ///
    /// Makes `GET /me/drive/items/{id}/content` which returns the raw file bytes.
    /// The Graph API follows a redirect to the actual download URL automatically
    /// (reqwest follows redirects by default).
    ///
    /// # Arguments
    /// * `id` - The OneDrive item ID of the file to download
    ///
    /// # Returns
    /// The file contents as a byte vector
    pub async fn download_file(&self, id: &RemoteId) -> Result<Vec<u8>> {
        let path = format!("/me/drive/items/{}/content", id.as_str());
        debug!("Downloading file: {}", id.as_str());

        let response = self
            .request(Method::GET, &path)
            .send()
            .await
            .context("Failed to send download request")?
            .error_for_status()
            .context("Download request returned error status")?;

        let bytes = response
            .bytes()
            .await
            .context("Failed to read download response body")?;

        debug!("Downloaded {} bytes for item {}", bytes.len(), id.as_str());
        Ok(bytes.to_vec())
    }

    // ========================================================================
    // T211: execute_with_retry - 429 response handling
    // ========================================================================

    /// Executes an HTTP request with automatic 429 retry and rate limiting.
    ///
    /// This method wraps the request lifecycle with:
    /// 1. **Proactive rate limiting**: If a rate limiter is configured, acquires
    ///    a token for the given endpoint before sending the request.
    /// 2. **429 handling**: On HTTP 429 (Too Many Requests), parses the
    ///    `Retry-After` header, notifies the rate limiter, sleeps, and retries.
    /// 3. **Success notification**: On a successful response, notifies the
    ///    rate limiter to support adaptive capacity recovery.
    ///
    /// # Arguments
    /// * `method` - HTTP method
    /// * `path` - API path relative to base URL
    /// * `endpoint_category` - Logical endpoint category for rate limiting
    ///   (e.g., "delta", "upload", "download", "metadata")
    ///
    /// # Returns
    /// The HTTP response on success, or an error after all retries are exhausted.
    pub async fn execute_with_retry(
        &self,
        method: Method,
        path: &str,
        endpoint_category: &str,
    ) -> Result<Response> {
        let max_retries = self
            .rate_limiter
            .as_ref()
            .map(|rl| rl.max_retries())
            .unwrap_or(DEFAULT_MAX_RETRIES);

        for attempt in 0..=max_retries {
            // Step 1: Acquire rate limit token if limiter is present
            if let Some(ref limiter) = self.rate_limiter {
                let _guard = limiter.acquire(endpoint_category).await;
            }

            // Step 2: Build and send request
            let response = self
                .request(method.clone(), path)
                .send()
                .await
                .context("Failed to send request")?;

            // Step 3: Check for 429
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                if attempt >= max_retries {
                    warn!(path, attempts = attempt + 1, "429 retry limit exhausted");
                    return Err(anyhow::anyhow!(
                        "Too many requests: retry limit exhausted after {} attempts for {}",
                        attempt + 1,
                        path
                    ));
                }

                // Parse Retry-After header
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| parse_retry_after(v, DEFAULT_RETRY_AFTER))
                    .unwrap_or(DEFAULT_RETRY_AFTER);

                // Notify rate limiter
                if let Some(ref limiter) = self.rate_limiter {
                    limiter.on_throttle(endpoint_category);
                }

                info!(
                    path,
                    attempt,
                    retry_after_ms = retry_after.as_millis(),
                    "Received 429, backing off"
                );

                tokio::time::sleep(retry_after).await;
                continue;
            }

            // Step 4: Success - notify rate limiter
            if let Some(ref limiter) = self.rate_limiter {
                limiter.on_success(endpoint_category);
            }

            if attempt > 0 {
                info!(path, attempt, "Request succeeded after retry");
            }

            return Ok(response);
        }

        Err(anyhow::anyhow!(
            "Request failed: retry loop exited unexpectedly for {}",
            path
        ))
    }

    /// Returns a reference to the underlying HTTP client
    ///
    /// This is useful for upload operations that need to make requests
    /// to absolute URLs (e.g., upload session URLs) rather than relative paths.
    pub(crate) fn http_client(&self) -> &Client {
        &self.client
    }

    /// Returns the base URL for API requests
    ///
    /// Used when constructing direct API URLs (e.g., for download URLs).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns a reference to the underlying reqwest Client
    ///
    /// Useful for making direct HTTP requests (e.g., to pre-signed download URLs).
    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limit::RateLimitConfig;

    #[test]
    fn test_graph_client_creation() {
        let client = GraphClient::new("test-token");
        assert_eq!(client.access_token(), "test-token");
        assert!(client.rate_limiter().is_none());
    }

    #[test]
    fn test_set_access_token() {
        let mut client = GraphClient::new("old-token");
        client.set_access_token("new-token");
        assert_eq!(client.access_token(), "new-token");
    }

    #[test]
    fn test_request_builder() {
        let client = GraphClient::new("test-token");
        let request = client.request(Method::GET, "/me").build().unwrap();
        assert_eq!(
            request.url().as_str(),
            "https://graph.microsoft.com/v1.0/me"
        );
        // Verify Authorization header is present
        let auth_header = request
            .headers()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(auth_header, "Bearer test-token");
    }

    #[test]
    fn test_custom_base_url() {
        let client = GraphClient::with_base_url("token", "http://localhost:8080");
        let request = client.request(Method::GET, "/me").build().unwrap();
        assert_eq!(request.url().as_str(), "http://localhost:8080/me");
    }

    #[test]
    fn test_me_response_deserialization() {
        let json = r#"{
            "displayName": "John Doe",
            "mail": "john@example.com",
            "userPrincipalName": "john@example.com",
            "id": "user-123"
        }"#;

        let me: MeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(me.display_name.unwrap(), "John Doe");
        assert_eq!(me.mail.unwrap(), "john@example.com");
        assert_eq!(me.id.unwrap(), "user-123");
    }

    #[test]
    fn test_drive_response_deserialization() {
        let json = r#"{
            "id": "drive-123",
            "quota": {
                "total": 5368709120,
                "used": 1073741824,
                "remaining": 4294967296
            }
        }"#;

        let drive: DriveResponse = serde_json::from_str(json).unwrap();
        assert_eq!(drive.id.unwrap(), "drive-123");
        let quota = drive.quota.unwrap();
        assert_eq!(quota.total.unwrap(), 5368709120);
        assert_eq!(quota.used.unwrap(), 1073741824);
    }

    #[test]
    fn test_drive_response_missing_quota() {
        let json = r#"{"id": "drive-123"}"#;

        let drive: DriveResponse = serde_json::from_str(json).unwrap();
        assert!(drive.quota.is_none());
    }

    #[test]
    fn test_me_response_partial_fields() {
        let json = r#"{"id": "user-123"}"#;

        let me: MeResponse = serde_json::from_str(json).unwrap();
        assert!(me.display_name.is_none());
        assert!(me.mail.is_none());
        assert!(me.user_principal_name.is_none());
        assert_eq!(me.id.unwrap(), "user-123");
    }

    // ====================================================================
    // T210: Rate limiter integration tests
    // ====================================================================

    #[test]
    fn test_with_rate_limiter() {
        let limiter = Arc::new(AdaptiveRateLimiter::with_defaults());
        let client = GraphClient::new("token").with_rate_limiter(limiter.clone());
        assert!(client.rate_limiter().is_some());
    }

    #[test]
    fn test_set_rate_limiter() {
        let mut client = GraphClient::new("token");
        assert!(client.rate_limiter().is_none());

        let limiter = Arc::new(AdaptiveRateLimiter::with_defaults());
        client.set_rate_limiter(limiter);
        assert!(client.rate_limiter().is_some());
    }

    #[test]
    fn test_client_without_rate_limiter() {
        let client = GraphClient::new("token");
        assert!(client.rate_limiter().is_none());
        // Should still be able to build requests
        let req = client.request(Method::GET, "/me").build().unwrap();
        assert!(req.url().as_str().contains("/me"));
    }

    #[test]
    fn test_with_rate_limiter_preserves_token() {
        let limiter = Arc::new(AdaptiveRateLimiter::with_defaults());
        let client = GraphClient::new("my-token").with_rate_limiter(limiter);
        assert_eq!(client.access_token(), "my-token");
    }

    #[test]
    fn test_with_rate_limiter_custom_config() {
        let config = RateLimitConfig {
            default_capacity: 50,
            default_refill_rate: 2.0,
            endpoint_overrides: std::collections::HashMap::new(),
            max_retries: 10,
        };
        let limiter = Arc::new(AdaptiveRateLimiter::new(config));
        let client = GraphClient::new("token").with_rate_limiter(limiter.clone());
        assert_eq!(client.rate_limiter().unwrap().max_retries(), 10);
    }
}
