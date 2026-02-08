//! Configuration module for LNXDrive.
//!
//! Provides typed configuration structs that map to the YAML configuration file,
//! with loading, validation, defaults, and a builder pattern for programmatic use.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// T099: Config struct with sub-sections
// ---------------------------------------------------------------------------

/// Top-level configuration for LNXDrive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub sync: SyncConfig,
    pub rate_limiting: RateLimitingConfig,
    pub large_files: LargeFilesConfig,
    pub conflicts: ConflictsConfig,
    pub logging: LoggingConfig,
    pub auth: AuthConfig,
    pub fuse: FuseConfig,
}

/// Synchronization settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Root directory for the local OneDrive mirror.
    pub root: PathBuf,
    /// Seconds between remote polling cycles.
    pub poll_interval: u64,
    /// Seconds to wait after a local change before syncing (debounce).
    pub debounce_delay: u64,
}

/// Microsoft Graph API rate-limiting settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub delta_requests_per_minute: u32,
    pub upload_concurrent: u32,
    pub upload_requests_per_minute: u32,
    pub download_concurrent: u32,
    pub metadata_requests_per_minute: u32,
}

/// Large file upload / chunking settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeFilesConfig {
    /// Files above this size (in MiB) are uploaded in chunks.
    pub threshold_mb: u64,
    /// Size of each upload chunk (in MiB).
    pub chunk_size_mb: u64,
    /// Maximum concurrent large-file uploads.
    pub max_concurrent_large: u32,
}

/// Conflict resolution settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictsConfig {
    /// Default conflict strategy: `manual`, `keep_local`, `keep_remote`, or `keep_both`.
    pub default_strategy: String,
}

/// Logging / tracing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: `trace`, `debug`, `info`, `warn`, or `error`.
    pub level: String,
    /// Path to the log file.
    pub file: PathBuf,
    /// Maximum size of a single log file (in MiB) before rotation.
    pub max_size_mb: u64,
    /// Maximum number of rotated log files to keep.
    pub max_files: u32,
}

/// Authentication / OAuth settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Azure AD Application (client) ID. `None` until the user runs `lnxdrive auth login`.
    pub app_id: Option<String>,
}

/// Files-on-Demand (FUSE) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuseConfig {
    /// Directory where the FUSE filesystem will be mounted.
    pub mount_point: String,
    /// Whether to automatically mount the FUSE filesystem on startup.
    pub auto_mount: bool,
    /// Directory for caching hydrated file content.
    pub cache_dir: String,
    /// Maximum size of the cache in gigabytes.
    pub cache_max_size_gb: u32,
    /// Percentage of cache_max_size_gb that triggers dehydration (0-100).
    pub dehydration_threshold_percent: u8,
    /// Maximum age in days before a cached file becomes eligible for dehydration.
    pub dehydration_max_age_days: u32,
    /// Interval in minutes between dehydration background tasks.
    pub dehydration_interval_minutes: u32,
    /// Number of concurrent file hydration operations allowed.
    pub hydration_concurrency: u8,
}

// ---------------------------------------------------------------------------
// T100: Config::load()
// ---------------------------------------------------------------------------

impl Config {
    /// Load configuration from a YAML file at `path`.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Try to load from `path`; fall back to [`Config::default`] on any error.
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    /// Platform-appropriate default path for the configuration file.
    ///
    /// Typically `$XDG_CONFIG_HOME/lnxdrive/config.yaml` on Linux.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("lnxdrive")
            .join("config.yaml")
    }
}

// ---------------------------------------------------------------------------
// T101: Config::default()
// ---------------------------------------------------------------------------

// Config derives Default because all its fields implement Default.
// (clippy::derivable_impls)

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            root: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join("OneDrive"),
            poll_interval: 30,
            debounce_delay: 2,
        }
    }
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            delta_requests_per_minute: 10,
            upload_concurrent: 4,
            upload_requests_per_minute: 60,
            download_concurrent: 8,
            metadata_requests_per_minute: 100,
        }
    }
}

