//! LNXDrive Daemon - Background synchronization service
//!
//! This binary runs as a systemd user service and handles:
//! - File synchronization with OneDrive
//! - D-Bus interface for UI clients
//! - Periodic remote polling
//! - Graceful shutdown on SIGTERM/SIGINT
//!
//! # Architecture
//!
//! The daemon starts a D-Bus service for IPC, then enters a main loop
//! that periodically runs the SyncEngine. The loop is controlled by a
//! `CancellationToken` that is triggered on receipt of SIGTERM or SIGINT.

use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use lnxdrive_cache::{pool::DatabasePool, SqliteStateRepository};
use lnxdrive_core::{config::Config, ports::state_repository::IStateRepository};
use lnxdrive_fuse::{mount, unmount, BackgroundSession};
use lnxdrive_graph::{
    auth::KeyringTokenStorage, client::GraphClient, provider::GraphCloudProvider,
};
use lnxdrive_ipc::service::{DaemonState, DaemonSyncState, DbusService, DBUS_NAME};
use lnxdrive_sync::{engine::SyncEngine, filesystem::LocalFileSystemAdapter};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

// ============================================================================
// T214: DaemonService struct
// ============================================================================

/// Main daemon service that orchestrates synchronization and IPC
///
/// Holds the configuration, state repository, shared daemon state,
/// and a cancellation token for graceful shutdown.
struct DaemonService {
    /// Application configuration loaded from YAML
    config: Config,
    /// SQLite state repository for sync state persistence
    state_repo: Arc<SqliteStateRepository>,
    /// Database pool (needed for FUSE mount)
    db_pool: DatabasePool,
    /// Shared state between daemon and D-Bus interfaces
    daemon_state: Arc<Mutex<DaemonState>>,
    /// Token for signalling graceful shutdown to all async tasks
    shutdown: CancellationToken,
    /// T095: FUSE session handle (when auto-mounted)
    fuse_session: std::sync::Mutex<Option<BackgroundSession>>,
}

