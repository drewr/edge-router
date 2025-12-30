//! HTTP proxy implementation with request forwarding

use hyper::{Response, StatusCode, body::Bytes, Request};
use router_core::{ServiceRegistry, Endpoint};
use std::sync::Arc;
use tracing::debug;
use anyhow::Result;

/// HTTP proxy for forwarding requests to backend services
pub struct HttpProxy {
    registry: Arc<ServiceRegistry>,
}

impl HttpProxy {
    /// Create a new HTTP proxy with a service registry
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }

    /// Get the service registry
    pub fn registry(&self) -> &Arc<ServiceRegistry> {
        &self.registry
    }

    /// Get the endpoint to use for routing to a service
    pub async fn get_endpoint(
        &self,
        namespace: &str,
        service_name: &str,
    ) -> Result<Endpoint> {
        // Build the service ID (namespace/name)
        let service_id = format!("{}/{}", namespace, service_name);

        // Get endpoints for the service
        let endpoints = self.registry.get_endpoints(&service_id).await?;

        if endpoints.is_empty() {
            return Err(anyhow::anyhow!("No endpoints available for service: {}", service_id));
        }

        // Use the first endpoint (simple selection; load balancer can override this)
        let endpoint = endpoints[0].clone();

        // Check if endpoint is ready
        if !endpoint.ready {
            return Err(anyhow::anyhow!("No ready endpoints for service: {}", service_id));
        }

        debug!("Selected endpoint for {}: {}:{}", service_id, endpoint.ip, endpoint.port);

        Ok(endpoint)
    }

    /// Build a target URL for an endpoint
    pub fn build_target_url(endpoint: &Endpoint, path: &str) -> String {
        format!(
            "http://{}:{}{}",
            endpoint.ip,
            endpoint.port,
            path
        )
    }

    /// Build a proxy request to a backend service
    /// This constructs a new request with the target endpoint
    pub fn build_proxy_request(
        endpoint: &Endpoint,
        original_req: &Request<hyper::body::Incoming>,
    ) -> Result<String> {
        let path = original_req.uri().path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let url = Self::build_target_url(endpoint, path);

        debug!("Building proxy request to: {}", url);
        Ok(url)
    }

    /// Create a 502 Bad Gateway response
    pub fn bad_gateway_response(reason: &str) -> Response<Bytes> {
        let body = Bytes::from(format!("Bad Gateway: {}\n", reason));
        Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(body)
            .unwrap()
    }

    /// Create a 503 Service Unavailable response
    pub fn service_unavailable_response(reason: &str) -> Response<Bytes> {
        let body = Bytes::from(format!("Service Unavailable: {}\n", reason));
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(body)
            .unwrap()
    }

    /// Create a 504 Gateway Timeout response
    pub fn gateway_timeout_response(reason: &str) -> Response<Bytes> {
        let body = Bytes::from(format!("Gateway Timeout: {}\n", reason));
        Response::builder()
            .status(StatusCode::GATEWAY_TIMEOUT)
            .body(body)
            .unwrap()
    }

    /// Create a 404 Not Found response
    pub fn not_found_response(reason: &str) -> Response<Bytes> {
        let body = Bytes::from(format!("Not Found: {}\n", reason));
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(body)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use router_core::ServiceRegistry;

    #[test]
    fn test_build_target_url() {
        let endpoint = Endpoint {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            ready: true,
        };

        let url = HttpProxy::build_target_url(&endpoint, "/api/v1/users");
        assert_eq!(url, "http://10.0.0.1:8080/api/v1/users");
    }

    #[test]
    fn test_build_target_url_with_root_path() {
        let endpoint = Endpoint {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            ready: true,
        };

        let url = HttpProxy::build_target_url(&endpoint, "/");
        assert_eq!(url, "http://10.0.0.1:8080/");
    }

    #[test]
    fn test_response_builders() {
        let bad_gw = HttpProxy::bad_gateway_response("Connection failed");
        assert_eq!(bad_gw.status(), StatusCode::BAD_GATEWAY);

        let svc_unavail = HttpProxy::service_unavailable_response("No endpoints");
        assert_eq!(svc_unavail.status(), StatusCode::SERVICE_UNAVAILABLE);

        let timeout = HttpProxy::gateway_timeout_response("Request timeout");
        assert_eq!(timeout.status(), StatusCode::GATEWAY_TIMEOUT);

        let not_found = HttpProxy::not_found_response("Route not found");
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    }
}
