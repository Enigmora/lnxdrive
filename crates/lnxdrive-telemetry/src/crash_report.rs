//! Crash report generation and persistence
//!
//! Captures panic information and saves structured JSON reports
//! to `~/.local/share/lnxdrive/reports/`.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::os_info::OsInfo;

/// A structured crash report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub id: String,
    pub timestamp: String,
    pub version: String,
    pub component: String,
    pub panic_message: String,
    pub location: String,
    pub backtrace: String,
    pub os_info: OsInfo,
}

impl CrashReport {
    /// Create a new crash report from panic information.
    pub fn new(
        component: &str,
        panic_message: &str,
        location: &str,
        backtrace: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            component: component.to_string(),
            panic_message: panic_message.to_string(),
            location: location.to_string(),
            backtrace: backtrace.to_string(),
            os_info: OsInfo::collect(),
        }
    }
}

/// Save a crash report to the reports directory.
///
/// Creates the directory if needed. File name: `crash-{date}-{uuid8}.json`
pub fn save_crash_report(reports_dir: &Path, report: &CrashReport) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(reports_dir)?;

    let date = Utc::now().format("%Y%m%d");
    let short_id = &report.id[..8];
    let filename = format!("crash-{date}-{short_id}.json");
    let path = reports_dir.join(filename);

    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&path, json)?;

    Ok(path)
}

/// Installs a panic hook that saves crash reports to `reports_dir`.
///
/// Chains with the existing panic hook so default behavior (stderr output)
/// is preserved.
pub fn install_crash_reporter(reports_dir: PathBuf) {
    let previous_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();

        let backtrace = std::backtrace::Backtrace::force_capture().to_string();

        let report = CrashReport::new("lnxdrive", &message, &location, &backtrace);

        if let Err(e) = save_crash_report(&reports_dir, &report) {
            eprintln!("Failed to save crash report: {e}");
        }

        // Call the previous panic hook
        previous_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_report_creation() {
        let report = CrashReport::new("test", "test panic", "lib.rs:42:1", "fake backtrace");
        assert!(!report.id.is_empty());
        assert_eq!(report.component, "test");
        assert_eq!(report.panic_message, "test panic");
        assert_eq!(report.location, "lib.rs:42:1");
    }

    #[test]
    fn test_save_crash_report() {
        let dir = tempfile::tempdir().unwrap();
        let report = CrashReport::new("test", "boom", "main.rs:10:5", "");

        let path = save_crash_report(dir.path(), &report).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: CrashReport = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.panic_message, "boom");
    }
}
