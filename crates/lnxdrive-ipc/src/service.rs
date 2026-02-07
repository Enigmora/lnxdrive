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

use lnxdrive_core::ports::state_repository::IStateRepository;
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
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            sync_state: DaemonSyncState::Idle,
            sync_requested: false,
            account_email: None,
            account_display_name: None,
            last_sync_result: None,
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
/// Provides methods to list unresolved conflicts, resolve them using a
/// specified strategy, and query individual conflict details. Uses the
/// real `IStateRepository` for persistent conflict data.
pub struct ConflictsInterface {
    #[allow(dead_code)]
    state: Arc<Mutex<DaemonState>>,
    state_repository: Arc<dyn IStateRepository>,
}

impl ConflictsInterface {
    /// Creates a new ConflictsInterface with state repository
    pub fn new(
        state: Arc<Mutex<DaemonState>>,
        state_repository: Arc<dyn IStateRepository>,
    ) -> Self {
        Self {
            state,
            state_repository,
        }
    }
}

#[zbus::interface(name = "com.enigmora.LNXDrive.Conflicts")]
impl ConflictsInterface {
    /// Returns a JSON array of unresolved conflicts
    ///
    /// Each conflict contains its ID, item_id, detection timestamp, and
    /// version metadata for both local and remote sides.
    async fn list(&self) -> String {
        match self.state_repository.get_unresolved_conflicts().await {
            Ok(conflicts) => {
                let json: Vec<serde_json::Value> = conflicts
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id().to_string(),
                            "item_id": c.item_id().to_string(),
                            "detected_at": c.detected_at().to_rfc3339(),
                            "local_version": {
                                "hash": c.local_version().hash().to_string(),
                                "size_bytes": c.local_version().size_bytes(),
                                "modified_at": c.local_version().modified_at().to_rfc3339(),
                            },
                            "remote_version": {
                                "hash": c.remote_version().hash().to_string(),
                                "size_bytes": c.remote_version().size_bytes(),
                                "modified_at": c.remote_version().modified_at().to_rfc3339(),
                            },
                        })
                    })
                    .collect();
                serde_json::to_string(&json).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                warn!(error = %e, "Failed to query unresolved conflicts");
                "[]".to_string()
            }
        }
    }

    /// Returns detailed JSON for a specific conflict
    ///
    /// # Arguments
    /// * `id` - The conflict's unique identifier
    ///
    /// # Returns
    /// JSON string with conflict details, or empty object if not found
    async fn get_details(&self, id: String) -> String {
        use lnxdrive_core::domain::newtypes::ConflictId;

        let conflict_id = match id.parse::<ConflictId>() {
            Ok(cid) => cid,
            Err(_) => return "{}".to_string(),
        };

        match self.state_repository.get_conflict_by_id(&conflict_id).await {
            Ok(Some(conflict)) => serde_json::json!({
                "id": conflict.id().to_string(),
                "item_id": conflict.item_id().to_string(),
                "detected_at": conflict.detected_at().to_rfc3339(),
                "is_resolved": conflict.is_resolved(),
                "local_version": {
                    "hash": conflict.local_version().hash().to_string(),
                    "size_bytes": conflict.local_version().size_bytes(),
                    "modified_at": conflict.local_version().modified_at().to_rfc3339(),
                    "etag": conflict.local_version().etag(),
                },
                "remote_version": {
                    "hash": conflict.remote_version().hash().to_string(),
                    "size_bytes": conflict.remote_version().size_bytes(),
                    "modified_at": conflict.remote_version().modified_at().to_rfc3339(),
                    "etag": conflict.remote_version().etag(),
                },
            })
            .to_string(),
            Ok(None) => "{}".to_string(),
            Err(e) => {
                warn!(error = %e, conflict_id = %id, "Failed to get conflict details");
                "{}".to_string()
            }
        }
    }

    /// Attempts to resolve a conflict with the given strategy
    ///
    /// # Arguments
    /// * `id` - The conflict's unique identifier
    /// * `strategy` - Resolution strategy: "keep_local", "keep_remote", or "keep_both"
    ///
    /// # Returns
    /// `true` if the conflict was resolved, `false` on error or invalid input
    async fn resolve(&self, id: String, strategy: String) -> bool {
        use lnxdrive_core::domain::{
            conflict::{Resolution, ResolutionSource},
            newtypes::ConflictId,
        };

        let resolution = match strategy.as_str() {
            "keep_local" | "local" => Resolution::KeepLocal,
            "keep_remote" | "remote" => Resolution::KeepRemote,
            "keep_both" | "both" => Resolution::KeepBoth,
            _ => {
                warn!(strategy = %strategy, "Invalid conflict resolution strategy");
                return false;
            }
        };

        let conflict_id = match id.parse::<ConflictId>() {
            Ok(cid) => cid,
            Err(_) => {
                warn!(id = %id, "Invalid conflict ID");
                return false;
            }
        };

        // Find the conflict
        let conflict = match self.state_repository.get_conflict_by_id(&conflict_id).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                warn!(id = %id, "Conflict not found");
                return false;
            }
            Err(e) => {
                warn!(error = %e, "Failed to query conflict");
                return false;
            }
        };

        if conflict.is_resolved() {
            warn!(id = %id, "Conflict already resolved");
            return false;
        }

        info!(
            conflict_id = %id,
            strategy = %strategy,
            "Resolving conflict via D-Bus"
        );

        // Mark as resolved in the database
        let resolved = conflict.resolve(resolution, ResolutionSource::User);
        match self.state_repository.save_conflict(&resolved).await {
            Ok(()) => {
                info!(conflict_id = %id, "Conflict resolved successfully");
                true
            }
            Err(e) => {
                warn!(error = %e, "Failed to save resolved conflict");
                false
            }
        }
    }

    /// Resolve all unresolved conflicts with the same strategy
    ///
    /// # Arguments
    /// * `strategy` - Resolution strategy: "keep_local", "keep_remote", or "keep_both"
    ///
    /// # Returns
    /// Number of conflicts resolved
    async fn resolve_all(&self, strategy: String) -> u32 {
        use lnxdrive_core::domain::conflict::{Resolution, ResolutionSource};

        let resolution = match strategy.as_str() {
            "keep_local" | "local" => Resolution::KeepLocal,
            "keep_remote" | "remote" => Resolution::KeepRemote,
            "keep_both" | "both" => Resolution::KeepBoth,
            _ => {
                warn!(strategy = %strategy, "Invalid strategy for resolve_all");
                return 0;
            }
        };

        let conflicts = match self.state_repository.get_unresolved_conflicts().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to query conflicts for batch resolve");
                return 0;
            }
        };

        let mut resolved_count = 0u32;
        for conflict in conflicts {
            let resolved = conflict.resolve(resolution.clone(), ResolutionSource::User);
            if self.state_repository.save_conflict(&resolved).await.is_ok() {
                resolved_count += 1;
            }
        }

        info!(count = resolved_count, strategy = %strategy, "Batch resolve completed");
        resolved_count
    }

    // D-Bus Signals

    /// Emitted when a new conflict is detected
    #[zbus(signal)]
    async fn conflict_detected(
        signal_ctxt: &zbus::SignalContext<'_>,
        conflict_json: &str,
    ) -> zbus::Result<()>;

    /// Emitted when a conflict is resolved
    #[zbus(signal)]
    async fn conflict_resolved(
        signal_ctxt: &zbus::SignalContext<'_>,
        conflict_id: &str,
        strategy: &str,
    ) -> zbus::Result<()>;
}

