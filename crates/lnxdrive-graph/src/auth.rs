//! OAuth2 PKCE authentication flow for Microsoft Graph API
//!
//! Implements the Authorization Code flow with PKCE (RFC 7636) for
//! authenticating native desktop applications with Microsoft identity platform.
//!
//! ## Components
//!
//! - [`OAuth2Config`] - Configuration for the OAuth2 flow
//! - [`KeyringTokenStorage`] - Secure token storage using the system keyring
//! - [`PKCEFlow`] - OAuth2 PKCE challenge/exchange logic
//! - [`LocalCallbackServer`] - Minimal HTTP server for the OAuth redirect
//! - [`GraphAuthAdapter`] - Orchestrates the full authentication flow

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use lnxdrive_core::ports::cloud_provider::Tokens;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope,
    TokenResponse, TokenUrl,
};
// serde is used by Tokens (from lnxdrive-core) for JSON serialization in KeyringTokenStorage
use tracing::{debug, info, warn};

/// Default Microsoft OAuth2 authorization endpoint (consumers tenant)
const AUTH_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";

/// Default Microsoft OAuth2 token endpoint (consumers tenant)
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

/// Default redirect URI for the local callback server
const REDIRECT_URI: &str = "http://127.0.0.1:8400/callback";

/// Keyring service name for storing tokens
const KEYRING_SERVICE: &str = "lnxdrive";

/// Default OAuth2 scopes for OneDrive access
const DEFAULT_SCOPES: &[&str] = &["Files.ReadWrite.All", "User.Read", "offline_access"];

// ============================================================================
// OAuth2Config
// ============================================================================

/// Configuration for the OAuth2 PKCE authentication flow
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    /// Application (client) ID from Azure AD app registration
    pub app_id: String,
    /// Redirect URI for receiving the authorization code
    pub redirect_uri: String,
    /// OAuth scopes to request
    pub scopes: Vec<String>,
}

impl OAuth2Config {
    /// Creates a new OAuth2Config with the given app_id and default settings
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            redirect_uri: REDIRECT_URI.to_string(),
            scopes: DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Creates a config with custom scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Creates a config with a custom redirect URI
    pub fn with_redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = uri.into();
        self
    }
}

// ============================================================================
// KeyringTokenStorage
// ============================================================================

/// Stores and retrieves OAuth tokens from the system keyring
///
/// Uses the `keyring` crate to store tokens securely in the OS credential
/// store (e.g., GNOME Keyring, KDE Wallet, macOS Keychain).
/// Tokens are serialized as JSON with the service name "lnxdrive" and the
/// user's email as the username.
pub struct KeyringTokenStorage;

impl KeyringTokenStorage {
    /// Stores tokens in the system keyring for the given user
    ///
    /// # Arguments
    /// * `username` - The user's email address (used as keyring username)
    /// * `tokens` - The OAuth tokens to store
    pub fn store(username: &str, tokens: &Tokens) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .context("Failed to create keyring entry")?;

        let json = serde_json::to_string(tokens).context("Failed to serialize tokens")?;

        entry
            .set_password(&json)
            .context("Failed to store tokens in keyring")?;

        debug!("Stored tokens in keyring for user: {}", username);
        Ok(())
    }

    /// Loads tokens from the system keyring for the given user
    ///
    /// # Arguments
    /// * `username` - The user's email address (used as keyring username)
    ///
    /// # Returns
    /// `Some(Tokens)` if found and valid, `None` if not found
    pub fn load(username: &str) -> Result<Option<Tokens>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .context("Failed to create keyring entry")?;

        match entry.get_password() {
            Ok(json) => {
                let tokens: Tokens = serde_json::from_str(&json)
                    .context("Failed to deserialize tokens from keyring")?;
                debug!("Loaded tokens from keyring for user: {}", username);
                Ok(Some(tokens))
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No tokens found in keyring for user: {}", username);
                Ok(None)
            }
            Err(e) => Err(anyhow::Error::new(e).context("Failed to read from keyring")),
        }
    }

    /// Removes tokens from the system keyring for the given user
    ///
    /// # Arguments
    /// * `username` - The user's email address (used as keyring username)
    pub fn clear(username: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, username)
            .context("Failed to create keyring entry")?;

        match entry.delete_credential() {
            Ok(()) => {
                info!("Cleared tokens from keyring for user: {}", username);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No tokens to clear for user: {}", username);
                Ok(())
            }
            Err(e) => Err(anyhow::Error::new(e).context("Failed to delete from keyring")),
        }
    }
}

