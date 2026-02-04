//! Config command - View and manage LNXDrive configuration
//!
//! Provides the `lnxdrive config` CLI command which:
//! 1. Shows the current configuration (YAML or JSON)
//! 2. Sets individual configuration values via dot-notation keys
//! 3. Validates the configuration file and reports errors

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use tracing::info;

use crate::output::{get_formatter, OutputFormat};

/// T233: Config subcommands
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Display current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., "sync.poll_interval")
        key: String,
        /// New value
        value: String,
    },
    /// Validate configuration file
    Validate,
}

impl ConfigCommand {
    /// Execute the config command
    pub async fn execute(&self, format: OutputFormat) -> Result<()> {
        match self {
            ConfigCommand::Show => self.execute_show(format).await,
            ConfigCommand::Set { key, value } => self.execute_set(key, value, format).await,
            ConfigCommand::Validate => self.execute_validate(format).await,
        }
    }

    /// T234: Show current configuration
    async fn execute_show(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::config::Config;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let config_path = Config::default_path();
        let config = Config::load_or_default(&config_path);

        info!(config_path = %config_path.display(), "Showing configuration");

        if matches!(format, OutputFormat::Json) {
            let json = serde_json::to_value(&config)
                .context("Failed to serialize configuration to JSON")?;
            formatter.print_json(&json);
        } else {
            formatter.success(&format!("Configuration ({})", config_path.display()));
            formatter.info("");

            let yaml = serde_yaml::to_string(&config)
                .context("Failed to serialize configuration to YAML")?;

            for line in yaml.lines() {
                formatter.info(line);
            }
        }

        Ok(())
    }

