//! HTTP proxy implementation with request forwarding

use hyper::{Response, StatusCode, body::Bytes};
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

    /// Create a 404 Not Found response
    pub fn not_found_response(reason: &str) -> Response<Bytes> {
        let body = Bytes::from(format!("Not Found: {}\n", reason));
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(body)
            .unwrap()
    }
}