// ============================================================================
// PKCEFlow
// ============================================================================

/// OAuth2 PKCE flow implementation using the `oauth2` crate
///
/// Handles generating authorization URLs with PKCE challenges,
/// exchanging authorization codes for tokens, and refreshing tokens.
pub struct PKCEFlow {
    client: BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
    scopes: Vec<String>,
}

impl PKCEFlow {
    /// Creates a new PKCEFlow with the given configuration
    pub fn new(config: &OAuth2Config) -> Result<Self> {
        let client = BasicClient::new(ClientId::new(config.app_id.clone()))
            .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).context("Invalid authorization URL")?)
            .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).context("Invalid token URL")?)
            .set_redirect_uri(
                RedirectUrl::new(config.redirect_uri.clone()).context("Invalid redirect URI")?,
            );

        Ok(Self {
            client,
            scopes: config.scopes.clone(),
        })
    }

    /// Generates an authorization URL with a PKCE challenge
    ///
    /// # Returns
    /// A tuple of `(authorization_url, csrf_token, pkce_verifier)`.
    /// The `pkce_verifier` must be kept until the code exchange step.
    pub fn generate_auth_url(&self) -> (String, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut auth_request = self.client.authorize_url(CsrfToken::new_random);

        for scope in &self.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = auth_request.set_pkce_challenge(pkce_challenge).url();

        debug!("Generated authorization URL");
        (auth_url.to_string(), csrf_token, pkce_verifier)
    }

    /// Exchanges an authorization code for OAuth tokens
    ///
    /// # Arguments
    /// * `code` - The authorization code received from the callback
    /// * `pkce_verifier` - The PKCE verifier generated alongside the auth URL
    ///
    /// # Returns
    /// OAuth tokens on success
    pub async fn exchange_code(
        &self,
        code: String,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<Tokens> {
        info!("Exchanging authorization code for tokens");

        let http_client = reqwest::Client::new();
        let token_result = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .context("Failed to exchange authorization code")?;

        let expires_at = token_result
            .expires_in()
            .map(|d| Utc::now() + Duration::seconds(d.as_secs() as i64))
            .unwrap_or_else(|| Utc::now() + Duration::hours(1));

        let tokens = Tokens {
            access_token: token_result.access_token().secret().to_string(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().to_string()),
            expires_at,
        };

        info!("Successfully obtained OAuth tokens");
        Ok(tokens)
    }

    /// Refreshes an expired access token using a refresh token
    ///
    /// # Arguments
    /// * `refresh_token` - The refresh token from a previous authentication
    ///
    /// # Returns
    /// New OAuth tokens with a fresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<Tokens> {
        info!("Refreshing access token");

        let http_client = reqwest::Client::new();
        let token_result = self
            .client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
            .request_async(&http_client)
            .await
            .context("Failed to refresh token")?;

        let expires_at = token_result
            .expires_in()
            .map(|d| Utc::now() + Duration::seconds(d.as_secs() as i64))
            .unwrap_or_else(|| Utc::now() + Duration::hours(1));

        let tokens = Tokens {
            access_token: token_result.access_token().secret().to_string(),
            refresh_token: token_result
                .refresh_token()
                .map(|t| t.secret().to_string())
                .or_else(|| Some(refresh_token.to_string())),
            expires_at,
        };

        info!("Successfully refreshed access token");
        Ok(tokens)
    }
}

// ============================================================================
// LocalCallbackServer
// ============================================================================

/// Minimal HTTP server that listens on localhost for the OAuth2 redirect callback.
///
/// Starts an HTTP server on `127.0.0.1:8400` that waits for the OAuth provider
/// to redirect the user's browser back with an authorization code. Once the
/// code is received, it responds with a success HTML page and shuts down.
pub struct LocalCallbackServer;

/// Parameters extracted from the OAuth2 callback
#[derive(Debug)]
pub struct CallbackParams {
    /// The authorization code
    pub code: String,
    /// The CSRF state parameter
    pub state: String,
}