    /// T235: Set a configuration value using dot-notation
    async fn execute_set(&self, key: &str, value: &str, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::config::Config;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let config_path = Config::default_path();
        let mut config = Config::load_or_default(&config_path);

        info!(key = %key, value = %value, "Setting configuration value");

        match apply_config_value(&mut config, key, value) {
            Ok(()) => {
                // Validate the new config before saving
                let validation_errors = config.validate();
                // Filter out sync.root errors since the directory may not exist yet
                let real_errors: Vec<_> = validation_errors
                    .iter()
                    .filter(|e| e.field != "sync.root")
                    .collect();

                if !real_errors.is_empty() {
                    let error_msgs: Vec<String> = real_errors
                        .iter()
                        .map(|e| format!("{}: {}", e.field, e.message))
                        .collect();

                    if matches!(format, OutputFormat::Json) {
                        let json = serde_json::json!({
                            "success": false,
                            "key": key,
                            "value": value,
                            "errors": error_msgs,
                        });
                        formatter.print_json(&json);
                    } else {
                        formatter.error(&format!(
                            "Invalid value for '{}': {}",
                            key,
                            error_msgs.join("; ")
                        ));
                    }
                    return Ok(());
                }

                // Ensure parent directory exists
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)
                        .context("Failed to create configuration directory")?;
                }

                // Serialize and save
                let yaml =
                    serde_yaml::to_string(&config).context("Failed to serialize configuration")?;
                std::fs::write(&config_path, &yaml)
                    .context("Failed to write configuration file")?;

                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "success": true,
                        "key": key,
                        "value": value,
                        "config_path": config_path.display().to_string(),
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.success(&format!("Set {} = {}", key, value));
                    formatter.info(&format!("Saved to {}", config_path.display()));
                }
            }
            Err(e) => {
                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "success": false,
                        "key": key,
                        "value": value,
                        "error": e.to_string(),
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.error(&format!("Failed to set '{}': {}", key, e));
                    formatter.info("");
                    formatter.info("Supported keys:");
                    formatter.info("  sync.root                            - Sync root directory");
                    formatter
                        .info("  sync.poll_interval                   - Seconds between polling");
                    formatter
                        .info("  sync.debounce_delay                  - Seconds debounce delay");
                    formatter.info("  rate_limiting.delta_requests_per_minute");
                    formatter.info("  rate_limiting.upload_concurrent");
                    formatter.info("  rate_limiting.upload_requests_per_minute");
                    formatter.info("  rate_limiting.download_concurrent");
                    formatter.info("  rate_limiting.metadata_requests_per_minute");
                    formatter.info(
                        "  large_files.threshold_mb             - Large file threshold (MiB)",
                    );
                    formatter
                        .info("  large_files.chunk_size_mb            - Upload chunk size (MiB)");
                    formatter.info(
                        "  large_files.max_concurrent_large     - Max concurrent large uploads",
                    );
                    formatter.info("  conflicts.default_strategy           - manual|keep_local|keep_remote|keep_both");
                    formatter.info(
                        "  logging.level                        - trace|debug|info|warn|error",
                    );
                    formatter.info("  logging.file                         - Log file path");
                    formatter
                        .info("  logging.max_size_mb                  - Max log file size (MiB)");
                    formatter
                        .info("  logging.max_files                    - Max rotated log files");
                    formatter
                        .info("  auth.app_id                          - Azure AD application ID");
                }
            }
        }

        Ok(())
    }

    /// T236: Validate configuration file
    async fn execute_validate(&self, format: OutputFormat) -> Result<()> {
        use lnxdrive_core::config::Config;

        let formatter = get_formatter(matches!(format, OutputFormat::Json));

        let config_path = Config::default_path();

        // Try to load the config file explicitly (not load_or_default)
        let config = match Config::load(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                if !config_path.exists() {
                    if matches!(format, OutputFormat::Json) {
                        let json = serde_json::json!({
                            "valid": false,
                            "config_path": config_path.display().to_string(),
                            "errors": ["Configuration file not found. Using defaults."],
                        });
                        formatter.print_json(&json);
                    } else {
                        formatter.info(&format!(
                            "Configuration file not found at {}",
                            config_path.display()
                        ));
                        formatter.info("Using default configuration. Run 'lnxdrive config set <key> <value>' to create one.");
                    }
                    return Ok(());
                }

                if matches!(format, OutputFormat::Json) {
                    let json = serde_json::json!({
                        "valid": false,
                        "config_path": config_path.display().to_string(),
                        "errors": [format!("Failed to parse configuration: {}", e)],
                    });
                    formatter.print_json(&json);
                } else {
                    formatter.error(&format!("Failed to parse configuration: {}", e));
                    formatter.info(&format!("File: {}", config_path.display()));
                }
                return Ok(());
            }
        };

        info!(config_path = %config_path.display(), "Validating configuration");

        let errors = config.validate();

        if matches!(format, OutputFormat::Json) {
            let error_strings: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            let json = serde_json::json!({
                "valid": errors.is_empty(),
                "config_path": config_path.display().to_string(),
                "errors": error_strings,
            });
            formatter.print_json(&json);
        } else if errors.is_empty() {
            formatter.success("Configuration is valid");
            formatter.info(&format!("File: {}", config_path.display()));
        } else {
            formatter.error(&format!(
                "Configuration has {} error{}:",
                errors.len(),
                if errors.len() == 1 { "" } else { "s" }
            ));
            formatter.info(&format!("File: {}", config_path.display()));
            formatter.info("");
            for error in &errors {
                formatter.info(&format!("  {} - {}", error.field, error.message));
            }
        }

        Ok(())
    }
}

