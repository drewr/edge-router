//! Prometheus metrics middleware for observability

use prometheus::{
    Counter, CounterVec, HistogramVec, Registry, Encoder, TextEncoder,
    Opts,
};
use std::sync::Arc;
use anyhow::Result;
use tracing::debug;
use crate::middleware::{Middleware, MiddlewareContext};

/// Prometheus metrics collector for HTTP requests
pub struct MetricsCollector {
    /// Total HTTP requests received
    pub http_requests_total: CounterVec,
    /// HTTP request duration in seconds
    pub http_request_duration_seconds: HistogramVec,
    /// HTTP responses by status code
    pub http_responses_total: CounterVec,
    /// HTTP errors total
    pub http_errors_total: Counter,
    /// Request body size in bytes
    pub http_request_size_bytes: HistogramVec,
    /// Response body size in bytes
    pub http_response_size_bytes: HistogramVec,
    /// Prometheus registry for metrics
    pub registry: Arc<Registry>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Result<Self> {
        let registry = Arc::new(Registry::new());

        // Create metrics
        let http_requests_total = CounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path"],
        )?;

        let http_request_duration_seconds = HistogramVec::new(
            Opts::new(
                "http_request_duration_seconds",
                "HTTP request latency in seconds",
            )
            .into(),
            &["method", "path"],
        )?;

        let http_responses_total = CounterVec::new(
            Opts::new("http_responses_total", "Total HTTP responses by status"),
            &["status"],
        )?;

        let http_errors_total = Counter::new(
            "http_errors_total",
            "Total HTTP errors",
        )?;

        let http_request_size_bytes = HistogramVec::new(
            Opts::new(
                "http_request_size_bytes",
                "HTTP request size in bytes",
            )
            .into(),
            &["method"],
        )?;

        let http_response_size_bytes = HistogramVec::new(
            Opts::new(
                "http_response_size_bytes",
                "HTTP response size in bytes",
            )
            .into(),
            &["status"],
        )?;

        // Register metrics
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;
        registry.register(Box::new(http_responses_total.clone()))?;
        registry.register(Box::new(http_errors_total.clone()))?;
        registry.register(Box::new(http_request_size_bytes.clone()))?;
        registry.register(Box::new(http_response_size_bytes.clone()))?;

        Ok(Self {
            http_requests_total,
            http_request_duration_seconds,
            http_responses_total,
            http_errors_total,
            http_request_size_bytes,
            http_response_size_bytes,
            registry,
        })
    }

    /// Gather all metrics in Prometheus text format
    pub fn gather(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create default MetricsCollector")
    }
}

impl Clone for MetricsCollector {
    fn clone(&self) -> Self {
        // Share the same registry and metrics across clones
        Self {
            http_requests_total: self.http_requests_total.clone(),
            http_request_duration_seconds: self.http_request_duration_seconds.clone(),
            http_responses_total: self.http_responses_total.clone(),
            http_errors_total: self.http_errors_total.clone(),
            http_request_size_bytes: self.http_request_size_bytes.clone(),
            http_response_size_bytes: self.http_response_size_bytes.clone(),
            registry: self.registry.clone(),
        }
    }
}

/// Prometheus metrics middleware
pub struct MetricsMiddleware {
    pub collector: MetricsCollector,
}

impl MetricsMiddleware {
    /// Create a new metrics middleware
    pub fn new(collector: MetricsCollector) -> Self {
        Self { collector }
    }
}

#[async_trait::async_trait]
impl Middleware for MetricsMiddleware {
    fn name(&self) -> &'static str {
        "MetricsMiddleware"
    }

    async fn on_request(&self, context: &MiddlewareContext) -> Result<()> {
        debug!("Recording request metrics for {} {}", context.method, context.path);

        // Increment total requests counter
        self.collector
            .http_requests_total
            .with_label_values(&[&context.method, &context.path])
            .inc();

        // Record start time for latency measurement
        context.set_metadata(
            "metrics_start_time".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs_f64()
                .to_string(),
        );

        Ok(())
    }

    async fn on_response(
        &self,
        context: &MiddlewareContext,
        status: u16,
    ) -> Result<()> {
        debug!("Recording response metrics for {} {} -> {}", context.method, context.path, status);

        // Record response status
        self.collector
            .http_responses_total
            .with_label_values(&[&status.to_string()])
            .inc();

        // Calculate and record request duration
        if let Some(start_time_str) = context.get_metadata("metrics_start_time") {
            if let Ok(start_time) = start_time_str.parse::<f64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs_f64();
                let duration = now - start_time;
                self.collector
                    .http_request_duration_seconds
                    .with_label_values(&[&context.method, &context.path])
                    .observe(duration);
            }
        }

        Ok(())
    }

    async fn on_error(&self, context: &MiddlewareContext, error: &str) -> Result<()> {
        debug!("Recording error metrics for {} {}: {}", context.method, context.path, error);

        // Increment error counter
        self.collector.http_errors_total.inc();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        assert!(collector.gather().is_ok());
    }

    #[test]
    fn test_metrics_collector_default() {
        let collector = MetricsCollector::default();
        let metrics = collector.gather().expect("Failed to gather metrics");
        assert!(!metrics.is_empty());
        // Verify text format headers exist
        assert!(metrics.contains("# HELP"));
    }

    #[test]
    fn test_metrics_collector_clone() {
        let collector1 = MetricsCollector::new().expect("Failed to create collector");
        let collector2 = collector1.clone();

        // Both should share the same registry
        assert!(collector1.gather().is_ok());
        assert!(collector2.gather().is_ok());
    }

    #[tokio::test]
    async fn test_metrics_middleware_creation() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        let middleware = MetricsMiddleware::new(collector);
        assert_eq!(middleware.name(), "MetricsMiddleware");
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_request() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        let middleware = MetricsMiddleware::new(collector);

        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = middleware.on_request(&context).await;
        assert!(result.is_ok());
        assert!(context.get_metadata("metrics_start_time").is_some());

        // Verify request counter was incremented
        let metrics = middleware.collector.gather().expect("Failed to gather metrics");
        assert!(metrics.contains("http_requests_total"));
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_response() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        let middleware = MetricsMiddleware::new(collector);

        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: Some(200),
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        // Set start time
        context.set_metadata(
            "metrics_start_time".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
                .to_string(),
        );

        let result = middleware.on_response(&context, 200).await;
        assert!(result.is_ok());

        // Verify metrics were recorded
        let metrics = middleware.collector.gather().expect("Failed to gather metrics");
        assert!(metrics.contains("http_responses_total"));
    }

    #[tokio::test]
    async fn test_metrics_middleware_on_error() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        let middleware = MetricsMiddleware::new(collector);

        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = middleware.on_error(&context, "Test error").await;
        assert!(result.is_ok());

        // Verify error counter was incremented
        let metrics = middleware.collector.gather().expect("Failed to gather metrics");
        assert!(metrics.contains("http_errors_total"));
    }

    #[test]
    fn test_metrics_text_format_structure() {
        let collector = MetricsCollector::new().expect("Failed to create collector");
        // Increment a metric so it appears in output
        collector
            .http_requests_total
            .with_label_values(&["GET", "/test"])
            .inc();

        let metrics = collector.gather().expect("Failed to gather metrics");

        // Verify Prometheus text format
        assert!(metrics.contains("# HELP"));
        assert!(metrics.contains("# TYPE"));
        assert!(metrics.contains("http_requests_total"));
    }
}
