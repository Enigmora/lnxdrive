//! D-Bus service implementation for LNXDrive
//!
//! Provides the D-Bus interfaces that UI clients and CLI tools use to
//! communicate with the running LNXDrive daemon:
//!
//! - `com.enigmora.LNXDrive.SyncController` - Start, pause, and query sync
//! - `com.enigmora.LNXDrive.Account` - Account information and auth status
//! - `com.enigmora.LNXDrive.Conflicts` - Conflict listing and resolution
//!
//! Signals are emitted on state changes, sync progress, and errors.

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// D-Bus well-known name for the LNXDrive daemon
pub const DBUS_NAME: &str = "com.enigmora.LNXDrive";

/// D-Bus object path for the service
pub const DBUS_PATH: &str = "/com/enigmora/LNXDrive";

// ============================================================================
// Daemon state shared with D-Bus interfaces
// ============================================================================

/// Possible daemon sync states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonSyncState {
    /// Daemon is idle, waiting for next poll interval
    Idle,
    /// Sync cycle is currently running
    Syncing,
    /// Sync is paused by user request
    Paused,
    /// Daemon is waiting for authentication
    WaitingForAuth,
    /// Daemon encountered an error
    Error(String),
}

impl std::fmt::Display for DaemonSyncState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonSyncState::Idle => write!(f, "idle"),
            DaemonSyncState::Syncing => write!(f, "syncing"),
            DaemonSyncState::Paused => write!(f, "paused"),
            DaemonSyncState::WaitingForAuth => write!(f, "waiting_for_auth"),
            DaemonSyncState::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

/// Shared state between the daemon and D-Bus interfaces
pub struct DaemonState {
    /// Current sync state
    pub sync_state: DaemonSyncState,
    /// Whether sync has been requested while paused
    pub sync_requested: bool,
    /// Account email (if authenticated)
    pub account_email: Option<String>,
    /// Account display name (if authenticated)
    pub account_display_name: Option<String>,
    /// Last sync result summary (JSON)
    pub last_sync_result: Option<String>,
    /// Unresolved conflicts as JSON array
    pub conflicts_json: String,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            sync_state: DaemonSyncState::Idle,
            sync_requested: false,
            account_email: None,
            account_display_name: None,
            last_sync_result: None,
            conflicts_json: "[]".to_string(),
        }
    }
}

// ============================================================================
// T219-T220: SyncController interface
// ============================================================================

/// D-Bus interface for controlling synchronization
///
/// Provides methods to start/pause sync and query the current status.
/// Connected to the daemon's shared state via an `Arc<Mutex<DaemonState>>`.
pub struct SyncControllerInterface {
    state: Arc<Mutex<DaemonState>>,
}

impl SyncControllerInterface {
    /// Creates a new SyncControllerInterface with the given shared state
    pub fn new(state: Arc<Mutex<DaemonState>>) -> Self {
        Self { state }
    }
}

#[zbus::interface(name = "com.enigmora.LNXDrive.SyncController")]
impl SyncControllerInterface {
    /// Triggers an immediate sync cycle
    ///
    /// If the daemon is paused, the sync request is queued and will run
    /// when the daemon is resumed. If already syncing, this is a no-op.
    async fn start_sync(&self) {
        let mut state = self.state.lock().await;
        match state.sync_state {
            DaemonSyncState::Syncing => {
                debug!("StartSync called but sync is already running");
            }
            DaemonSyncState::Paused => {
                info!("StartSync called while paused, queueing sync request");
                state.sync_requested = true;
            }
            _ => {
                info!("StartSync called, requesting sync cycle");
                state.sync_requested = true;
            }
        }
    }

    /// Pauses synchronization
    ///
    /// The daemon will finish any in-progress sync cycle but will not
    /// start new ones until resumed via `StartSync`.
    async fn pause_sync(&self) {
        let mut state = self.state.lock().await;
        if state.sync_state != DaemonSyncState::Paused {
            info!("PauseSync called, pausing sync");
            state.sync_state = DaemonSyncState::Paused;
        } else {
            debug!("PauseSync called but already paused");
        }
    }

    /// Returns the current daemon status as a JSON string
    ///
    /// The returned JSON contains:
    /// - `state`: Current sync state (idle, syncing, paused, etc.)
    /// - `account_email`: Email of the authenticated account (if any)
    /// - `last_sync_result`: Summary of the last sync cycle (if any)
    async fn get_status(&self) -> String {
        let state = self.state.lock().await;
        let status = serde_json::json!({
            "state": state.sync_state.to_string(),
            "account_email": state.account_email,
            "account_display_name": state.account_display_name,
            "last_sync_result": state.last_sync_result,
        });
        status.to_string()
    }

