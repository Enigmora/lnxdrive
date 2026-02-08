//! LNXDrive Telemetry - Observability and opt-in reporting
//!
//! Provides:
//! - `MetricsRegistry`: Prometheus metrics (counters, gauges, histograms)
//! - `MetricsServer`: HTTP server for Prometheus scraping
//! - `CrashReport`: Structured panic reports with backtraces
//! - `ErrorReport`: Non-fatal error reports with filtering
//! - `Anonymizer`: PII stripping for reports
//! - `LocalReportStore`: File-based report management

pub mod anonymizer;
pub mod crash_report;
pub mod error_report;
pub mod metrics;
pub mod os_info;
pub mod server;
pub mod store;

pub use anonymizer::Anonymizer;
pub use crash_report::{install_crash_reporter, save_crash_report, CrashReport};
pub use error_report::{ErrorReport, ErrorReporter};
pub use metrics::MetricsRegistry;
pub use os_info::OsInfo;
pub use server::MetricsServer;
pub use store::LocalReportStore;
