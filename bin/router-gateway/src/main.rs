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
use router_proxy::{HttpProxy, HealthCheckConfig, HealthChecker, TrafficPolicy, RequestForwarder, TlsServerConfig, MiddlewareChain, LoggingMiddleware, HeaderInspectionMiddleware, MetricsCollector, MetricsMiddleware, TracingMiddleware};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{info, debug, warn};
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

    // Initialize metrics collector
    let metrics_collector = MetricsCollector::new()
        .expect("Failed to create metrics collector");
    let metrics_collector = Arc::new(metrics_collector);
    info!("Metrics collector initialized");

    // Initialize middleware chain
    let middleware = Arc::new(
        MiddlewareChain::new()
            .add(TracingMiddleware::new())
            .add(LoggingMiddleware)
            .add(HeaderInspectionMiddleware::new(vec![
                "content-type".to_string(),
                "authorization".to_string(),
                "user-agent".to_string(),
            ]))
            .add(MetricsMiddleware::new((*metrics_collector).clone()))
    );
    info!("Middleware chain initialized with tracing, logging, header inspection, and metrics");

    // Try to load TLS configuration from environment or default
    let tls_config = load_tls_config();
    let tls_acceptor = tls_config.as_ref().map(|config| {
        TlsAcceptor::from(config.config.clone())
    });

    // Start HTTP server on port 8080
    let http_addr: SocketAddr = ([0, 0, 0, 0], 8080).into();
    let http_listener = TcpListener::bind(&http_addr).await?;
    info!("HTTP server listening on {}", http_addr);

    // Optionally start HTTPS server on port 8443
    if tls_acceptor.is_some() {
        let https_addr: SocketAddr = ([0, 0, 0, 0], 8443).into();
        let https_listener = TcpListener::bind(&https_addr).await?;
        info!("HTTPS server listening on {} (TLS configured)", https_addr);

        let tls_acceptor = tls_acceptor.clone();
        let proxy = proxy.clone();
        let router = router.clone();
        let forwarder = forwarder.clone();
        let middleware = middleware.clone();
        let metrics_collector = metrics_collector.clone();

        tokio::task::spawn(accept_https_connections(
            https_listener,
            proxy,
            router,
            forwarder,
            middleware,
            metrics_collector,
            tls_acceptor.unwrap(),
        ));
    } else {
        warn!("TLS not configured - HTTPS listener not started");
        warn!("Set ROUTER_TLS_CERT and ROUTER_TLS_KEY environment variables to enable HTTPS");
    }

    // Accept HTTP connections in a loop
    loop {
        let (stream, peer_addr) = http_listener.accept().await?;
        let io = TokioIo::new(stream);

        let proxy = proxy.clone();
        let router = router.clone();
        let forwarder = forwarder.clone();
        let middleware = middleware.clone();
        let metrics_collector = metrics_collector.clone();

        tokio::task::spawn(async move {
            let service = service_fn(move |req| {
                let proxy = proxy.clone();
                let router = router.clone();
                let forwarder = forwarder.clone();
                let middleware = middleware.clone();
                let metrics_collector = metrics_collector.clone();
                handle_request(req, proxy, router, forwarder, middleware, metrics_collector)
            });

            if let Err(e) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                debug!("Error serving HTTP connection from {}: {}", peer_addr, e);
            }
        });
    }
}

/// Load TLS configuration from environment variables
fn load_tls_config() -> Option<TlsServerConfig> {
    let cert_path = std::env::var("ROUTER_TLS_CERT").ok();
    let key_path = std::env::var("ROUTER_TLS_KEY").ok();

    match (cert_path, key_path) {
        (Some(cert_path), Some(key_path)) => {
            match (
                std::fs::read(&cert_path),
                std::fs::read(&key_path),
            ) {
                (Ok(cert), Ok(key)) => {
                    match TlsServerConfig::from_pem(&cert, &key, None, None) {
                        Ok(config) => {
                            info!("TLS configuration loaded from {} and {}", cert_path, key_path);
                            Some(config)
                        }
                        Err(e) => {
                            warn!("Failed to parse TLS configuration: {}", e);
                            None
                        }
                    }
                }
                (cert_err, key_err) => {
                    if cert_err.is_err() {
                        warn!("Failed to read TLS certificate from {}", cert_path);
                    }
                    if key_err.is_err() {
                        warn!("Failed to read TLS key from {}", key_path);
                    }
                    None
                }
            }
        }
        _ => None,
    }
}