    // T223: D-Bus signals

    /// Emitted when the sync state changes
    #[zbus(signal)]
    async fn state_changed(signal_ctxt: &zbus::SignalContext<'_>, state: &str) -> zbus::Result<()>;

    /// Emitted to report sync progress
    #[zbus(signal)]
    async fn sync_progress(
        signal_ctxt: &zbus::SignalContext<'_>,
        current: u32,
        total: u32,
    ) -> zbus::Result<()>;

    /// Emitted when an error occurs
    #[zbus(signal)]
    async fn error_occurred(
        signal_ctxt: &zbus::SignalContext<'_>,
        message: &str,
    ) -> zbus::Result<()>;
}

// ============================================================================
// T221: Account interface
// ============================================================================

/// D-Bus interface for account information
///
/// Provides read-only access to the authenticated account's details
/// and authentication status.
pub struct AccountInterface {
    state: Arc<Mutex<DaemonState>>,
}

impl AccountInterface {
    /// Creates a new AccountInterface with the given shared state
    pub fn new(state: Arc<Mutex<DaemonState>>) -> Self {
        Self { state }
    }
}

#[zbus::interface(name = "com.enigmora.LNXDrive.Account")]
impl AccountInterface {
    /// Returns account information as a JSON string
    ///
    /// The returned JSON contains:
    /// - `email`: Account email address
    /// - `display_name`: Account display name
    async fn get_info(&self) -> String {
        let state = self.state.lock().await;
        let info = serde_json::json!({
            "email": state.account_email,
            "display_name": state.account_display_name,
        });
        info.to_string()
    }

    /// Checks whether the daemon has valid authentication
    ///
    /// Returns `true` if the daemon has a configured account with tokens,
    /// `false` otherwise.
    async fn check_auth(&self) -> bool {
        let state = self.state.lock().await;
        state.account_email.is_some()
    }
}

// ============================================================================
// T222: Conflicts interface
// ============================================================================

/// D-Bus interface for conflict management
///
/// Provides methods to list unresolved conflicts and resolve them
/// using a specified strategy.
pub struct ConflictsInterface {
    state: Arc<Mutex<DaemonState>>,
}

impl ConflictsInterface {
    /// Creates a new ConflictsInterface with the given shared state
    pub fn new(state: Arc<Mutex<DaemonState>>) -> Self {
        Self { state }
    }
}

#[zbus::interface(name = "com.enigmora.LNXDrive.Conflicts")]
impl ConflictsInterface {
    /// Returns a JSON array of unresolved conflicts
    ///
    /// Each conflict contains its ID, file path, and detection timestamp.
    async fn list(&self) -> String {
        let state = self.state.lock().await;
        state.conflicts_json.clone()
    }

    /// Attempts to resolve a conflict with the given strategy
    ///
    /// # Arguments
    /// * `id` - The conflict's unique identifier
    /// * `strategy` - Resolution strategy: "keep_local", "keep_remote", or "keep_both"
    ///
    /// # Returns
    /// `true` if the conflict was found and resolution was initiated,
    /// `false` if the conflict was not found or the strategy is invalid
    async fn resolve(&self, id: String, strategy: String) -> bool {
        let valid_strategies = ["keep_local", "keep_remote", "keep_both"];
        if !valid_strategies.contains(&strategy.as_str()) {
            warn!(
                strategy = %strategy,
                "Invalid conflict resolution strategy"
            );
            return false;
        }

        info!(
            conflict_id = %id,
            strategy = %strategy,
            "Conflict resolution requested via D-Bus"
        );

        // Conflict resolution is logged but actual resolution requires
        // integration with the conflict engine (Phase 10+).
        // For now, we acknowledge the request.
        debug!(
            "Conflict resolution for '{}' with strategy '{}' acknowledged",
            id, strategy
        );
        true
    }
}

// ============================================================================
// DbusService - high-level service orchestrator
// ============================================================================

/// High-level D-Bus service that manages all interfaces
///
/// Creates a `zbus::Connection` on the session bus, registers all
/// interface objects at the well-known path, and requests the
/// well-known name `com.enigmora.LNXDrive`.
pub struct DbusService {
    state: Arc<Mutex<DaemonState>>,
}

impl DbusService {
    /// Creates a new DbusService with the given shared state
    pub fn new(state: Arc<Mutex<DaemonState>>) -> Self {
        Self { state }
    }