// ============================================================================
// T4-049: ObservabilityInterface
// ============================================================================

/// D-Bus interface for observability data
///
/// Provides methods to query audit trail entries and Prometheus metrics
/// from D-Bus clients (CLI, GNOME extension, etc.).
pub struct ObservabilityInterface {
    state_repository: Arc<dyn IStateRepository>,
    metrics: Option<Arc<lnxdrive_telemetry::MetricsRegistry>>,
}

impl ObservabilityInterface {
    /// Creates a new ObservabilityInterface
    pub fn new(
        state_repository: Arc<dyn IStateRepository>,
        metrics: Option<Arc<lnxdrive_telemetry::MetricsRegistry>>,
    ) -> Self {
        Self {
            state_repository,
            metrics,
        }
    }
}

#[zbus::interface(name = "com.enigmora.LNXDrive.Observability")]
impl ObservabilityInterface {
    /// Returns recent audit trail entries as a JSON array
    ///
    /// # Arguments
    /// * `since_hours` - How many hours back to query (0 = last 24h)
    /// * `limit` - Maximum number of entries to return
    async fn get_audit_trail(&self, since_hours: u32, limit: u32) -> String {
        let hours = if since_hours == 0 { 24 } else { since_hours };
        let since = chrono::Utc::now()
            - chrono::Duration::hours(hours as i64);
        let limit = if limit == 0 { 50 } else { limit };

        match self
            .state_repository
            .get_audit_since(since, limit)
            .await
        {
            Ok(entries) => {
                let json: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "timestamp": e.timestamp().to_rfc3339(),
                            "action": e.action().to_string(),
                            "result": serde_json::to_value(e.result()).unwrap_or_default(),
                            "details": e.details(),
                            "duration_ms": e.duration_ms(),
                        })
                    })
                    .collect();
                serde_json::to_string(&json).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                warn!(error = %e, "Failed to query audit trail via D-Bus");
                "[]".to_string()
            }
        }
    }

    /// Returns current Prometheus metrics in text exposition format
    async fn get_metrics(&self) -> String {
        match &self.metrics {
            Some(metrics) => metrics.encode().unwrap_or_else(|e| {
                warn!(error = %e, "Failed to encode metrics");
                String::new()
            }),
            None => "# Metrics not enabled\n".to_string(),
        }
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
    state_repository: Option<Arc<dyn IStateRepository>>,
    metrics: Option<Arc<lnxdrive_telemetry::MetricsRegistry>>,
}

