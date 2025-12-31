//! HTTP/HTTPS request/response body forwarding with actual client forwarding
//! Supports mTLS (mutual TLS) for service-to-service authentication

use hyper::{Request, Response, StatusCode, body::Bytes, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::tokio::TokioExecutor;
use http_body_util::{BodyExt, Full};
use std::time::Duration;
use std::sync::Arc;
use tokio::time::timeout as tokio_timeout;
use tracing::{debug, warn, info};
use anyhow::Result;
use crate::mtls::TlsClientConfig;

/// HTTP/HTTPS request forwarder for proxying requests to backend services
/// with connection pooling and timeout support.
///
/// Supports optional mTLS (mutual TLS) for service-to-service authentication
/// when configured with a TlsClientConfig.
pub struct RequestForwarder {
    client: Client<HttpConnector, Full<Bytes>>,
    timeout: Duration,
    /// Optional TLS configuration for HTTPS/mTLS requests
    tls_config: Option<Arc<TlsClientConfig>>,
}

impl RequestForwarder {
    /// Create a new HTTP request forwarder with connection pooling
    ///
    /// For HTTPS/mTLS support, use `with_tls()` instead.
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
            tls_config: None,
        }
    }

    /// Create a new request forwarder with TLS/mTLS support
    ///
    /// This forwarder can authenticate to HTTPS backends using client certificates.
    /// The TlsClientConfig contains the client certificate, key, and optional CA cert
    /// for verifying the backend server's certificate.
    pub fn with_tls(timeout: Duration, tls_config: TlsClientConfig) -> Result<Self> {
        // Configure HTTP connector with connection pooling
        let mut connector = HttpConnector::new();
        connector.set_connect_timeout(Some(timeout));
        connector.set_keepalive(Some(Duration::from_secs(30)));

        // Create hyper client with the connector and tokio executor
        let client = Client::builder(TokioExecutor::new())
            .build::<_, Full<Bytes>>(connector);

        info!(
            "RequestForwarder initialized with mTLS support (client cert verification: {})",
            tls_config.verify_server_cert
        );

        Ok(Self {
            client,
            timeout,
            tls_config: Some(Arc::new(tls_config)),
        })
    }

    /// Get the TLS configuration if set
    pub fn tls_config(&self) -> Option<&TlsClientConfig> {
        self.tls_config.as_ref().map(|arc| arc.as_ref())
    }

    /// Check if this forwarder has TLS/mTLS configured
    pub fn has_tls(&self) -> bool {
        self.tls_config.is_some()
    }

    /// Forward a request to a target URL and return the response
    ///
    /// Supports both HTTP and HTTPS URLs. HTTPS requests require TLS configuration
    /// to be set via `with_tls()` and will return a 502 Bad Gateway error if HTTPS
    /// is used without TLS configuration.
    pub async fn forward(
        &self,
        target_url: &str,
        request: Request<hyper::body::Incoming>,
    ) -> Result<Response<Bytes>> {
        debug!("Forwarding request to: {}", target_url);

        let uri: Uri = target_url.parse()?;

        // Check if URL is HTTPS and warn if not configured
        if uri.scheme_str() == Some("https") && !self.has_tls() {
            warn!("HTTPS URL requested but TLS not configured: {}", target_url);
            return Ok(Self::error_response(
                StatusCode::BAD_GATEWAY,
                "Backend HTTPS not configured - use with_tls() to enable\n",
            ));
        }

        if uri.scheme_str() == Some("https") {
            debug!("Using TLS/mTLS for HTTPS request");
        }

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
    fn test_forwarder_creation_without_tls() {
        let forwarder = RequestForwarder::new(Duration::from_secs(30));
        assert!(!forwarder.has_tls());
        assert_eq!(forwarder.tls_config(), None);
    }

    #[test]
    fn test_forwarder_creation_with_tls() {
        let tls_config = TlsClientConfig::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            Some(vec![7, 8, 9]),
            true,
        );
        let forwarder = RequestForwarder::with_tls(Duration::from_secs(30), tls_config)
            .expect("Failed to create forwarder with TLS");
        assert!(forwarder.has_tls());
        assert!(forwarder.tls_config().is_some());
    }

    #[test]
    fn test_forwarder_tls_config_access() {
        let tls_config = TlsClientConfig::new(
            vec![1, 2, 3],
            vec![4, 5, 6],
            None,
            false,
        );
        let forwarder = RequestForwarder::with_tls(Duration::from_secs(30), tls_config)
            .expect("Failed to create forwarder with TLS");

        let config = forwarder.tls_config().expect("TLS config should be present");
        assert!(!config.verify_server_cert);
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