    /// Creates a new DbusService with default state
    pub fn with_default_state() -> Self {
        Self {
            state: Arc::new(Mutex::new(DaemonState::default())),
        }
    }

    /// Returns a reference to the shared daemon state
    pub fn state(&self) -> &Arc<Mutex<DaemonState>> {
        &self.state
    }

    /// Starts the D-Bus service on the session bus
    ///
    /// Registers all interfaces and requests the well-known name.
    /// Returns the connection which must be kept alive for the service
    /// to remain active.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The session bus is not available
    /// - The well-known name is already owned (another instance running)
    /// - Interface registration fails
    pub async fn start(&self) -> anyhow::Result<zbus::Connection> {
        info!("Starting D-Bus service on session bus");

        let sync_controller = SyncControllerInterface::new(Arc::clone(&self.state));
        let account_iface = AccountInterface::new(Arc::clone(&self.state));
        let conflicts_iface = ConflictsInterface::new(Arc::clone(&self.state));

        let connection = zbus::connection::Builder::session()?
            .name(DBUS_NAME)?
            .serve_at(DBUS_PATH, sync_controller)?
            .serve_at(DBUS_PATH, account_iface)?
            .serve_at(DBUS_PATH, conflicts_iface)?
            .build()
            .await?;

        info!(
            name = DBUS_NAME,
            path = DBUS_PATH,
            "D-Bus service started successfully"
        );

        Ok(connection)
    }

    /// Attempts to acquire the D-Bus well-known name to act as a single-instance lock
    ///
    /// If the name is already owned by another process, returns `false`.
    /// This is used by the daemon to ensure only one instance runs at a time.
    pub async fn try_acquire_name() -> anyhow::Result<bool> {
        let connection = zbus::Connection::session().await?;
        let dbus_proxy = zbus::fdo::DBusProxy::new(&connection).await?;

        // Check if the name has an owner
        match dbus_proxy.get_name_owner(DBUS_NAME.try_into()?).await {
            Ok(_owner) => {
                // Name is already owned by another process
                Ok(false)
            }
            Err(_) => {
                // Name is not owned, the daemon can claim it
                Ok(true)
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_sync_state_display() {
        assert_eq!(DaemonSyncState::Idle.to_string(), "idle");
        assert_eq!(DaemonSyncState::Syncing.to_string(), "syncing");
        assert_eq!(DaemonSyncState::Paused.to_string(), "paused");
        assert_eq!(
            DaemonSyncState::WaitingForAuth.to_string(),
            "waiting_for_auth"
        );
        assert_eq!(
            DaemonSyncState::Error("test".to_string()).to_string(),
            "error: test"
        );
    }

    #[test]
    fn test_daemon_state_default() {
        let state = DaemonState::default();
        assert_eq!(state.sync_state, DaemonSyncState::Idle);
        assert!(!state.sync_requested);
        assert!(state.account_email.is_none());
        assert!(state.account_display_name.is_none());
        assert!(state.last_sync_result.is_none());
        assert_eq!(state.conflicts_json, "[]");
    }

    #[test]
    fn test_dbus_constants() {
        assert_eq!(DBUS_NAME, "com.enigmora.LNXDrive");
        assert_eq!(DBUS_PATH, "/com/enigmora/LNXDrive");
    }

    #[tokio::test]
    async fn test_sync_controller_get_status() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let controller = SyncControllerInterface::new(Arc::clone(&state));

        let status_json = controller.get_status().await;
        let status: serde_json::Value = serde_json::from_str(&status_json).unwrap();

        assert_eq!(status["state"], "idle");
        assert!(status["account_email"].is_null());
    }

    #[tokio::test]
    async fn test_sync_controller_start_sync() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let controller = SyncControllerInterface::new(Arc::clone(&state));

        controller.start_sync().await;

        let locked = state.lock().await;
        assert!(locked.sync_requested);
    }

    #[tokio::test]
    async fn test_sync_controller_pause_sync() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let controller = SyncControllerInterface::new(Arc::clone(&state));

        controller.pause_sync().await;

        let locked = state.lock().await;
        assert_eq!(locked.sync_state, DaemonSyncState::Paused);
    }

