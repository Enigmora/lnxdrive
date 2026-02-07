//! Prometheus metrics registry for LNXDrive
//!
//! Provides typed, labeled counters, gauges, and histograms for all
//! observable operations in the sync engine, API layer, and conflict system.

use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};

/// Central metrics registry holding all Prometheus metrics.
pub struct MetricsRegistry {
    registry: Registry,
    /// Gauge: number of tracked files per state (online, hydrated, etc.)
    pub files_total: IntGaugeVec,
    /// Counter: total sync operations by (operation, status)
    pub sync_operations_total: IntCounterVec,
    /// Counter: bytes transferred by direction (upload, download)
    pub sync_bytes_total: IntCounterVec,
    /// Counter: Graph API requests by (endpoint, status)
    pub api_requests_total: IntCounterVec,
    /// Counter: conflicts by resolution type
    pub conflicts_total: IntCounterVec,
    /// Histogram: file hydration duration in seconds
    pub hydration_duration_seconds: HistogramVec,
}

impl MetricsRegistry {
    /// Creates a new `MetricsRegistry` with all metrics registered.
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new_custom(Some("lnxdrive".to_string()), None)?;

        let files_total = IntGaugeVec::new(
            Opts::new("files_total", "Number of tracked files by state"),
            &["state"],
        )?;
        registry.register(Box::new(files_total.clone()))?;

        let sync_operations_total = IntCounterVec::new(
            Opts::new("sync_operations_total", "Total sync operations"),
            &["operation", "status"],
        )?;
        registry.register(Box::new(sync_operations_total.clone()))?;

        let sync_bytes_total = IntCounterVec::new(
            Opts::new("sync_bytes_total", "Total bytes transferred"),
            &["direction"],
        )?;
        registry.register(Box::new(sync_bytes_total.clone()))?;

        let api_requests_total = IntCounterVec::new(
            Opts::new("api_requests_total", "Total Graph API requests"),
            &["endpoint", "status"],
        )?;
        registry.register(Box::new(api_requests_total.clone()))?;

        let conflicts_total = IntCounterVec::new(
            Opts::new("conflicts_total", "Total conflicts by resolution"),
            &["resolution"],
        )?;
        registry.register(Box::new(conflicts_total.clone()))?;

        let hydration_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "hydration_duration_seconds",
                "File hydration duration in seconds",
            )
            .buckets(vec![1.0, 5.0, 30.0, f64::INFINITY]),
            &["result"],
        )?;
        registry.register(Box::new(hydration_duration_seconds.clone()))?;

        Ok(Self {
            registry,
            files_total,
            sync_operations_total,
            sync_bytes_total,
            api_requests_total,
            conflicts_total,
            hydration_duration_seconds,
        })
    }

    // ========================================================================
    // Recording helpers
    // ========================================================================

    /// Record a sync operation outcome.
    pub fn record_sync_operation(&self, operation: &str, status: &str) {
        self.sync_operations_total
            .with_label_values(&[operation, status])
            .inc();
    }

    /// Record bytes transferred in a given direction.
    pub fn record_bytes_transferred(&self, direction: &str, bytes: u64) {
        self.sync_bytes_total
            .with_label_values(&[direction])
            .inc_by(bytes);
    }

    /// Record a Graph API request.
    pub fn record_api_request(&self, endpoint: &str, status: &str) {
        self.api_requests_total
            .with_label_values(&[endpoint, status])
            .inc();
    }

    /// Record a conflict with its resolution.
    pub fn record_conflict(&self, resolution: &str) {
        self.conflicts_total
            .with_label_values(&[resolution])
            .inc();
    }

    /// Observe a file hydration duration.
    pub fn observe_hydration_duration(&self, result: &str, duration_secs: f64) {
        self.hydration_duration_seconds
            .with_label_values(&[result])
            .observe(duration_secs);
    }

    /// Set the gauge for files per state.
    pub fn set_files_total(&self, state: &str, count: i64) {
        self.files_total.with_label_values(&[state]).set(count);
    }

    // ========================================================================
    // Encoding
    // ========================================================================

    /// Encode all metrics in Prometheus text exposition format.
    pub fn encode(&self) -> anyhow::Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry_creation() {
        let registry = MetricsRegistry::new().expect("create registry");
        // Should be able to encode empty metrics
        let output = registry.encode().expect("encode");
        assert!(output.is_empty() || output.contains("lnxdrive"));
    }

    #[test]
    fn test_record_sync_operation() {
        let registry = MetricsRegistry::new().unwrap();
        registry.record_sync_operation("download", "success");
        registry.record_sync_operation("download", "success");
        registry.record_sync_operation("upload", "failure");

        let output = registry.encode().unwrap();
        assert!(output.contains("lnxdrive_sync_operations_total"));
        assert!(output.contains("download"));
        assert!(output.contains("upload"));
    }

    #[test]
    fn test_record_bytes_transferred() {
        let registry = MetricsRegistry::new().unwrap();
        registry.record_bytes_transferred("download", 1024);
        registry.record_bytes_transferred("upload", 512);

        let output = registry.encode().unwrap();
        assert!(output.contains("lnxdrive_sync_bytes_total"));
    }

    #[test]
    fn test_record_conflict() {
        let registry = MetricsRegistry::new().unwrap();
        registry.record_conflict("keep_local");
        registry.record_conflict("keep_both");

        let output = registry.encode().unwrap();
        assert!(output.contains("lnxdrive_conflicts_total"));
    }

    #[test]
    fn test_set_files_total() {
        let registry = MetricsRegistry::new().unwrap();
        registry.set_files_total("online", 100);
        registry.set_files_total("hydrated", 50);

        let output = registry.encode().unwrap();
        assert!(output.contains("lnxdrive_files_total"));
    }

    #[test]
    fn test_observe_hydration_duration() {
        let registry = MetricsRegistry::new().unwrap();
        registry.observe_hydration_duration("success", 2.5);

        let output = registry.encode().unwrap();
        assert!(output.contains("lnxdrive_hydration_duration_seconds"));
    }

    #[test]
    fn test_encode_produces_valid_output() {
        let registry = MetricsRegistry::new().unwrap();
        registry.record_sync_operation("download", "success");
        registry.record_bytes_transferred("download", 2048);
        registry.set_files_total("online", 42);

        let output = registry.encode().unwrap();
        // Should contain HELP and TYPE lines
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }
}