impl DbusService {
    /// Creates a new DbusService with the given shared state and state repository
    pub fn new(
        state: Arc<Mutex<DaemonState>>,
        state_repository: Arc<dyn IStateRepository>,
    ) -> Self {
        Self {
            state,
            state_repository: Some(state_repository),
            metrics: None,
        }
    }

    /// Sets the Prometheus metrics registry for the Observability D-Bus interface
    pub fn with_metrics(mut self, metrics: Arc<lnxdrive_telemetry::MetricsRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Creates a new DbusService with default state (no repository — for testing)
    pub fn with_default_state() -> Self {
        Self {
            state: Arc::new(Mutex::new(DaemonState::default())),
            state_repository: None,
            metrics: None,
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
    /// - No state repository is configured
    pub async fn start(&self) -> anyhow::Result<zbus::Connection> {
        info!("Starting D-Bus service on session bus");

        let state_repo = self
            .state_repository
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DbusService requires a state_repository to start"))?
            .clone();

        let sync_controller = SyncControllerInterface::new(Arc::clone(&self.state));
        let account_iface = AccountInterface::new(Arc::clone(&self.state));
        let conflicts_iface =
            ConflictsInterface::new(Arc::clone(&self.state), Arc::clone(&state_repo));
        let observability_iface =
            ObservabilityInterface::new(state_repo, self.metrics.clone());

        let connection = zbus::connection::Builder::session()?
            .name(DBUS_NAME)?
            .serve_at(DBUS_PATH, sync_controller)?
            .serve_at(DBUS_PATH, account_iface)?
            .serve_at(DBUS_PATH, conflicts_iface)?
            .serve_at(DBUS_PATH, observability_iface)?
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

    async fn make_test_repo() -> Arc<lnxdrive_cache::SqliteStateRepository> {
        let pool = lnxdrive_cache::pool::DatabasePool::in_memory()
            .await
            .expect("Failed to create in-memory database");
        Arc::new(lnxdrive_cache::SqliteStateRepository::new(
            pool.pool().clone(),
        ))
    }

    #[tokio::test]
    async fn test_conflicts_list_empty() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let repo = make_test_repo().await;
        let conflicts = ConflictsInterface::new(state, repo);

        let result = conflicts.list().await;
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_conflicts_resolve_invalid_strategy() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let repo = make_test_repo().await;
        let conflicts = ConflictsInterface::new(state, repo);

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

    #[tokio::test]
    async fn test_conflicts_resolve_not_found() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let repo = make_test_repo().await;
        let conflicts = ConflictsInterface::new(state, repo);

        // Valid strategy but non-existent conflict
        let fake_id = lnxdrive_core::domain::newtypes::ConflictId::new().to_string();
        assert!(
            !conflicts
                .resolve(fake_id, "keep_local".to_string())
                .await
        );
    }

    #[tokio::test]
    async fn test_conflicts_resolve_all_empty() {
        let state = Arc::new(Mutex::new(DaemonState::default()));
        let repo = make_test_repo().await;
        let conflicts = ConflictsInterface::new(state, repo);

        let count = conflicts.resolve_all("keep_local".to_string()).await;
        assert_eq!(count, 0);
    }

    #[test]
    fn test_dbus_service_with_default_state() {
        let service = DbusService::with_default_state();
        // Just verify it constructs without panic
        let _state = service.state();
    }

    #[tokio::test]
    async fn test_dbus_service_with_custom_state() {
        let state = Arc::new(Mutex::new(DaemonState {
            account_email: Some("user@test.com".to_string()),
            ..DaemonState::default()
        }));
        let repo = make_test_repo().await;
        let service = DbusService::new(state, repo);
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

    // -- ObservabilityInterface tests --

    #[tokio::test]
    async fn test_observability_get_audit_trail_empty() {
        let repo = make_test_repo().await;
        let obs = ObservabilityInterface::new(repo, None);

        let result = obs.get_audit_trail(24, 50).await;
        assert_eq!(result, "[]");
    }

    #[tokio::test]
    async fn test_observability_get_metrics_disabled() {
        let repo = make_test_repo().await;
        let obs = ObservabilityInterface::new(repo, None);

        let result = obs.get_metrics().await;
        assert!(result.contains("not enabled"));
    }

    #[tokio::test]
    async fn test_observability_get_metrics_enabled() {
        let repo = make_test_repo().await;
        let metrics = Arc::new(lnxdrive_telemetry::MetricsRegistry::new().unwrap());
        metrics.record_sync_operation("download", "success");

        let obs = ObservabilityInterface::new(repo, Some(metrics));

        let result = obs.get_metrics().await;
        assert!(result.contains("lnxdrive_sync_operations_total"));
    }
}