impl DaemonService {
    /// Creates a new DaemonService
    ///
    /// Loads configuration, opens the database, and initializes shared state.
    async fn new(shutdown: CancellationToken) -> Result<Self> {
        // Load configuration
        let config_path = Config::default_path();
        let config = Config::load_or_default(&config_path);
        info!(config_path = %config_path.display(), "Loaded configuration");

        // Open database
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("lnxdrive")
            .join("lnxdrive.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db_pool = DatabasePool::new(Path::new(&db_path))
            .await
            .context("Failed to open database")?;
        let state_repo = Arc::new(SqliteStateRepository::new(db_pool.pool().clone()));

        let daemon_state = Arc::new(Mutex::new(DaemonState::default()));

        Ok(Self {
            config,
            state_repo,
            db_pool,
            daemon_state,
            shutdown,
            fuse_session: std::sync::Mutex::new(None),
        })
    }

    // ========================================================================
    // T215: DaemonService::run() - async main loop
    // ========================================================================

    /// Runs the daemon's main loop
    ///
    /// 1. Checks for an authenticated account
    /// 2. Starts the D-Bus service
    /// 3. Creates adapters and SyncEngine
    /// 4. Enters the polling loop with graceful shutdown support
    async fn run(&self) -> Result<()> {
        // T231: Single instance lock via D-Bus name
        info!("Checking for existing daemon instance...");

        // T224: Start D-Bus service (this also acquires the well-known name)
        let dbus_service = DbusService::new(Arc::clone(&self.daemon_state));
        let _dbus_connection = match dbus_service.start().await {
            Ok(conn) => {
                info!("D-Bus service started, acquired name {}", DBUS_NAME);
                conn
            }
            Err(e) => {
                let err_str = format!("{e:#}");
                if err_str.contains("already taken")
                    || err_str.contains("already owned")
                    || err_str.contains("NameTaken")
                    || err_str.contains("name already")
                {
                    error!(
                        "Another instance of lnxdrived is already running (D-Bus name {} is taken)",
                        DBUS_NAME
                    );
                    anyhow::bail!(
                        "Another instance of lnxdrived is already running. \
                         Use 'lnxdrive daemon stop' to stop it first."
                    );
                }
                return Err(e).context("Failed to start D-Bus service");
            }
        };

        // Try to load account and tokens
        let account_opt = self
            .state_repo
            .get_default_account()
            .await
            .context("Failed to query default account")?;

        let (_account, tokens) = match account_opt {
            Some(account) => {
                match KeyringTokenStorage::load(account.email().as_str()) {
                    Ok(Some(t)) => {
                        info!(
                            email = %account.email(),
                            "Found account with stored tokens"
                        );

                        // Update daemon state with account info
                        {
                            let mut state = self.daemon_state.lock().await;
                            state.account_email = Some(account.email().as_str().to_string());
                            state.account_display_name = Some(account.display_name().to_string());
                        }

                        (account, t)
                    }
                    Ok(None) => {
                        warn!(
                            email = %account.email(),
                            "Account found but no tokens in keyring. \
                             Run 'lnxdrive auth login' to authenticate."
                        );
                        return self.wait_for_auth_loop().await;
                    }
                    Err(e) => {
                        warn!(
                            email = %account.email(),
                            error = %e,
                            "Failed to load tokens from keyring"
                        );
                        return self.wait_for_auth_loop().await;
                    }
                }
            }
            None => {
                warn!("No account configured. Run 'lnxdrive auth login' to set up an account.");
                return self.wait_for_auth_loop().await;
            }
        };

        // Create adapters
        let graph_client = GraphClient::new(&tokens.access_token);
        let cloud_provider = Arc::new(GraphCloudProvider::new(graph_client));
        let local_fs = Arc::new(LocalFileSystemAdapter::new());

        // Create SyncEngine
        let engine = SyncEngine::new(
            cloud_provider,
            Arc::clone(&self.state_repo) as Arc<dyn IStateRepository + Send + Sync>,
            local_fs,
            &self.config,
        );

        // T095: Auto-mount FUSE filesystem if enabled
        if self.config.fuse.auto_mount {
            self.mount_fuse();
        }

        // T216: Enter periodic polling loop
        let result = self.sync_loop(&engine).await;

        // T095: Unmount FUSE on shutdown
        self.unmount_fuse();

        result
    }

    // ========================================================================
    // T095: FUSE Auto-Mount
    // ========================================================================

    /// Mounts the FUSE filesystem if auto_mount is enabled.
    ///
    /// Clones the database pool for the FUSE layer and mounts
    /// the filesystem at the configured mount point. The session handle
    /// is stored for graceful unmount during shutdown.
    fn mount_fuse(&self) {
        info!(
            mount_point = %self.config.fuse.mount_point,
            "Auto-mounting FUSE filesystem"
        );

        // Clone the database pool for FUSE (pool is thread-safe)
        let fuse_pool = self.db_pool.clone();

        let rt_handle = tokio::runtime::Handle::current();

        match mount(self.config.fuse.clone(), fuse_pool, rt_handle) {
            Ok(session) => {
                info!(
                    mount_point = %self.config.fuse.mount_point,
                    "FUSE filesystem mounted successfully"
                );
                if let Ok(mut guard) = self.fuse_session.lock() {
                    *guard = Some(session);
                }
            }
            Err(e) => {
                error!(
                    mount_point = %self.config.fuse.mount_point,
                    error = %e,
                    "Failed to mount FUSE filesystem"
                );
            }
        }
    }

    /// Unmounts the FUSE filesystem if it was auto-mounted.
    ///
    /// Takes ownership of the session handle and drops it, triggering
    /// the kernel unmount operation.
    fn unmount_fuse(&self) {
        if let Ok(mut guard) = self.fuse_session.lock() {
            if let Some(session) = guard.take() {
                info!(
                    mount_point = %self.config.fuse.mount_point,
                    "Unmounting FUSE filesystem"
                );
                unmount(session);
                info!("FUSE filesystem unmounted");
            }
        }
    }

    // ========================================================================
    // T216: Periodic remote polling
    // ========================================================================

    /// Main synchronization loop with periodic polling
    ///
    /// Uses `tokio::time::interval` based on `config.sync.poll_interval`
    /// (defaults to 30 seconds). Each tick runs `engine.sync()` unless
    /// the daemon is paused or shutting down.
    async fn sync_loop(&self, engine: &SyncEngine) -> Result<()> {
        let poll_secs = self.config.sync.poll_interval;
        let poll_duration = Duration::from_secs(poll_secs);

        info!(poll_interval_secs = poll_secs, "Starting sync loop");

        let mut interval = tokio::time::interval(poll_duration);
        // The first tick fires immediately; we want to sync right away
        interval.tick().await;

        loop {
            // Check if a sync was requested via D-Bus
            let sync_requested = {
                let mut state = self.daemon_state.lock().await;
                let requested = state.sync_requested;
                state.sync_requested = false;
                requested
            };

            // Check if paused
            let is_paused = {
                let state = self.daemon_state.lock().await;
                state.sync_state == DaemonSyncState::Paused
            };

            if is_paused && !sync_requested {
                // Wait for either the next interval tick or shutdown
                tokio::select! {
                    _ = interval.tick() => continue,
                    _ = self.shutdown.cancelled() => {
                        info!("Shutdown signal received while paused");
                        break;
                    }
                }
            }

            // If a sync was requested while paused, resume
            if is_paused && sync_requested {
                let mut state = self.daemon_state.lock().await;
                state.sync_state = DaemonSyncState::Idle;
                info!("Resuming sync (requested via D-Bus)");
            }

            // Run a sync cycle
            {
                let mut state = self.daemon_state.lock().await;
                state.sync_state = DaemonSyncState::Syncing;
            }

            info!("Starting sync cycle");

            match engine.sync().await {
                Ok(result) => {
                    let result_json = serde_json::json!({
                        "files_downloaded": result.files_downloaded,
                        "files_uploaded": result.files_uploaded,
                        "files_deleted": result.files_deleted,
                        "errors": result.errors,
                        "duration_ms": result.duration_ms,
                    })
                    .to_string();

                    info!(
                        downloaded = result.files_downloaded,
                        uploaded = result.files_uploaded,
                        deleted = result.files_deleted,
                        errors = result.errors.len(),
                        duration_ms = result.duration_ms,
                        "Sync cycle completed"
                    );

                    let mut state = self.daemon_state.lock().await;
                    state.sync_state = DaemonSyncState::Idle;
                    state.last_sync_result = Some(result_json);
                }
                Err(e) => {
                    let err_msg = format!("{e:#}");
                    error!(error = %err_msg, "Sync cycle failed");

                    let mut state = self.daemon_state.lock().await;
                    state.sync_state = DaemonSyncState::Error(err_msg);
                }
            }

            // Wait for the next interval or shutdown
            tokio::select! {
                _ = interval.tick() => {}
                _ = self.shutdown.cancelled() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }

        info!("Sync loop terminated");
        Ok(())
    }

    /// Waits for authentication in a loop, checking periodically
    ///
    /// When no account or tokens are available, the daemon enters this
    /// wait loop. It checks every 30 seconds for a newly configured account.
    async fn wait_for_auth_loop(&self) -> Result<()> {
        {
            let mut state = self.daemon_state.lock().await;
            state.sync_state = DaemonSyncState::WaitingForAuth;
        }

        info!("Waiting for authentication. Run 'lnxdrive auth login' to configure.");

        let check_interval = Duration::from_secs(30);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(check_interval) => {
                    // Check if an account has been configured
                    match self.state_repo.get_default_account().await {
                        Ok(Some(account)) => {
                            match KeyringTokenStorage::load(account.email().as_str()) {
                                Ok(Some(_tokens)) => {
                                    info!(
                                        email = %account.email(),
                                        "Account and tokens found, restarting daemon"
                                    );
                                    // Authentication is now available; restart the
                                    // run process by returning Ok to signal the caller.
                                    // In practice the daemon would need to be restarted
                                    // or re-enter the run loop.
                                    return Ok(());
                                }
                                _ => {
                                    // Still no tokens, keep waiting
                                }
                            }
                        }
                        _ => {
                            // Still no account, keep waiting
                        }
                    }
                }
                _ = self.shutdown.cancelled() => {
                    info!("Shutdown signal received while waiting for auth");
                    return Ok(());
                }
            }
        }
    }
}