impl LocalCallbackServer {
    /// Starts the local callback server and waits for the OAuth redirect
    ///
    /// # Returns
    /// The callback parameters (code and state) extracted from the redirect URL
    pub async fn start() -> Result<CallbackParams> {
        use http_body_util::Full;
        use hyper::body::Bytes;
        use hyper::server::conn::http1;
        use hyper::service::service_fn;
        use hyper::{Request, Response, StatusCode};
        use hyper_util::rt::TokioIo;
        use tokio::net::TcpListener;
        use tokio::sync::oneshot;

        info!("Starting local OAuth callback server on 127.0.0.1:8400");

        let listener = TcpListener::bind("127.0.0.1:8400")
            .await
            .context("Failed to bind callback server to 127.0.0.1:8400")?;

        let (tx, rx) = oneshot::channel::<CallbackParams>();
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        // Accept a single connection
        let (stream, _addr) = listener
            .accept()
            .await
            .context("Failed to accept connection on callback server")?;

        let io = TokioIo::new(stream);
        let tx_clone = tx.clone();

        let service = service_fn(move |req: Request<hyper::body::Incoming>| {
            let tx_inner = tx_clone.clone();
            async move {
                let uri = req.uri().to_string();
                debug!("Callback server received request: {}", uri);

                // Parse query parameters from the URI
                let params = parse_callback_params(&uri);

                match params {
                    Some(callback_params) => {
                        // Send the params through the channel
                        if let Some(sender) = tx_inner.lock().await.take() {
                            let _ = sender.send(callback_params);
                        }

                        // Return success page
                        let html = success_html();
                        Ok::<_, hyper::Error>(
                            Response::builder()
                                .status(StatusCode::OK)
                                .header("Content-Type", "text/html; charset=utf-8")
                                .body(Full::new(Bytes::from(html)))
                                .unwrap(),
                        )
                    }
                    None => {
                        // Return error page
                        let html = error_html("Missing authorization code in callback");
                        Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .header("Content-Type", "text/html; charset=utf-8")
                            .body(Full::new(Bytes::from(html)))
                            .unwrap())
                    }
                }
            }
        });

        // Serve the single connection
        tokio::spawn(async move {
            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                warn!("Callback server connection error: {}", e);
            }
        });

        // Wait for the callback parameters
        let params = rx
            .await
            .context("Callback server channel closed without receiving parameters")?;

        info!("Received OAuth callback with authorization code");
        Ok(params)
    }
}

/// Parses the authorization code and state from a callback URI
fn parse_callback_params(uri: &str) -> Option<CallbackParams> {
    let url = url::Url::parse(&format!("http://localhost{}", uri)).ok()?;
    let mut code = None;
    let mut state = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.to_string()),
            "state" => state = Some(value.to_string()),
            _ => {}
        }
    }

    Some(CallbackParams {
        code: code?,
        state: state.unwrap_or_default(),
    })
}

/// Returns the HTML for a successful authentication page
fn success_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head><title>LNXDrive - Authentication Successful</title></head>
<body style="font-family: sans-serif; text-align: center; padding-top: 50px;">
    <h1>Authentication Successful</h1>
    <p>You have been authenticated with OneDrive.</p>
    <p>You can close this window and return to LNXDrive.</p>
    <script>setTimeout(function() { window.close(); }, 3000);</script>
</body>
</html>"#
        .to_string()
}

/// Returns the HTML for an authentication error page
fn error_html(message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><title>LNXDrive - Authentication Error</title></head>
<body style="font-family: sans-serif; text-align: center; padding-top: 50px;">
    <h1>Authentication Error</h1>
    <p>{}</p>
    <p>Please close this window and try again.</p>
</body>
</html>"#,
        message
    )
}

// ============================================================================
// GraphAuthAdapter
// ============================================================================

/// High-level authentication adapter that orchestrates the full OAuth2 PKCE flow.
///
/// Combines [`PKCEFlow`], [`LocalCallbackServer`], and browser launching to
/// provide a complete interactive authentication experience:
///
/// 1. Generates PKCE authorization URL
/// 2. Opens the user's browser to the Microsoft login page
/// 3. Starts a local callback server to receive the redirect
/// 4. Exchanges the authorization code for tokens
/// 5. Returns the OAuth tokens
pub struct GraphAuthAdapter {
    config: OAuth2Config,
}

impl GraphAuthAdapter {
    /// Creates a new GraphAuthAdapter with the given configuration
    pub fn new(config: OAuth2Config) -> Self {
        Self { config }
    }

    /// Creates a new GraphAuthAdapter with just an app ID
    pub fn with_app_id(app_id: impl Into<String>) -> Self {
        Self {
            config: OAuth2Config::new(app_id),
        }
    }

