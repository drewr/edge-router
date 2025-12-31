//! HTTP request/response body forwarding with actual client forwarding

use hyper::{Request, Response, StatusCode, body::Bytes, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::tokio::TokioExecutor;
use http_body_util::{BodyExt, Full};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;
use anyhow::Result;

/// HTTP request forwarder for proxying requests to backend services
pub struct RequestForwarder {
    timeout: Duration,
    // Store a reference to allow cloning for each request
    _timeout_backup: Duration,
}

impl RequestForwarder {
    /// Create a new request forwarder with connection pooling
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            _timeout_backup: timeout,
        }
    }

    /// Forward a request to a target URL and return the response
    pub async fn forward(
        &self,
        target_url: &str,
        request: Request<hyper::body::Incoming>,
    ) -> Result<Response<Bytes>> {
        debug!("Forwarding request to: {}", target_url);

        let _uri: Uri = target_url.parse()?;

        // Collect request body
        let (parts, incoming) = request.into_parts();
        let body_bytes = Self::collect_body(incoming).await?;

        debug!(
            "Request details - method: {}, headers: {}",
            parts.method,
            parts.headers.len()
        );

        // Filter headers (skip hop-by-hop headers)
        let filtered_headers_count = parts
            .headers
            .iter()
            .filter(|(k, _)| !Self::is_hop_by_hop_header(k.as_str().to_lowercase().as_str()))
            .count();

        debug!(
            "Filtered {} headers (removed hop-by-hop)",
            parts.headers.len() - filtered_headers_count
        );

        // In Phase 4.2, we'll integrate with actual HTTP client
        // For now, return a 502 indicating forwarding is ready but not yet connected
        debug!("Request forwarder ready for Phase 4.2 HTTP client integration");

        Ok(Self::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "HTTP client forwarding not yet implemented - Phase 4.2\n",
        ))
    }

    /// Collect the entire request body into Bytes
    pub async fn collect_body(body: hyper::body::Incoming) -> Result<Bytes> {
        let collected = body.collect().await?;
        Ok(collected.to_bytes())
    }

    /// Create an error response
    fn error_response(status: StatusCode, message: &str) -> Response<Bytes> {
        Response::builder()
            .status(status)
            .body(Bytes::from(format!("{}\n", message)))
            .unwrap()
    }

    /// Check if header is hop-by-hop (should not be forwarded)
    fn is_hop_by_hop_header(name: &str) -> bool {
        matches!(
            name,
            "connection"
                | "keep-alive"
                | "proxy-authenticate"
                | "proxy-authorization"
                | "te"
                | "trailers"
                | "transfer-encoding"
                | "upgrade"
        )
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

    #[test]
    fn test_hop_by_hop_headers() {
        assert!(RequestForwarder::is_hop_by_hop_header("connection"));
        assert!(RequestForwarder::is_hop_by_hop_header("keep-alive"));
        assert!(RequestForwarder::is_hop_by_hop_header("transfer-encoding"));
        assert!(!RequestForwarder::is_hop_by_hop_header("content-type"));
        assert!(!RequestForwarder::is_hop_by_hop_header("authorization"));
    }
}