/// Accept HTTPS connections with TLS
async fn accept_https_connections(
    listener: TcpListener,
    proxy: Arc<HttpProxy>,
    router: Arc<Router>,
    forwarder: Arc<RequestForwarder>,
    middleware: Arc<MiddlewareChain>,
    metrics_collector: Arc<MetricsCollector>,
    tls_acceptor: TlsAcceptor,
) {
    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let tls_acceptor = tls_acceptor.clone();
                let proxy = proxy.clone();
                let router = router.clone();
                let forwarder = forwarder.clone();
                let middleware = middleware.clone();
                let metrics_collector = metrics_collector.clone();

                tokio::task::spawn(async move {
                    match tls_acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = TokioIo::new(tls_stream);
                            let service = service_fn(move |req| {
                                let proxy = proxy.clone();
                                let router = router.clone();
                                let forwarder = forwarder.clone();
                                let middleware = middleware.clone();
                                let metrics_collector = metrics_collector.clone();
                                handle_request(req, proxy, router, forwarder, middleware, metrics_collector)
                            });

                            if let Err(e) = http1::Builder::new()
                                .serve_connection(io, service)
                                .await
                            {
                                debug!("Error serving HTTPS connection from {}: {}", peer_addr, e);
                            }
                        }
                        Err(e) => {
                            debug!("TLS error from {}: {}", peer_addr, e);
                        }
                    }
                });
            }
            Err(e) => {
                warn!("Error accepting HTTPS connection: {}", e);
            }
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    _proxy: Arc<HttpProxy>,
    _router: Arc<Router>,
    forwarder: Arc<RequestForwarder>,
    middleware: Arc<MiddlewareChain>,
    metrics_collector: Arc<MetricsCollector>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    use router_proxy::MiddlewareContext;

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    debug!("{} {}", method, path);

    // Create middleware context
    let context = MiddlewareContext::from_request(&req);

    // Call on_request middleware hooks
    if let Err(e) = middleware.on_request(&context).await {
        debug!("Middleware on_request error: {}", e);
    }

    // Metrics endpoint
    if path == "/metrics" && method == "GET" {
        let metrics_text = metrics_collector
            .gather()
            .unwrap_or_else(|_| "Failed to gather metrics\n".to_string());
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain; version=0.0.4")
            .body(Full::new(Bytes::from(metrics_text)))
            .unwrap();

        if let Err(e) = middleware.on_response(&context, 200).await {
            debug!("Middleware on_response error: {}", e);
        }

        return Ok(response);
    }

    // Health check endpoint
    if path == "/healthz" {
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("OK\n")))
            .unwrap();

        if let Err(e) = middleware.on_response(&context, 200).await {
            debug!("Middleware on_response error: {}", e);
        }

        return Ok(response);
    }

    // Route the request based on VPCRoute rules
    // Phase 2: Basic routing is available in Router module
    // Phase 3: Health checks and policies are ready
    // Phase 4.2: Using RequestForwarder for actual HTTP forwarding
    // Phase 4.4: Middleware hooks integrated

    debug!("Processing request: {} {}", method, path);

    // Use forwarder to forward the request
    let result = match forwarder.forward("http://backend-service:8080", req).await {
        Ok(response) => {
            // Convert response body to Full<Bytes>
            let (parts, body) = response.into_parts();
            let status = parts.status.as_u16();
            let response = Response::from_parts(parts, Full::new(body));

            // Call on_response middleware hooks
            if let Err(e) = middleware.on_response(&context, status).await {
                debug!("Middleware on_response error: {}", e);
            }

            Ok(response)
        }
        Err(e) => {
            debug!("Forwarder error: {}", e);
            let error_response = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Internal Server Error\n")))
                .unwrap();

            // Call on_error middleware hooks
            if let Err(mw_err) = middleware.on_error(&context, &e.to_string()).await {
                debug!("Middleware on_error error: {}", mw_err);
            }

            // Call on_response middleware hooks for error response
            if let Err(e) = middleware.on_response(&context, 500).await {
                debug!("Middleware on_response error: {}", e);
            }

            Ok(error_response)
        }
    };

    result
}
