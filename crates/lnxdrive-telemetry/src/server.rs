//! HTTP metrics server for Prometheus scraping
//!
//! Exposes a `/metrics` endpoint on `127.0.0.1:9100` (configurable)
//! that returns Prometheus text exposition format.

use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::metrics::MetricsRegistry;

/// HTTP server that serves Prometheus metrics on a configurable endpoint.
pub struct MetricsServer {
    metrics: Arc<MetricsRegistry>,
    addr: SocketAddr,
}

impl MetricsServer {
    /// Creates a new `MetricsServer`.
    ///
    /// # Arguments
    /// * `metrics` - The shared metrics registry
    /// * `endpoint` - Address to bind, e.g. `"127.0.0.1:9100"`
    pub fn new(metrics: Arc<MetricsRegistry>, endpoint: &str) -> anyhow::Result<Self> {
        let addr: SocketAddr = endpoint.parse()?;
        Ok(Self { metrics, addr })
    }

    /// Starts the HTTP server. This future runs indefinitely until the
    /// provided cancellation token is triggered.
    ///
    /// Should be spawned as a background task.
    pub async fn run(&self, shutdown: tokio_util::sync::CancellationToken) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        info!(addr = %self.addr, "Metrics server listening");

        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, _) = result?;
                    let io = TokioIo::new(stream);
                    let metrics = Arc::clone(&self.metrics);

                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            let metrics = Arc::clone(&metrics);
                            async move { handle_request(req, &metrics) }
                        });

                        if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                            error!(error = %e, "Metrics HTTP connection error");
                        }
                    });
                }
                _ = shutdown.cancelled() => {
                    info!("Metrics server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Handle a single HTTP request.
fn handle_request(
    req: Request<hyper::body::Incoming>,
    metrics: &MetricsRegistry,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    if req.uri().path() == "/metrics" {
        match metrics.encode() {
            Ok(body) => Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
                .body(Full::new(Bytes::from(body)))
                .unwrap()),
            Err(e) => {
                let body = format!("Failed to encode metrics: {e}");
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from(body)))
                    .unwrap())
            }
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_server_creation() {
        let metrics = Arc::new(MetricsRegistry::new().unwrap());
        let server = MetricsServer::new(metrics, "127.0.0.1:0");
        assert!(server.is_ok());
    }

    #[test]
    fn test_metrics_server_invalid_addr() {
        let metrics = Arc::new(MetricsRegistry::new().unwrap());
        let server = MetricsServer::new(metrics, "not-an-address");
        assert!(server.is_err());
    }

    #[test]
    fn test_encode_after_recording() {
        let metrics = MetricsRegistry::new().unwrap();
        metrics.record_sync_operation("download", "success");

        let encoded = metrics.encode().unwrap();
        assert!(encoded.contains("lnxdrive_sync_operations_total"));
    }
}