// ============================================================================
// T217: Graceful shutdown signal handler
// ============================================================================

/// Waits for SIGTERM or SIGINT and triggers the cancellation token
///
/// This function spawns a task that listens for OS signals and cancels
/// the provided token when a shutdown signal is received.
async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received SIGINT (Ctrl+C)");
        }
        _ = terminate => {
            info!("Received SIGTERM");
        }
    }

    token.cancel();
}

// ============================================================================
// T215/T217/T218: Main entry point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .init();

    info!("LNXDrive daemon starting (lnxdrived)");

    // T218: Create cancellation token for propagation to all tasks
    let shutdown_token = CancellationToken::new();

    // Spawn signal handler task
    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        shutdown_signal(signal_token).await;
    });

    // Create and run the daemon service
    let service = DaemonService::new(shutdown_token.clone()).await?;

    let result = service.run().await;

    match &result {
        Ok(()) => info!("LNXDrive daemon shut down gracefully"),
        Err(e) => error!(error = %e, "LNXDrive daemon exiting with error"),
    }

    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_token_creation() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_cancel() {
        let token = CancellationToken::new();
        let child = token.child_token();
        token.cancel();
        assert!(token.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_child_propagation() {
        let parent = CancellationToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();

        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());

        parent.cancel();

        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
    }

    #[test]
    fn test_config_default_poll_interval() {
        let config = Config::default();
        assert!(config.sync.poll_interval > 0);
    }

    #[test]
    fn test_config_default_path_exists() {
        let path = Config::default_path();
        // Just verify it returns a non-empty path
        assert!(!path.as_os_str().is_empty());
    }
}
