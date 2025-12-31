use anyhow::Result;
use hyper::{
    body::Bytes,
    server::conn::http1,
    service::service_fn,
    Request, Response, StatusCode,
};
use hyper_util::rt::tokio::TokioIo;
use http_body_util::Full;
use router_core::ServiceRegistry;
use router_proxy::{HttpProxy, HealthCheckConfig, HealthChecker, TrafficPolicy, RequestForwarder};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, debug};
use tracing_subscriber::fmt::init as tracing_init;

mod router;

use router::Router;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init();

    info!("Starting router-gateway...");

    // Create service registry
    let registry = Arc::new(ServiceRegistry::new());
    info!("Service registry initialized");

    // Create HTTP proxy
    let proxy = Arc::new(HttpProxy::new(registry.clone()));
    info!("HTTP proxy initialized");

    // Create router
    let router = Arc::new(Router::new(registry.clone()));
    info!("Router initialized");

    // Initialize health checker
    let health_check_config = HealthCheckConfig {
        http_path: "/healthz".to_string(),
        check_interval: Duration::from_secs(10),
        timeout: Duration::from_secs(5),
        unhealthy_threshold: 3,
        healthy_threshold: 2,
    };
    let _health_checker = Arc::new(HealthChecker::new(health_check_config));
    info!("Health checker initialized");

    // Initialize traffic policy
    let _traffic_policy = Arc::new(TrafficPolicy::default());
    info!("Traffic policy initialized");
    info!("  - Timeout: {:?}", _traffic_policy.timeout.request_timeout);
    info!("  - Max Retries: {}", _traffic_policy.retry.max_retries);
    info!("  - Circuit Breaker Failure Threshold: {}", _traffic_policy.circuit_breaker.failure_threshold);

    // Initialize request forwarder
    let forwarder = Arc::new(RequestForwarder::new(Duration::from_secs(30)));
    info!("Request forwarder initialized with 30s timeout");

    // Start the HTTP server on port 8080
    let addr: SocketAddr = ([0, 0, 0, 0], 8080).into();
    let listener = TcpListener::bind(&addr).await?;

    info!("HTTP server listening on {}", addr);

    // Accept connections in a loop
    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let proxy = proxy.clone();
        let router = router.clone();
        let forwarder = forwarder.clone();

        tokio::task::spawn(async move {
            let service = service_fn(move |req| {
                let proxy = proxy.clone();
                let router = router.clone();
                let forwarder = forwarder.clone();
                handle_request(req, proxy, router, forwarder)
            });

            if let Err(e) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                debug!("Error serving connection from {}: {}", peer_addr, e);
            }
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    _proxy: Arc<HttpProxy>,
    _router: Arc<Router>,
    forwarder: Arc<RequestForwarder>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    debug!("{} {}", method, path);

    // Health check endpoint
    if path == "/healthz" {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("OK\n")))
            .unwrap());
    }

    // Route the request based on VPCRoute rules
    // Phase 2: Basic routing is available in Router module
    // Phase 3: Health checks and policies are ready
    // Phase 4: Now using RequestForwarder for actual HTTP forwarding

    debug!("Processing request: {} {}", method, path);

    // Use forwarder to forward the request
    // For now, it returns a 503 until full HTTP client is integrated
    match forwarder.forward("http://backend-service:8080", req).await {
        Ok(response) => {
            // Convert response body to Full<Bytes>
            let (parts, body) = response.into_parts();
            Ok(Response::from_parts(parts, Full::new(body)))
        }
        Err(e) => {
            debug!("Forwarder error: {}", e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Internal Server Error\n")))
                .unwrap())
        }
    }
}