/// Apply a dot-notation key/value pair to a Config struct
///
/// Supported keys:
/// - sync.root, sync.poll_interval, sync.debounce_delay
/// - rate_limiting.delta_requests_per_minute, etc.
/// - large_files.threshold_mb, etc.
/// - conflicts.default_strategy
/// - logging.level, logging.file, logging.max_size_mb, logging.max_files
/// - auth.app_id
fn apply_config_value(
    config: &mut lnxdrive_core::config::Config,
    key: &str,
    value: &str,
) -> Result<()> {
    match key {
        // --- sync ---
        "sync.root" => {
            config.sync.root = PathBuf::from(value);
        }
        "sync.poll_interval" => {
            config.sync.poll_interval = value
                .parse::<u64>()
                .context("Expected a positive integer for sync.poll_interval")?;
        }
        "sync.debounce_delay" => {
            config.sync.debounce_delay = value
                .parse::<u64>()
                .context("Expected a positive integer for sync.debounce_delay")?;
        }

        // --- rate_limiting ---
        "rate_limiting.delta_requests_per_minute" => {
            config.rate_limiting.delta_requests_per_minute = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }
        "rate_limiting.upload_concurrent" => {
            config.rate_limiting.upload_concurrent = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }
        "rate_limiting.upload_requests_per_minute" => {
            config.rate_limiting.upload_requests_per_minute = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }
        "rate_limiting.download_concurrent" => {
            config.rate_limiting.download_concurrent = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }
        "rate_limiting.metadata_requests_per_minute" => {
            config.rate_limiting.metadata_requests_per_minute = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }

        // --- large_files ---
        "large_files.threshold_mb" => {
            config.large_files.threshold_mb = value
                .parse::<u64>()
                .context("Expected a positive integer")?;
        }
        "large_files.chunk_size_mb" => {
            config.large_files.chunk_size_mb = value
                .parse::<u64>()
                .context("Expected a positive integer")?;
        }
        "large_files.max_concurrent_large" => {
            config.large_files.max_concurrent_large = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }

        // --- conflicts ---
        "conflicts.default_strategy" => {
            config.conflicts.default_strategy = value.to_string();
        }

        // --- logging ---
        "logging.level" => {
            config.logging.level = value.to_string();
        }
        "logging.file" => {
            config.logging.file = PathBuf::from(value);
        }
        "logging.max_size_mb" => {
            config.logging.max_size_mb = value
                .parse::<u64>()
                .context("Expected a positive integer")?;
        }
        "logging.max_files" => {
            config.logging.max_files = value
                .parse::<u32>()
                .context("Expected a positive integer")?;
        }

        // --- auth ---
        "auth.app_id" => {
            config.auth.app_id = if value.is_empty() || value == "none" {
                None
            } else {
                Some(value.to_string())
            };
        }

        _ => {
            anyhow::bail!("Unknown configuration key: '{}'", key);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnxdrive_core::config::Config;

    #[test]
    fn test_apply_sync_root() {
        let mut config = Config::default();
        apply_config_value(&mut config, "sync.root", "/custom/path").unwrap();
        assert_eq!(config.sync.root, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_apply_sync_poll_interval() {
        let mut config = Config::default();
        apply_config_value(&mut config, "sync.poll_interval", "60").unwrap();
        assert_eq!(config.sync.poll_interval, 60);
    }

    #[test]
    fn test_apply_sync_debounce_delay() {
        let mut config = Config::default();
        apply_config_value(&mut config, "sync.debounce_delay", "5").unwrap();
        assert_eq!(config.sync.debounce_delay, 5);
    }

    #[test]
    fn test_apply_rate_limiting_delta() {
        let mut config = Config::default();
        apply_config_value(&mut config, "rate_limiting.delta_requests_per_minute", "20").unwrap();
        assert_eq!(config.rate_limiting.delta_requests_per_minute, 20);
    }

    #[test]
    fn test_apply_rate_limiting_upload_concurrent() {
        let mut config = Config::default();
        apply_config_value(&mut config, "rate_limiting.upload_concurrent", "8").unwrap();
        assert_eq!(config.rate_limiting.upload_concurrent, 8);
    }

    #[test]
    fn test_apply_rate_limiting_upload_rpm() {
        let mut config = Config::default();
        apply_config_value(
            &mut config,
            "rate_limiting.upload_requests_per_minute",
            "120",
        )
        .unwrap();
        assert_eq!(config.rate_limiting.upload_requests_per_minute, 120);
    }

    #[test]
    fn test_apply_rate_limiting_download_concurrent() {
        let mut config = Config::default();
        apply_config_value(&mut config, "rate_limiting.download_concurrent", "16").unwrap();
        assert_eq!(config.rate_limiting.download_concurrent, 16);
    }

    #[test]
    fn test_apply_rate_limiting_metadata_rpm() {
        let mut config = Config::default();
        apply_config_value(
            &mut config,
            "rate_limiting.metadata_requests_per_minute",
            "200",
        )
        .unwrap();
        assert_eq!(config.rate_limiting.metadata_requests_per_minute, 200);
    }

    #[test]
    fn test_apply_large_files_threshold() {
        let mut config = Config::default();
        apply_config_value(&mut config, "large_files.threshold_mb", "200").unwrap();
        assert_eq!(config.large_files.threshold_mb, 200);
    }

    #[test]
    fn test_apply_large_files_chunk_size() {
        let mut config = Config::default();
        apply_config_value(&mut config, "large_files.chunk_size_mb", "20").unwrap();
        assert_eq!(config.large_files.chunk_size_mb, 20);
    }

    #[test]
    fn test_apply_large_files_max_concurrent() {
        let mut config = Config::default();
        apply_config_value(&mut config, "large_files.max_concurrent_large", "3").unwrap();
        assert_eq!(config.large_files.max_concurrent_large, 3);
    }

    #[test]
    fn test_apply_conflicts_strategy() {
        let mut config = Config::default();
        apply_config_value(&mut config, "conflicts.default_strategy", "keep_local").unwrap();
        assert_eq!(config.conflicts.default_strategy, "keep_local");
    }

    #[test]
    fn test_apply_logging_level() {
        let mut config = Config::default();
        apply_config_value(&mut config, "logging.level", "debug").unwrap();
        assert_eq!(config.logging.level, "debug");
    }

    #[test]
    fn test_apply_logging_file() {
        let mut config = Config::default();
        apply_config_value(&mut config, "logging.file", "/var/log/lnxdrive.log").unwrap();
        assert_eq!(config.logging.file, PathBuf::from("/var/log/lnxdrive.log"));
    }

    #[test]
    fn test_apply_logging_max_size() {
        let mut config = Config::default();
        apply_config_value(&mut config, "logging.max_size_mb", "100").unwrap();
        assert_eq!(config.logging.max_size_mb, 100);
    }

    #[test]
    fn test_apply_logging_max_files() {
        let mut config = Config::default();
        apply_config_value(&mut config, "logging.max_files", "10").unwrap();
        assert_eq!(config.logging.max_files, 10);
    }

    #[test]
    fn test_apply_auth_app_id() {
        let mut config = Config::default();
        apply_config_value(&mut config, "auth.app_id", "my-app-id").unwrap();
        assert_eq!(config.auth.app_id, Some("my-app-id".to_string()));
    }

    #[test]
    fn test_apply_auth_app_id_none() {
        let mut config = Config::default();
        config.auth.app_id = Some("existing".to_string());
        apply_config_value(&mut config, "auth.app_id", "none").unwrap();
        assert_eq!(config.auth.app_id, None);
    }

    #[test]
    fn test_apply_auth_app_id_empty() {
        let mut config = Config::default();
        config.auth.app_id = Some("existing".to_string());
        apply_config_value(&mut config, "auth.app_id", "").unwrap();
        assert_eq!(config.auth.app_id, None);
    }

    #[test]
    fn test_apply_unknown_key_fails() {
        let mut config = Config::default();
        let result = apply_config_value(&mut config, "unknown.key", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_invalid_u64_fails() {
        let mut config = Config::default();
        let result = apply_config_value(&mut config, "sync.poll_interval", "not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_invalid_u32_fails() {
        let mut config = Config::default();
        let result = apply_config_value(
            &mut config,
            "rate_limiting.upload_concurrent",
            "not_a_number",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_negative_number_fails() {
        let mut config = Config::default();
        let result = apply_config_value(&mut config, "sync.poll_interval", "-5");
        assert!(result.is_err());
    }
}