    /// Performs the full interactive OAuth2 PKCE login flow
    ///
    /// This will:
    /// 1. Generate a PKCE-secured authorization URL
    /// 2. Open the user's default browser to Microsoft login
    /// 3. Start a local HTTP server to receive the callback
    /// 4. Exchange the authorization code for tokens
    ///
    /// # Returns
    /// OAuth tokens on successful authentication
    pub async fn login(&self) -> Result<Tokens> {
        info!("Starting OAuth2 PKCE login flow");

        let flow = PKCEFlow::new(&self.config)?;

        // Step 1: Generate authorization URL with PKCE
        let (auth_url, _csrf_token, pkce_verifier) = flow.generate_auth_url();

        // Step 2: Open the browser
        info!("Opening browser for authentication");
        webbrowser::open(&auth_url).context("Failed to open browser for authentication")?;

        // Step 3: Start local callback server and wait for redirect
        let callback = LocalCallbackServer::start().await?;

        // Step 4: Exchange authorization code for tokens
        let tokens = flow.exchange_code(callback.code, pkce_verifier).await?;

        info!("OAuth2 PKCE login completed successfully");
        Ok(tokens)
    }

    /// Refreshes an expired access token
    ///
    /// # Arguments
    /// * `refresh_token` - The refresh token from a previous authentication
    ///
    /// # Returns
    /// New OAuth tokens
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens> {
        let flow = PKCEFlow::new(&self.config)?;
        flow.refresh_token(refresh_token).await
    }

    /// Returns a reference to the current configuration
    pub fn config(&self) -> &OAuth2Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth2_config_defaults() {
        let config = OAuth2Config::new("test-app-id");
        assert_eq!(config.app_id, "test-app-id");
        assert_eq!(config.redirect_uri, REDIRECT_URI);
        assert_eq!(config.scopes.len(), 3);
        assert!(config.scopes.contains(&"Files.ReadWrite.All".to_string()));
        assert!(config.scopes.contains(&"User.Read".to_string()));
        assert!(config.scopes.contains(&"offline_access".to_string()));
    }

    #[test]
    fn test_oauth2_config_custom_scopes() {
        let config = OAuth2Config::new("test-app-id").with_scopes(vec!["Files.Read".to_string()]);
        assert_eq!(config.scopes.len(), 1);
        assert_eq!(config.scopes[0], "Files.Read");
    }

    #[test]
    fn test_oauth2_config_custom_redirect() {
        let config = OAuth2Config::new("test-app-id").with_redirect_uri("http://localhost:9999/cb");
        assert_eq!(config.redirect_uri, "http://localhost:9999/cb");
    }

    #[test]
    fn test_pkce_flow_creation() {
        let config = OAuth2Config::new("test-app-id");
        let flow = PKCEFlow::new(&config);
        assert!(flow.is_ok());
    }

    #[test]
    fn test_pkce_flow_generates_auth_url() {
        let config = OAuth2Config::new("test-app-id");
        let flow = PKCEFlow::new(&config).unwrap();
        let (url, _csrf, _verifier) = flow.generate_auth_url();

        assert!(url.contains("login.microsoftonline.com"));
        assert!(url.contains("test-app-id"));
        assert!(url.contains("code_challenge"));
    }

    #[test]
    fn test_parse_callback_params_valid() {
        let uri = "/callback?code=M.C507_SN1.2.abc123&state=xyz789";
        let params = parse_callback_params(uri);
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.code, "M.C507_SN1.2.abc123");
        assert_eq!(params.state, "xyz789");
    }

    #[test]
    fn test_parse_callback_params_missing_code() {
        let uri = "/callback?state=xyz789";
        let params = parse_callback_params(uri);
        assert!(params.is_none());
    }

    #[test]
    fn test_parse_callback_params_missing_state() {
        let uri = "/callback?code=abc123";
        let params = parse_callback_params(uri);
        assert!(params.is_some());
        let params = params.unwrap();
        assert_eq!(params.code, "abc123");
        assert_eq!(params.state, "");
    }

    #[test]
    fn test_success_html_contains_message() {
        let html = success_html();
        assert!(html.contains("Authentication Successful"));
        assert!(html.contains("LNXDrive"));
    }

    #[test]
    fn test_error_html_contains_message() {
        let html = error_html("test error message");
        assert!(html.contains("test error message"));
        assert!(html.contains("Authentication Error"));
    }

    #[test]
    fn test_graph_auth_adapter_creation() {
        let adapter = GraphAuthAdapter::with_app_id("test-id");
        assert_eq!(adapter.config().app_id, "test-id");
    }
}
