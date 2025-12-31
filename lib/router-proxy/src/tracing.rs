//! OpenTelemetry distributed tracing middleware

use std::collections::HashMap;
use anyhow::Result;
use tracing::{info, error};
use crate::middleware::{Middleware, MiddlewareContext};

/// Distributed tracing middleware using tracing and OpenTelemetry
pub struct TracingMiddleware {
    /// Service name for traces
    pub service_name: String,
}

impl TracingMiddleware {
    /// Create a new tracing middleware
    pub fn new() -> Self {
        Self {
            service_name: "datum-router".to_string(),
        }
    }

    /// Create a new tracing middleware with custom service name
    pub fn with_service_name(service_name: String) -> Self {
        Self { service_name }
    }

    /// Extract W3C Trace Context from request headers
    /// Returns (trace_id, span_id, trace_flags) if present
    pub fn extract_w3c_trace_context(headers: &HashMap<String, String>) -> Option<(String, String, String)> {
        // W3C Trace Context header format: version-trace_id-span_id-trace_flags
        headers.get("traceparent").and_then(|v| {
            let parts: Vec<&str> = v.split('-').collect();
            if parts.len() >= 4 {
                Some((parts[1].to_string(), parts[2].to_string(), parts[3].to_string()))
            } else {
                None
            }
        })
    }

    /// Create W3C Trace Context header value
    pub fn create_w3c_trace_context(trace_id: &str, span_id: &str, trace_flags: &str) -> String {
        format!("00-{}-{}-{}", trace_id, span_id, trace_flags)
    }

    /// Generate a new span ID (random 16 hex digits)
    pub fn generate_span_id() -> String {
        use std::fmt::Write;
        let mut id = String::with_capacity(16);
        for _ in 0..8 {
            write!(&mut id, "{:02x}", rand::random::<u8>()).ok();
        }
        id
    }

    /// Generate a new trace ID (random 32 hex digits)
    pub fn generate_trace_id() -> String {
        use std::fmt::Write;
        let mut id = String::with_capacity(32);
        for _ in 0..16 {
            write!(&mut id, "{:02x}", rand::random::<u8>()).ok();
        }
        id
    }
}

