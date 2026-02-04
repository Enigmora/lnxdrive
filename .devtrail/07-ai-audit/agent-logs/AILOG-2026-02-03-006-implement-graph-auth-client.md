---
id: AILOG-2026-02-03-006
title: Implement MS Graph OAuth2 PKCE authentication and API client
status: accepted
created: 2026-02-03
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: medium
tags: [oauth2, pkce, authentication, graph-api, http-client]
related: [T109, T110, T111, T112, T113, T114, T115, T116, T117, T118, T119, T120, T121, T122, T123, T124, T125, T126]
---

# AILOG: Implement MS Graph OAuth2 PKCE Authentication and API Client

## Summary

Implemented the complete OAuth2 Authorization Code with PKCE flow and Microsoft Graph API client in the `lnxdrive-graph` crate. This covers tasks T109-T126, providing the authentication adapter and HTTP client needed for the OneDrive integration.

## Context

LNXDrive requires OAuth2 authentication with Microsoft's identity platform to access OneDrive via the Graph API. The PKCE flow (RFC 7636) is the recommended approach for native desktop applications that cannot securely store a client secret. A typed HTTP client is needed for making authenticated API calls.

## Actions Performed

1. Created `crates/lnxdrive-graph/src/auth.rs` with five components:
   - `OAuth2Config` - Configuration struct for app_id, redirect_uri, and scopes
   - `KeyringTokenStorage` - Secure token storage using the system keyring (store/load/clear)
   - `PKCEFlow` - OAuth2 PKCE flow using the `oauth2` crate (generate_auth_url, exchange_code, refresh_token)
   - `LocalCallbackServer` - Minimal HTTP server on 127.0.0.1:8400 using hyper 1.x for OAuth redirect
   - `GraphAuthAdapter` - High-level adapter orchestrating browser launch, callback server, and code exchange

2. Created `crates/lnxdrive-graph/src/client.rs` with:
   - `GraphClient` - HTTP client wrapping `reqwest` with Bearer auth headers
   - `get_user_info()` - Fetches user profile from /me and quota from /me/drive
   - `get_drive_quota()` - Fetches storage quota (used/total bytes)
   - `request()` helper for building authenticated requests
   - Internal response DTOs for Graph API JSON deserialization

3. Updated `crates/lnxdrive-graph/src/lib.rs` to export auth and client modules

4. Updated `crates/lnxdrive-graph/Cargo.toml` to add required dependencies (lnxdrive-core, serde_json, anyhow, url)

## Modified Files

| File | Change |
|------|--------|
| `crates/lnxdrive-graph/Cargo.toml` | Added lnxdrive-core, serde_json, anyhow, url dependencies |
| `crates/lnxdrive-graph/src/lib.rs` | Added `pub mod auth;` and `pub mod client;` exports, updated doc comments |
| `crates/lnxdrive-graph/src/auth.rs` | Created: OAuth2Config, KeyringTokenStorage, PKCEFlow, LocalCallbackServer, GraphAuthAdapter |
| `crates/lnxdrive-graph/src/client.rs` | Created: GraphClient with get_user_info(), get_drive_quota(), request() |

## Decisions Made

- **PKCE over Device Code**: Used Authorization Code with PKCE as the primary OAuth flow since it provides a better user experience for desktop apps (opens browser, auto-redirects back)
- **hyper 1.x API**: Used hyper 1.x with hyper-util and http-body-util, which required the TokioIo adapter and service_fn pattern (significant API change from hyper 0.14)
- **Keyring storage**: Tokens are serialized as JSON and stored in the OS keyring using the user's email as the username key
- **oauth2 crate 4.4**: Used `request_async(oauth2::reqwest::async_http_client)` for async token exchanges
- **Optional fields in Graph responses**: All Graph API response fields are `Option<T>` to handle partial responses gracefully
- **Refresh token preservation**: When refreshing, if the server doesn't return a new refresh token, the old one is preserved

## Impact

- **Functionality**: Enables OAuth2 PKCE authentication with Microsoft and basic Graph API calls (user info, quota)
- **Performance**: N/A - network I/O bound, standard async patterns
- **Security**: Medium risk level due to OAuth token handling; tokens stored in system keyring (encrypted by OS), never logged at info level

## Verification

- [x] Code compiles without errors (zero warnings)
- [x] 19 unit tests pass, 1 doc-test passes
- [ ] Manual review performed

## Additional Notes

- The `DEFAULT_APP_ID` is intentionally empty; users must register their own Azure AD application
- The callback server accepts a single connection and shuts down, preventing resource leaks
- The `GraphClient` includes a `with_base_url` constructor (test-only) for integration testing with mock servers

---

<!-- Template: DevTrail | https://enigmora.com -->
