//! Middleware framework for extensible request/response processing

use hyper::{Request, Response, body::Bytes};
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tracing::{debug, span, Level};

/// Context passed through middleware chain
#[derive(Clone)]
pub struct MiddlewareContext {
    /// Request path
    pub path: String,
    /// Request method
    pub method: String,
    /// Request headers
    pub request_headers: HashMap<String, String>,
    /// Response status code (set after response)
    pub response_status: Option<u16>,
    /// Response headers (set after response)
    pub response_headers: HashMap<String, String>,
    /// Custom metadata for middleware
    pub metadata: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

impl MiddlewareContext {
    /// Create a new middleware context from a request
    pub fn from_request(req: &Request<hyper::body::Incoming>) -> Self {
        let mut headers = HashMap::new();
        for (k, v) in req.headers() {
            if let Ok(v_str) = v.to_str() {
                headers.insert(k.to_string(), v_str.to_string());
            }
        }

        Self {
            path: req.uri().path().to_string(),
            method: req.method().to_string(),
            request_headers: headers,
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Get a metadata value
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata
            .lock()
            .ok()
            .and_then(|m| m.get(key).cloned())
    }

    /// Set a metadata value
    pub fn set_metadata(&self, key: String, value: String) {
        if let Ok(mut m) = self.metadata.lock() {
            m.insert(key, value);
        }
    }
}

/// Middleware trait for processing requests and responses
#[async_trait::async_trait]
pub trait Middleware: Send + Sync {
    /// Hook called before processing request (name for logging)
    fn name(&self) -> &'static str {
        "UnnamedMiddleware"
    }

    /// Called before request is processed
    async fn on_request(&self, _context: &MiddlewareContext) -> Result<()> {
        Ok(())
    }

    /// Called after response is ready (status and headers available)
    async fn on_response(
        &self,
        _context: &MiddlewareContext,
        _status: u16,
    ) -> Result<()> {
        Ok(())
    }

    /// Called on error
    async fn on_error(&self, _context: &MiddlewareContext, _error: &str) -> Result<()> {
        Ok(())
    }
}

/// Chain of middleware to execute in order
pub struct MiddlewareChain {
    middleware: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Create a new middleware chain
    pub fn new() -> Self {
        Self {
            middleware: Vec::new(),
        }
    }

    /// Add middleware to the chain
    pub fn add<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middleware.push(Arc::new(middleware));
        self
    }

    /// Process request through all middleware
    pub async fn on_request(&self, context: &MiddlewareContext) -> Result<()> {
        for mw in &self.middleware {
            let span = span!(Level::DEBUG, "middleware", name = mw.name());
            let _guard = span.enter();
            debug!("Processing on_request");
            mw.on_request(context).await?;
        }
        Ok(())
    }

    /// Process response through all middleware (in reverse order)
    pub async fn on_response(
        &self,
        context: &MiddlewareContext,
        status: u16,
    ) -> Result<()> {
        for mw in self.middleware.iter().rev() {
            let span = span!(Level::DEBUG, "middleware", name = mw.name());
            let _guard = span.enter();
            debug!("Processing on_response");
            mw.on_response(context, status).await?;
        }
        Ok(())
    }

    /// Process error through all middleware
    pub async fn on_error(&self, context: &MiddlewareContext, error: &str) -> Result<()> {
        for mw in &self.middleware {
            let span = span!(Level::DEBUG, "middleware", name = mw.name());
            let _guard = span.enter();
            debug!("Processing on_error");
            mw.on_error(context, error).await?;
        }
        Ok(())
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Logging middleware that logs request and response info
pub struct LoggingMiddleware;

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    fn name(&self) -> &'static str {
        "LoggingMiddleware"
    }

    async fn on_request(&self, context: &MiddlewareContext) -> Result<()> {
        debug!(
            "Request: {} {} (headers: {})",
            context.method,
            context.path,
            context.request_headers.len()
        );
        context.set_metadata("start_time".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
                .to_string());
        Ok(())
    }

    async fn on_response(
        &self,
        context: &MiddlewareContext,
        status: u16,
    ) -> Result<()> {
        let duration = if let Some(start_time) = context.get_metadata("start_time") {
            if let Ok(start) = start_time.parse::<u128>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis();
                now.saturating_sub(start)
            } else {
                0
            }
        } else {
            0
        };

        debug!(
            "Response: {} {} -> {} (duration: {}ms)",
            context.method,
            context.path,
            status,
            duration
        );
        Ok(())
    }

    async fn on_error(&self, context: &MiddlewareContext, error: &str) -> Result<()> {
        debug!("Error: {} {} - {}", context.method, context.path, error);
        Ok(())
    }
}

/// Header inspection middleware that logs specific headers
pub struct HeaderInspectionMiddleware {
    pub headers_to_log: Vec<String>,
}

impl HeaderInspectionMiddleware {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers_to_log: headers,
        }
    }
}

#[async_trait::async_trait]
impl Middleware for HeaderInspectionMiddleware {
    fn name(&self) -> &'static str {
        "HeaderInspectionMiddleware"
    }

    async fn on_request(&self, context: &MiddlewareContext) -> Result<()> {
        for header in &self.headers_to_log {
            if let Some(value) = context.request_headers.get(header) {
                debug!("Request header {}: {}", header, value);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_context_creation() {
        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };
        assert_eq!(context.path, "/test");
        assert_eq!(context.method, "GET");
    }

    #[test]
    fn test_middleware_context_metadata() {
        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        context.set_metadata("key1".to_string(), "value1".to_string());
        assert_eq!(context.get_metadata("key1"), Some("value1".to_string()));
        assert_eq!(context.get_metadata("key2"), None);
    }

    #[tokio::test]
    async fn test_middleware_chain() {
        let chain = MiddlewareChain::default();
        let context = MiddlewareContext {
            path: "/test".to_string(),
            method: "GET".to_string(),
            request_headers: HashMap::new(),
            response_status: None,
            response_headers: HashMap::new(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        };

        let result = chain.on_request(&context).await;
        assert!(result.is_ok());

        let result = chain.on_response(&context, 200).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let middleware = LoggingMiddleware;
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
        assert!(context.get_metadata("start_time").is_some());
    }

    #[test]
    fn test_header_inspection_middleware_creation() {
        let middleware = HeaderInspectionMiddleware::new(vec![
            "content-type".to_string(),
            "authorization".to_string(),
        ]);
        assert_eq!(middleware.name(), "HeaderInspectionMiddleware");
        assert_eq!(middleware.headers_to_log.len(), 2);
    }
}
