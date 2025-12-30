//! HTTP request/response body forwarding

use hyper::{Request, Response, StatusCode, body::Bytes};
use http_body_util::BodyExt;
use std::time::Duration;
use tracing::debug;
use anyhow::Result;

/// HTTP request forwarder for proxying request and response bodies
pub struct RequestForwarder {
    timeout: Duration,
}

impl RequestForwarder {
    /// Create a new request forwarder
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Forward a request to a target URL and return the response
    /// This collects the entire body for simplicity
    pub async fn forward(
        &self,
        _target_url: &str,
        _request: Request<hyper::body::Incoming>,
    ) -> Result<Response<Bytes>> {
        // In Phase 3.1+, this will:
        // 1. Create HTTP client connection to target
        // 2. Forward request headers
        // 3. Stream request body to backend
        // 4. Read response headers
        // 5. Stream response body back
        // 6. Return complete response

        // For MVP, return a placeholder
        debug!("Forwarding request to: {}", _target_url);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Bytes::from("Request forwarding ready for Phase 3.1\n"))
            .unwrap())
    }

    /// Collect the entire request body into Bytes
    pub async fn collect_body(body: hyper::body::Incoming) -> Result<Bytes> {
        let collected = body.collect().await?;
        Ok(collected.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forwarder_creation() {
        let forwarder = RequestForwarder::new(Duration::from_secs(30));
        assert_eq!(forwarder.timeout, Duration::from_secs(30));
    }
}