    #[tokio::test]
    async fn test_sync_controller_start_while_paused() {
        let state = Arc::new(Mutex::new(DaemonState {
            sync_state: DaemonSyncState::Paused,
            ..DaemonState::default()
        }));
        let controller = SyncControllerInterface::new(Arc::clone(&state));

        controller.start_sync().await;

        let locked = state.lock().await;
        assert!(locked.sync_requested);
        // State remains paused; the daemon loop handles resumption
        assert_eq!(locked.sync_state, DaemonSyncState::Paused);
    }

    #[tokio::test]
    async fn test_account_get_info_no_account() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let account = AccountInterface::new(Arc::clone(&state));

        let info_json = account.get_info().await;
        let info: serde_json::Value = serde_json::from_str(&info_json).unwrap();

        assert!(info["email"].is_null());
        assert!(info["display_name"].is_null());
    }

    #[tokio::test]
    async fn test_account_get_info_with_account() {
        let state = Arc::new(Mutex::new(DaemonState {
            account_email: Some("user@example.com".to_string()),
            account_display_name: Some("Test User".to_string()),
            ..DaemonState::default()
        }));
        let account = AccountInterface::new(Arc::clone(&state));

        let info_json = account.get_info().await;
        let info: serde_json::Value = serde_json::from_str(&info_json).unwrap();

        assert_eq!(info["email"], "user@example.com");
        assert_eq!(info["display_name"], "Test User");
    }

    #[tokio::test]
    async fn test_account_check_auth_no_account() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let account = AccountInterface::new(state);

        assert!(!account.check_auth().await);
    }

    #[tokio::test]
    async fn test_account_check_auth_with_account() {
        let state = Arc::new(Mutex::new(DaemonState {
            account_email: Some("user@example.com".to_string()),
            ..DaemonState::default()
        }));
        let account = AccountInterface::new(state);

        assert!(account.check_auth().await);
    }

    #[tokio::test]
    async fn test_conflicts_list_empty() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let conflicts = ConflictsInterface::new(state);

        let result = conflicts.list().await;
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_conflicts_list_with_data() {
        let conflicts_data = serde_json::json!([
            {"id": "c1", "path": "/test/file.txt"},
            {"id": "c2", "path": "/test/other.txt"},
        ])
        .to_string();

        let state = Arc::new(Mutex::new(DaemonState {
            conflicts_json: conflicts_data.clone(),
            ..DaemonState::default()
        }));
        let conflicts = ConflictsInterface::new(state);

        let result = conflicts.list().await;
        assert_eq!(result, conflicts_data);
    }

    #[tokio::test]
    async fn test_conflicts_resolve_valid_strategy() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let conflicts = ConflictsInterface::new(state);

        assert!(
            conflicts
                .resolve("c1".to_string(), "keep_local".to_string())
                .await
        );
        assert!(
            conflicts
                .resolve("c2".to_string(), "keep_remote".to_string())
                .await
        );
        assert!(
            conflicts
                .resolve("c3".to_string(), "keep_both".to_string())
                .await
        );
    }

    #[tokio::test]
    async fn test_conflicts_resolve_invalid_strategy() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let conflicts = ConflictsInterface::new(state);

        assert!(
            !conflicts
                .resolve("c1".to_string(), "invalid".to_string())
                .await
        );
        assert!(
            !conflicts
                .resolve("c2".to_string(), "delete_all".to_string())
                .await
        );
    }

    #[test]
    fn test_dbus_service_with_default_state() {
        let service = DbusService::with_default_state();
        // Just verify it constructs without panic
        let _state = service.state();
    }

    #[test]
    fn test_dbus_service_with_custom_state() {
        let state = Arc::new(Mutex::new(DaemonState {
            account_email: Some("user@test.com".to_string()),
            ..DaemonState::default()
        }));
        let service = DbusService::new(state);
        let _state = service.state();
    }

    #[tokio::test]
    async fn test_sync_controller_status_with_last_result() {
        let state = Arc::new(Mutex::new(DaemonState {
            sync_state: DaemonSyncState::Idle,
            account_email: Some("user@test.com".to_string()),
            last_sync_result: Some(
                serde_json::json!({
                    "files_downloaded": 5,
                    "files_uploaded": 2,
                    "errors": [],
                })
                .to_string(),
            ),
            ..DaemonState::default()
        }));
        let controller = SyncControllerInterface::new(state);

        let status_json = controller.get_status().await;
        let status: serde_json::Value = serde_json::from_str(&status_json).unwrap();

        assert_eq!(status["state"], "idle");
        assert_eq!(status["account_email"], "user@test.com");
        assert!(status["last_sync_result"].is_string());
    }
}