impl Default for TracingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Middleware for TracingMiddleware {
    fn name(&self) -> &'static str {
        "TracingMiddleware"
    }

    async fn on_request(&self, context: &MiddlewareContext) -> Result<()> {
        // Extract trace context from incoming headers
        let (trace_id, span_id) = if let Some((t_id, s_id, _)) = Self::extract_w3c_trace_context(&context.request_headers) {
            (t_id, s_id)
        } else {
            // Create new trace if not present
            (Self::generate_trace_id(), Self::generate_span_id())
        };

        // Store trace context for response
        context.set_metadata("trace_id".to_string(), trace_id.clone());
        context.set_metadata("span_id".to_string(), span_id);

        // Log request with trace context
        info!(
            trace_id = %trace_id,
            method = %context.method,
            path = %context.path,
            "Request started"
        );

        Ok(())
    }

    async fn on_response(
        &self,
        context: &MiddlewareContext,
        status: u16,
    ) -> Result<()> {
        let trace_id = context.get_metadata("trace_id").unwrap_or_default();

        info!(
            trace_id = %trace_id,
            status = status,
            method = %context.method,
            path = %context.path,
            "Request completed"
        );

        Ok(())
    }

    async fn on_error(&self, context: &MiddlewareContext, error: &str) -> Result<()> {
        let trace_id = context.get_metadata("trace_id").unwrap_or_default();

        error!(
            trace_id = %trace_id,
            error = %error,
            method = %context.method,
            path = %context.path,
            "Request error"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_middleware_creation() {
        let middleware = TracingMiddleware::new();
        assert_eq!(middleware.name(), "TracingMiddleware");
        assert_eq!(middleware.service_name, "datum-router");
    }

    #[test]
    fn test_tracing_middleware_with_service_name() {
        let middleware = TracingMiddleware::with_service_name("custom-service".to_string());
        assert_eq!(middleware.service_name, "custom-service");
    }

    #[test]
    fn test_tracing_middleware_default() {
        let middleware = TracingMiddleware::default();
        assert_eq!(middleware.name(), "TracingMiddleware");
    }

    #[test]
    fn test_extract_w3c_trace_context_valid() {
        let mut headers = HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-0af7651916cd43dd-b7ad6b7169203331-01".to_string(),
        );

        let result = TracingMiddleware::extract_w3c_trace_context(&headers);
        assert!(result.is_some());

        let (trace_id, span_id, flags) = result.unwrap();
        assert_eq!(trace_id, "0af7651916cd43dd");
        assert_eq!(span_id, "b7ad6b7169203331");
        assert_eq!(flags, "01");
    }

    #[test]
    fn test_extract_w3c_trace_context_missing() {
        let headers = HashMap::new();
        let result = TracingMiddleware::extract_w3c_trace_context(&headers);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_w3c_trace_context_invalid() {
        let mut headers = HashMap::new();
        headers.insert("traceparent".to_string(), "invalid-format".to_string());

        let result = TracingMiddleware::extract_w3c_trace_context(&headers);
        assert!(result.is_none());
    }

    #[test]
    fn test_create_w3c_trace_context() {
        let trace_id = "0af7651916cd43dd";
        let span_id = "b7ad6b7169203331";
        let flags = "01";

        let header = TracingMiddleware::create_w3c_trace_context(trace_id, span_id, flags);
        assert_eq!(header, "00-0af7651916cd43dd-b7ad6b7169203331-01");
    }

    #[test]
    fn test_generate_span_id() {
        let span_id1 = TracingMiddleware::generate_span_id();
        let span_id2 = TracingMiddleware::generate_span_id();

        // Should be 16 hex digits
        assert_eq!(span_id1.len(), 16);
        assert_eq!(span_id2.len(), 16);

        // Should be different
        assert_ne!(span_id1, span_id2);

        // Should be valid hex
        assert!(u64::from_str_radix(&span_id1, 16).is_ok());
    }

    #[test]
    fn test_generate_trace_id() {
        let trace_id1 = TracingMiddleware::generate_trace_id();
        let trace_id2 = TracingMiddleware::generate_trace_id();

        // Should be 32 hex digits
        assert_eq!(trace_id1.len(), 32);
        assert_eq!(trace_id2.len(), 32);

        // Should be different
        assert_ne!(trace_id1, trace_id2);

        // Should be valid hex
        assert!(u128::from_str_radix(&trace_id1, 16).is_ok());
    }

    #[tokio::test]
    async fn test_tracing_middleware_on_request() {
        let middleware = TracingMiddleware::new();
        let context = MiddlewareContext {
            path: "/api/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = middleware.on_request(&context).await;
        assert!(result.is_ok());

        // Verify trace context was created
        assert!(context.get_metadata("trace_id").is_some());
        assert!(context.get_metadata("span_id").is_some());
    }

    #[tokio::test]
    async fn test_tracing_middleware_on_request_with_header() {
        let middleware = TracingMiddleware::new();
        let mut request_headers = HashMap::new();
        request_headers.insert(
            "traceparent".to_string(),
            "00-0af7651916cd43dd-b7ad6b7169203331-01".to_string(),
        );

        let context = MiddlewareContext {
            path: "/api/test".to_string(),
            method: "GET".to_string(),
            request_headers,
            response_status: None,
            response_headers: HashMap::new(),
            metadata: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = middleware.on_request(&context).await;
        assert!(result.is_ok());

        // Verify trace context was extracted from header
        let trace_id = context.get_metadata("trace_id");
        assert_eq!(trace_id, Some("0af7651916cd43dd".to_string()));
    }

    #[tokio::test]
    async fn test_tracing_middleware_on_response() {
        let middleware = TracingMiddleware::new();
        let context = MiddlewareContext {
            path: "/api/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: Some(200),
            response_headers: HashMap::new(),
            metadata: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        // Set trace_id as would be set by on_request
        context.set_metadata("trace_id".to_string(), "test-trace-id".to_string());

        let result = middleware.on_response(&context, 200).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tracing_middleware_on_error() {
        let middleware = TracingMiddleware::new();
        let context = MiddlewareContext {
            path: "/api/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = middleware.on_error(&context, "Test error").await;
        assert!(result.is_ok());
    }
}