impl Default for LargeFilesConfig {
    fn default() -> Self {
        Self {
            threshold_mb: 100,
            chunk_size_mb: 10,
            max_concurrent_large: 1,
        }
    }
}

impl Default for ConflictsConfig {
    fn default() -> Self {
        Self {
            default_strategy: "manual".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("lnxdrive");
        Self {
            level: "info".to_string(),
            file: data_dir.join("lnxdrive.log"),
            max_size_mb: 50,
            max_files: 5,
        }
    }
}

// AuthConfig derives Default (Option<String> defaults to None).
// (clippy::derivable_impls)

impl Default for FuseConfig {
    fn default() -> Self {
        Self {
            mount_point: "~/OneDrive".to_string(),
            auto_mount: true,
            cache_dir: "~/.local/share/lnxdrive/cache".to_string(),
            cache_max_size_gb: 10,
            dehydration_threshold_percent: 80,
            dehydration_max_age_days: 30,
            dehydration_interval_minutes: 60,
            hydration_concurrency: 8,
        }
    }
}

// ---------------------------------------------------------------------------
// T102: Config::validate()
// ---------------------------------------------------------------------------

/// A single validation error found in the configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Dotted path to the offending field, e.g. `"sync.poll_interval"`.
    pub field: String,
    /// Human-readable explanation.
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Valid values for `logging.level`.
const VALID_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];

/// Valid values for `conflicts.default_strategy`.
const VALID_CONFLICT_STRATEGIES: &[&str] = &["manual", "keep_local", "keep_remote", "keep_both"];

