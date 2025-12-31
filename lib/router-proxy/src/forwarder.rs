//! HTTP request/response body forwarding with actual client forwarding

use hyper::{Request, Response, StatusCode, body::Bytes, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::tokio::TokioExecutor;
use http_body_util::{BodyExt, Full};
use std::time::Duration;
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, warn};
use anyhow::Result;

/// HTTP request forwarder for proxying requests to backend services
/// with connection pooling and timeout support
pub struct RequestForwarder {
    client: Client<HttpConnector, Full<Bytes>>,
    timeout: Duration,
}

impl RequestForwarder {
    /// Create a new request forwarder with connection pooling
    pub fn new(timeout: Duration) -> Self {
        // Configure HTTP connector with connection pooling
        let mut connector = HttpConnector::new();
        connector.set_connect_timeout(Some(timeout));
        connector.set_keepalive(Some(Duration::from_secs(30)));

        // Create hyper client with the connector and tokio executor
        let client = Client::builder(TokioExecutor::new())
            .build::<_, Full<Bytes>>(connector);

        Self {
            client,
            timeout,
        }
    }

    /// Forward a request to a target URL and return the response
    pub async fn forward(
        &self,
        target_url: &str,
        request: Request<hyper::body::Incoming>,
    ) -> Result<Response<Bytes>> {
        debug!("Forwarding request to: {}", target_url);

        let uri: Uri = target_url.parse()?;

        // Collect request body
        let (mut parts, incoming) = request.into_parts();
        let body_bytes = Self::collect_body(incoming).await?;

        debug!(
            "Request details - method: {}, headers: {}",
            parts.method,
            parts.headers.len()
        );

        // Filter headers (skip hop-by-hop headers)
        let removed_count = parts
            .headers
            .iter()
            .filter(|(k, _)| Self::is_hop_by_hop_header(k.as_str().to_lowercase().as_str()))
            .count();

        debug!(
            "Filtered {} headers (removed hop-by-hop)",
            removed_count
        );

        // Remove hop-by-hop headers from the request
        let mut filtered_headers = hyper::header::HeaderMap::new();
        for (k, v) in parts.headers.iter() {
            if !Self::is_hop_by_hop_header(k.as_str().to_lowercase().as_str()) {
                filtered_headers.insert(k.clone(), v.clone());
            }
        }
        parts.headers = filtered_headers;

        // Update the URI to the target URL
        parts.uri = uri;

        // Build the forwarded request with the collected body
        let forwarded_request = Request::from_parts(parts, Full::new(body_bytes.clone()));

        debug!("Sending request to backend with {}s timeout", self.timeout.as_secs());

        // Send the request with timeout protection
        match tokio_timeout(self.timeout, self.client.request(forwarded_request)).await {
            Ok(Ok(response)) => {
                debug!("Backend responded with status: {}", response.status());

                // Collect response body
                let (response_parts, body) = response.into_parts();
                let response_bytes = Self::collect_body(body).await?;

                debug!("Response body size: {} bytes", response_bytes.len());

                Ok(Response::from_parts(response_parts, response_bytes))
            }
            Ok(Err(e)) => {
                warn!("Backend request error: {}", e);
                Ok(Self::error_response(
                    StatusCode::BAD_GATEWAY,
                    "Error communicating with backend service\n",
                ))
            }
            Err(_) => {
                warn!("Backend request timeout after {}s", self.timeout.as_secs());
                Ok(Self::error_response(
                    StatusCode::GATEWAY_TIMEOUT,
                    "Backend service request timeout\n",
                ))
            }
        }
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

    #[test]
    fn test_forwarder_creation_with_different_timeouts() {
        let forwarder_5s = RequestForwarder::new(Duration::from_secs(5));
        assert_eq!(forwarder_5s.timeout, Duration::from_secs(5));

        let forwarder_60s = RequestForwarder::new(Duration::from_secs(60));
        assert_eq!(forwarder_60s.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_error_response() {
        let response = RequestForwarder::error_response(StatusCode::BAD_GATEWAY, "Test error");
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
}