impl Config {
    /// Validate the configuration and return all errors found.
    ///
    /// An empty vector means the configuration is valid.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // --- sync ---
        if self.sync.poll_interval == 0 {
            errors.push(ValidationError {
                field: "sync.poll_interval".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.sync.debounce_delay == 0 {
            errors.push(ValidationError {
                field: "sync.debounce_delay".into(),
                message: "must be greater than 0".into(),
            });
        }

        // Check sync root only when it does not start with `~` (tilde is expanded at runtime).
        let root_str = self.sync.root.to_string_lossy();
        if !root_str.starts_with('~') && !self.sync.root.exists() {
            errors.push(ValidationError {
                field: "sync.root".into(),
                message: format!("directory does not exist: {}", self.sync.root.display()),
            });
        }

        // --- rate_limiting ---
        if self.rate_limiting.delta_requests_per_minute == 0 {
            errors.push(ValidationError {
                field: "rate_limiting.delta_requests_per_minute".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.rate_limiting.upload_concurrent == 0 {
            errors.push(ValidationError {
                field: "rate_limiting.upload_concurrent".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.rate_limiting.upload_requests_per_minute == 0 {
            errors.push(ValidationError {
                field: "rate_limiting.upload_requests_per_minute".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.rate_limiting.download_concurrent == 0 {
            errors.push(ValidationError {
                field: "rate_limiting.download_concurrent".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.rate_limiting.metadata_requests_per_minute == 0 {
            errors.push(ValidationError {
                field: "rate_limiting.metadata_requests_per_minute".into(),
                message: "must be greater than 0".into(),
            });
        }

        // --- large_files ---
        if self.large_files.chunk_size_mb == 0 {
            errors.push(ValidationError {
                field: "large_files.chunk_size_mb".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.large_files.threshold_mb == 0 {
            errors.push(ValidationError {
                field: "large_files.threshold_mb".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.large_files.chunk_size_mb > self.large_files.threshold_mb {
            errors.push(ValidationError {
                field: "large_files.chunk_size_mb".into(),
                message: format!(
                    "chunk_size_mb ({}) must not exceed threshold_mb ({})",
                    self.large_files.chunk_size_mb, self.large_files.threshold_mb
                ),
            });
        }
        if self.large_files.max_concurrent_large == 0 {
            errors.push(ValidationError {
                field: "large_files.max_concurrent_large".into(),
                message: "must be greater than 0".into(),
            });
        }

        // --- conflicts ---
        if !VALID_CONFLICT_STRATEGIES.contains(&self.conflicts.default_strategy.as_str()) {
            errors.push(ValidationError {
                field: "conflicts.default_strategy".into(),
                message: format!(
                    "invalid strategy '{}'; valid options: {}",
                    self.conflicts.default_strategy,
                    VALID_CONFLICT_STRATEGIES.join(", ")
                ),
            });
        }

        // --- logging ---
        if !VALID_LOG_LEVELS.contains(&self.logging.level.as_str()) {
            errors.push(ValidationError {
                field: "logging.level".into(),
                message: format!(
                    "invalid level '{}'; valid options: {}",
                    self.logging.level,
                    VALID_LOG_LEVELS.join(", ")
                ),
            });
        }
        if self.logging.max_size_mb == 0 {
            errors.push(ValidationError {
                field: "logging.max_size_mb".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.logging.max_files == 0 {
            errors.push(ValidationError {
                field: "logging.max_files".into(),
                message: "must be greater than 0".into(),
            });
        }

        // --- fuse ---
        if self.fuse.cache_max_size_gb == 0 {
            errors.push(ValidationError {
                field: "fuse.cache_max_size_gb".into(),
                message: "must be greater than 0".into(),
            });
        }
        if self.fuse.dehydration_threshold_percent == 0
            || self.fuse.dehydration_threshold_percent > 100
        {
            errors.push(ValidationError {
                field: "fuse.dehydration_threshold_percent".into(),
                message: "must be in range 1..=100".into(),
            });
        }
        if self.fuse.hydration_concurrency == 0 || self.fuse.hydration_concurrency > 32 {
            errors.push(ValidationError {
                field: "fuse.hydration_concurrency".into(),
                message: "must be in range 1..=32".into(),
            });
        }
        if self.fuse.dehydration_interval_minutes == 0 {
            errors.push(ValidationError {
                field: "fuse.dehydration_interval_minutes".into(),
                message: "must be greater than 0".into(),
            });
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// T103: ConfigBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`Config`] programmatically.
///
/// Starts from [`Config::default`] and allows selective overrides.
///
/// # Example
///
/// ```rust,no_run
/// use lnxdrive_core::config::ConfigBuilder;
/// use std::path::PathBuf;
///
/// let config = ConfigBuilder::new()
///     .sync_root(PathBuf::from("/home/user/OneDrive"))
///     .sync_poll_interval(60)
///     .logging_level("debug")
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Create a new builder initialised with [`Config::default`] values.
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    // --- sync ---

    pub fn sync_root(mut self, root: PathBuf) -> Self {
        self.config.sync.root = root;
        self
    }

    pub fn sync_poll_interval(mut self, seconds: u64) -> Self {
        self.config.sync.poll_interval = seconds;
        self
    }

    pub fn sync_debounce_delay(mut self, seconds: u64) -> Self {
        self.config.sync.debounce_delay = seconds;
        self
    }

    // --- rate_limiting ---

    pub fn rate_limiting_delta_requests_per_minute(mut self, n: u32) -> Self {
        self.config.rate_limiting.delta_requests_per_minute = n;
        self
    }

    pub fn rate_limiting_upload_concurrent(mut self, n: u32) -> Self {
        self.config.rate_limiting.upload_concurrent = n;
        self
    }

    pub fn rate_limiting_upload_requests_per_minute(mut self, n: u32) -> Self {
        self.config.rate_limiting.upload_requests_per_minute = n;
        self
    }

    pub fn rate_limiting_download_concurrent(mut self, n: u32) -> Self {
        self.config.rate_limiting.download_concurrent = n;
        self
    }

    pub fn rate_limiting_metadata_requests_per_minute(mut self, n: u32) -> Self {
        self.config.rate_limiting.metadata_requests_per_minute = n;
        self
    }

    // --- large_files ---

    pub fn large_files_threshold_mb(mut self, mb: u64) -> Self {
        self.config.large_files.threshold_mb = mb;
        self
    }

    pub fn large_files_chunk_size_mb(mut self, mb: u64) -> Self {
        self.config.large_files.chunk_size_mb = mb;
        self
    }

    pub fn large_files_max_concurrent_large(mut self, n: u32) -> Self {
        self.config.large_files.max_concurrent_large = n;
        self
    }

    // --- conflicts ---

    pub fn conflicts_default_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.config.conflicts.default_strategy = strategy.into();
        self
    }

    // --- logging ---

    pub fn logging_level(mut self, level: impl Into<String>) -> Self {
        self.config.logging.level = level.into();
        self
    }

    pub fn logging_file(mut self, file: PathBuf) -> Self {
        self.config.logging.file = file;
        self
    }

    pub fn logging_max_size_mb(mut self, mb: u64) -> Self {
        self.config.logging.max_size_mb = mb;
        self
    }

    pub fn logging_max_files(mut self, n: u32) -> Self {
        self.config.logging.max_files = n;
        self
    }

    // --- auth ---

    pub fn auth_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.config.auth.app_id = Some(app_id.into());
        self
    }

    // --- fuse ---

    pub fn fuse_mount_point(mut self, mount_point: impl Into<String>) -> Self {
        self.config.fuse.mount_point = mount_point.into();
        self
    }

    pub fn fuse_auto_mount(mut self, auto_mount: bool) -> Self {
        self.config.fuse.auto_mount = auto_mount;
        self
    }

    pub fn fuse_cache_dir(mut self, cache_dir: impl Into<String>) -> Self {
        self.config.fuse.cache_dir = cache_dir.into();
        self
    }

    pub fn fuse_cache_max_size_gb(mut self, gb: u32) -> Self {
        self.config.fuse.cache_max_size_gb = gb;
        self
    }

    pub fn fuse_dehydration_threshold_percent(mut self, percent: u8) -> Self {
        self.config.fuse.dehydration_threshold_percent = percent;
        self
    }

    pub fn fuse_dehydration_max_age_days(mut self, days: u32) -> Self {
        self.config.fuse.dehydration_max_age_days = days;
        self
    }

    pub fn fuse_dehydration_interval_minutes(mut self, minutes: u32) -> Self {
        self.config.fuse.dehydration_interval_minutes = minutes;
        self
    }

    pub fn fuse_hydration_concurrency(mut self, concurrency: u8) -> Self {
        self.config.fuse.hydration_concurrency = concurrency;
        self
    }

    // --- build ---

    /// Consume the builder and return the finished [`Config`].
    pub fn build(self) -> Config {
        self.config
    }

    /// Build and validate in one step. Returns `Err` with the list of
    /// validation errors if the configuration is invalid.
    pub fn build_validated(self) -> Result<Config, Vec<ValidationError>> {
        let config = self.build();
        let errors = config.validate();
        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// T104: Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    // -- Defaults --

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = Config::default();
        assert_eq!(cfg.sync.poll_interval, 30);
        assert_eq!(cfg.sync.debounce_delay, 2);
        assert!(cfg.sync.root.to_string_lossy().contains("OneDrive"));
        assert_eq!(cfg.rate_limiting.delta_requests_per_minute, 10);
        assert_eq!(cfg.rate_limiting.upload_concurrent, 4);
        assert_eq!(cfg.rate_limiting.upload_requests_per_minute, 60);
        assert_eq!(cfg.rate_limiting.download_concurrent, 8);
        assert_eq!(cfg.rate_limiting.metadata_requests_per_minute, 100);
        assert_eq!(cfg.large_files.threshold_mb, 100);
        assert_eq!(cfg.large_files.chunk_size_mb, 10);
        assert_eq!(cfg.large_files.max_concurrent_large, 1);
        assert_eq!(cfg.conflicts.default_strategy, "manual");
        assert_eq!(cfg.logging.level, "info");
        assert_eq!(cfg.logging.max_size_mb, 50);
        assert_eq!(cfg.logging.max_files, 5);
        assert!(cfg.auth.app_id.is_none());
        assert_eq!(cfg.fuse.mount_point, "~/OneDrive");
        assert!(cfg.fuse.auto_mount);
        assert_eq!(cfg.fuse.cache_dir, "~/.local/share/lnxdrive/cache");
        assert_eq!(cfg.fuse.cache_max_size_gb, 10);
        assert_eq!(cfg.fuse.dehydration_threshold_percent, 80);
        assert_eq!(cfg.fuse.dehydration_max_age_days, 30);
        assert_eq!(cfg.fuse.dehydration_interval_minutes, 60);
        assert_eq!(cfg.fuse.hydration_concurrency, 8);
    }

    #[test]
    fn default_config_passes_validation() {
        let cfg = Config::default();
        let errors = cfg.validate();
        // sync.root may not exist on a CI/test machine, filter that out
        let non_root_errors: Vec<_> = errors.iter().filter(|e| e.field != "sync.root").collect();
        assert!(
            non_root_errors.is_empty(),
            "unexpected validation errors: {non_root_errors:?}"
        );
    }

    // -- Loading --

    #[test]
    fn load_from_yaml_file() {
        let yaml = r#"
sync:
  root: /tmp/test-onedrive
  poll_interval: 60
  debounce_delay: 5
rate_limiting:
  delta_requests_per_minute: 20
  upload_concurrent: 2
  upload_requests_per_minute: 30
  download_concurrent: 4
  metadata_requests_per_minute: 50
large_files:
  threshold_mb: 200
  chunk_size_mb: 20
  max_concurrent_large: 2
conflicts:
  default_strategy: keep_both
logging:
  level: debug
  file: /tmp/test.log
  max_size_mb: 25
  max_files: 3
auth:
  app_id: "test-app-id-123"
fuse:
  mount_point: ~/OneDrive
  auto_mount: false
  cache_dir: /tmp/cache
  cache_max_size_gb: 5
  dehydration_threshold_percent: 70
  dehydration_max_age_days: 15
  dehydration_interval_minutes: 30
  hydration_concurrency: 4
"#;
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(yaml.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let cfg = Config::load(tmp.path()).expect("load config");
        assert_eq!(cfg.sync.root, PathBuf::from("/tmp/test-onedrive"));
        assert_eq!(cfg.sync.poll_interval, 60);
        assert_eq!(cfg.sync.debounce_delay, 5);
        assert_eq!(cfg.rate_limiting.delta_requests_per_minute, 20);
        assert_eq!(cfg.rate_limiting.upload_concurrent, 2);
        assert_eq!(cfg.large_files.threshold_mb, 200);
        assert_eq!(cfg.large_files.chunk_size_mb, 20);
        assert_eq!(cfg.conflicts.default_strategy, "keep_both");
        assert_eq!(cfg.logging.level, "debug");
        assert_eq!(cfg.logging.max_files, 3);
        assert_eq!(cfg.auth.app_id, Some("test-app-id-123".to_string()));
        assert_eq!(cfg.fuse.mount_point, "~/OneDrive");
        assert!(!cfg.fuse.auto_mount);
        assert_eq!(cfg.fuse.cache_dir, "/tmp/cache");
        assert_eq!(cfg.fuse.cache_max_size_gb, 5);
        assert_eq!(cfg.fuse.dehydration_threshold_percent, 70);
        assert_eq!(cfg.fuse.dehydration_max_age_days, 15);
        assert_eq!(cfg.fuse.dehydration_interval_minutes, 30);
        assert_eq!(cfg.fuse.hydration_concurrency, 4);
    }

    #[test]
    fn load_or_default_returns_default_on_missing_file() {
        let cfg = Config::load_or_default(Path::new("/nonexistent/config.yaml"));
        assert_eq!(cfg.sync.poll_interval, 30);
    }

    #[test]
    fn load_returns_error_on_invalid_yaml() {
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(b"not: [valid: yaml: {{{").unwrap();
        tmp.flush().unwrap();

        let result = Config::load(tmp.path());
        assert!(result.is_err());
    }

    // -- Validation --

    #[test]
    fn validate_catches_zero_poll_interval() {
        let mut cfg = Config::default();
        cfg.sync.poll_interval = 0;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "sync.poll_interval"));
    }

    #[test]
    fn validate_catches_zero_debounce_delay() {
        let mut cfg = Config::default();
        cfg.sync.debounce_delay = 0;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "sync.debounce_delay"));
    }

    #[test]
    fn validate_catches_zero_rate_limiting_values() {
        let mut cfg = Config::default();
        cfg.rate_limiting.delta_requests_per_minute = 0;
        cfg.rate_limiting.upload_concurrent = 0;
        cfg.rate_limiting.upload_requests_per_minute = 0;
        cfg.rate_limiting.download_concurrent = 0;
        cfg.rate_limiting.metadata_requests_per_minute = 0;
        let errors = cfg.validate();
        let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
        assert!(fields.contains(&"rate_limiting.delta_requests_per_minute"));
        assert!(fields.contains(&"rate_limiting.upload_concurrent"));
        assert!(fields.contains(&"rate_limiting.upload_requests_per_minute"));
        assert!(fields.contains(&"rate_limiting.download_concurrent"));
        assert!(fields.contains(&"rate_limiting.metadata_requests_per_minute"));
    }

    #[test]
    fn validate_catches_chunk_exceeding_threshold() {
        let mut cfg = Config::default();
        cfg.large_files.chunk_size_mb = 200;
        cfg.large_files.threshold_mb = 100;
        let errors = cfg.validate();
        assert!(errors.iter().any(
            |e| e.field == "large_files.chunk_size_mb" && e.message.contains("must not exceed")
        ));
    }

    #[test]
    fn validate_catches_zero_large_file_values() {
        let mut cfg = Config::default();
        cfg.large_files.chunk_size_mb = 0;
        cfg.large_files.threshold_mb = 0;
        cfg.large_files.max_concurrent_large = 0;
        let errors = cfg.validate();
        let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
        assert!(fields.contains(&"large_files.chunk_size_mb"));
        assert!(fields.contains(&"large_files.threshold_mb"));
        assert!(fields.contains(&"large_files.max_concurrent_large"));
    }

    #[test]
    fn validate_catches_invalid_log_level() {
        let mut cfg = Config::default();
        cfg.logging.level = "verbose".to_string();
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "logging.level"));
    }

    #[test]
    fn validate_catches_invalid_conflict_strategy() {
        let mut cfg = Config::default();
        cfg.conflicts.default_strategy = "yolo".to_string();
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "conflicts.default_strategy"));
    }

    #[test]
    fn validate_catches_zero_logging_max_size() {
        let mut cfg = Config::default();
        cfg.logging.max_size_mb = 0;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "logging.max_size_mb"));
    }

    #[test]
    fn validate_catches_zero_logging_max_files() {
        let mut cfg = Config::default();
        cfg.logging.max_files = 0;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "logging.max_files"));
    }

    #[test]
    fn validate_accepts_all_valid_log_levels() {
        for level in VALID_LOG_LEVELS {
            let mut cfg = Config::default();
            cfg.logging.level = level.to_string();
            let errors = cfg.validate();
            assert!(
                !errors.iter().any(|e| e.field == "logging.level"),
                "level '{level}' should be valid"
            );
        }
    }

    #[test]
    fn validate_accepts_all_valid_conflict_strategies() {
        for strat in VALID_CONFLICT_STRATEGIES {
            let mut cfg = Config::default();
            cfg.conflicts.default_strategy = strat.to_string();
            let errors = cfg.validate();
            assert!(
                !errors
                    .iter()
                    .any(|e| e.field == "conflicts.default_strategy"),
                "strategy '{strat}' should be valid"
            );
        }
    }

    // -- Builder --

    #[test]
    fn builder_starts_from_defaults() {
        let cfg = ConfigBuilder::new().build();
        assert_eq!(cfg.sync.poll_interval, 30);
        assert_eq!(cfg.conflicts.default_strategy, "manual");
    }

    #[test]
    fn builder_overrides_fields() {
        let cfg = ConfigBuilder::new()
            .sync_root(PathBuf::from("/custom/path"))
            .sync_poll_interval(120)
            .sync_debounce_delay(10)
            .rate_limiting_delta_requests_per_minute(5)
            .rate_limiting_upload_concurrent(8)
            .rate_limiting_upload_requests_per_minute(120)
            .rate_limiting_download_concurrent(16)
            .rate_limiting_metadata_requests_per_minute(200)
            .large_files_threshold_mb(500)
            .large_files_chunk_size_mb(50)
            .large_files_max_concurrent_large(3)
            .conflicts_default_strategy("keep_local")
            .logging_level("debug")
            .logging_file(PathBuf::from("/tmp/lnxdrive.log"))
            .logging_max_size_mb(100)
            .logging_max_files(10)
            .auth_app_id("my-app-id")
            .build();

        assert_eq!(cfg.sync.root, PathBuf::from("/custom/path"));
        assert_eq!(cfg.sync.poll_interval, 120);
        assert_eq!(cfg.sync.debounce_delay, 10);
        assert_eq!(cfg.rate_limiting.delta_requests_per_minute, 5);
        assert_eq!(cfg.rate_limiting.upload_concurrent, 8);
        assert_eq!(cfg.rate_limiting.upload_requests_per_minute, 120);
        assert_eq!(cfg.rate_limiting.download_concurrent, 16);
        assert_eq!(cfg.rate_limiting.metadata_requests_per_minute, 200);
        assert_eq!(cfg.large_files.threshold_mb, 500);
        assert_eq!(cfg.large_files.chunk_size_mb, 50);
        assert_eq!(cfg.large_files.max_concurrent_large, 3);
        assert_eq!(cfg.conflicts.default_strategy, "keep_local");
        assert_eq!(cfg.logging.level, "debug");
        assert_eq!(cfg.logging.file, PathBuf::from("/tmp/lnxdrive.log"));
        assert_eq!(cfg.logging.max_size_mb, 100);
        assert_eq!(cfg.logging.max_files, 10);
        assert_eq!(cfg.auth.app_id, Some("my-app-id".to_string()));
    }

    #[test]
    fn builder_build_validated_succeeds_for_valid_config() {
        let result = ConfigBuilder::new()
            .sync_root(PathBuf::from("~/OneDrive"))
            .build_validated();
        assert!(result.is_ok());
    }

    #[test]
    fn builder_build_validated_fails_for_invalid_config() {
        let result = ConfigBuilder::new()
            .sync_poll_interval(0)
            .logging_level("nope")
            .build_validated();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.len() >= 2);
    }

    #[test]
    fn validate_catches_zero_fuse_cache_max_size() {
        let mut cfg = Config::default();
        cfg.fuse.cache_max_size_gb = 0;
        let errors = cfg.validate();
        assert!(errors.iter().any(|e| e.field == "fuse.cache_max_size_gb"));
    }

    #[test]
    fn validate_catches_invalid_fuse_dehydration_threshold() {
        let mut cfg = Config::default();
        cfg.fuse.dehydration_threshold_percent = 0;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "fuse.dehydration_threshold_percent"));

        let mut cfg = Config::default();
        cfg.fuse.dehydration_threshold_percent = 101;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "fuse.dehydration_threshold_percent"));
    }

    #[test]
    fn validate_catches_invalid_fuse_hydration_concurrency() {
        let mut cfg = Config::default();
        cfg.fuse.hydration_concurrency = 0;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "fuse.hydration_concurrency"));

        let mut cfg = Config::default();
        cfg.fuse.hydration_concurrency = 33;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "fuse.hydration_concurrency"));
    }

    #[test]
    fn validate_catches_zero_fuse_dehydration_interval() {
        let mut cfg = Config::default();
        cfg.fuse.dehydration_interval_minutes = 0;
        let errors = cfg.validate();
        assert!(errors
            .iter()
            .any(|e| e.field == "fuse.dehydration_interval_minutes"));
    }

    #[test]
    fn validate_accepts_valid_fuse_values() {
        let mut cfg = Config::default();
        cfg.fuse.cache_max_size_gb = 20;
        cfg.fuse.dehydration_threshold_percent = 50;
        cfg.fuse.hydration_concurrency = 16;
        cfg.fuse.dehydration_interval_minutes = 120;
        let errors = cfg.validate();
        let fuse_errors: Vec<_> = errors
            .iter()
            .filter(|e| e.field.starts_with("fuse."))
            .collect();
        assert!(
            fuse_errors.is_empty(),
            "unexpected fuse validation errors: {fuse_errors:?}"
        );
    }

    // -- default_path --

    #[test]
    fn default_path_ends_with_config_yaml() {
        let p = Config::default_path();
        assert!(p.ends_with("lnxdrive/config.yaml"));
    }

    // -- ValidationError Display --

    #[test]
    fn validation_error_display() {
        let err = ValidationError {
            field: "sync.poll_interval".into(),
            message: "must be greater than 0".into(),
        };
        assert_eq!(
            err.to_string(),
            "sync.poll_interval: must be greater than 0"
        );
    }

    // -- FuseConfig-specific tests (T016) --

    #[test]
    fn fuse_config_default_returns_expected_values() {
        let fuse = FuseConfig::default();
        assert_eq!(fuse.mount_point, "~/OneDrive");
        assert_eq!(fuse.auto_mount, true);
        assert_eq!(fuse.cache_dir, "~/.local/share/lnxdrive/cache");
        assert_eq!(fuse.cache_max_size_gb, 10);
        assert_eq!(fuse.dehydration_threshold_percent, 80);
        assert_eq!(fuse.dehydration_max_age_days, 30);
        assert_eq!(fuse.dehydration_interval_minutes, 60);
        assert_eq!(fuse.hydration_concurrency, 8);
    }

    #[test]
    fn fuse_config_deserializes_from_yaml() {
        let yaml = r#"
mount_point: /mnt/onedrive
auto_mount: false
cache_dir: /var/cache/lnxdrive
cache_max_size_gb: 25
dehydration_threshold_percent: 75
dehydration_max_age_days: 45
dehydration_interval_minutes: 90
hydration_concurrency: 12
"#;
        let fuse: FuseConfig = serde_yaml::from_str(yaml).expect("deserialize FuseConfig");
        assert_eq!(fuse.mount_point, "/mnt/onedrive");
        assert_eq!(fuse.auto_mount, false);
        assert_eq!(fuse.cache_dir, "/var/cache/lnxdrive");
        assert_eq!(fuse.cache_max_size_gb, 25);
        assert_eq!(fuse.dehydration_threshold_percent, 75);
        assert_eq!(fuse.dehydration_max_age_days, 45);
        assert_eq!(fuse.dehydration_interval_minutes, 90);
        assert_eq!(fuse.hydration_concurrency, 12);
    }

    #[test]
    fn full_config_with_fuse_section_loads_correctly() {
        let yaml = r#"
sync:
  root: ~/OneDrive
  poll_interval: 30
  debounce_delay: 2
rate_limiting:
  delta_requests_per_minute: 10
  upload_concurrent: 4
  upload_requests_per_minute: 60
  download_concurrent: 8
  metadata_requests_per_minute: 100
large_files:
  threshold_mb: 100
  chunk_size_mb: 10
  max_concurrent_large: 1
conflicts:
  default_strategy: manual
logging:
  level: info
  file: ~/.local/share/lnxdrive/lnxdrive.log
  max_size_mb: 50
  max_files: 5
auth:
  app_id: null
fuse:
  mount_point: ~/OneDrive
  auto_mount: true
  cache_dir: ~/.local/share/lnxdrive/cache
  cache_max_size_gb: 15
  dehydration_threshold_percent: 85
  dehydration_max_age_days: 60
  dehydration_interval_minutes: 120
  hydration_concurrency: 10
"#;
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(yaml.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let cfg = Config::load(tmp.path()).expect("load config with fuse section");
        assert_eq!(cfg.fuse.mount_point, "~/OneDrive");
        assert_eq!(cfg.fuse.auto_mount, true);
        assert_eq!(cfg.fuse.cache_dir, "~/.local/share/lnxdrive/cache");
        assert_eq!(cfg.fuse.cache_max_size_gb, 15);
        assert_eq!(cfg.fuse.dehydration_threshold_percent, 85);
        assert_eq!(cfg.fuse.dehydration_max_age_days, 60);
        assert_eq!(cfg.fuse.dehydration_interval_minutes, 120);
        assert_eq!(cfg.fuse.hydration_concurrency, 10);
    }
}
