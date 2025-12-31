//! HTTP/gRPC proxy implementation
pub mod http;
pub mod load_balancer;
pub mod health_check;
pub mod policy;
pub mod forwarder;
pub mod tls;
pub mod mtls;
pub mod middleware;
pub mod metrics;
pub mod tracing;

pub use http::HttpProxy;
pub use load_balancer::LoadBalancer;
pub use health_check::{HealthChecker, HealthCheckConfig, HealthCheckMonitor};
pub use policy::{
    TimeoutPolicy, RetryPolicy, CircuitBreaker, CircuitBreakerConfig,
    CircuitState, TrafficPolicy
};
pub use forwarder::RequestForwarder;
pub use tls::{TlsServerConfig, CertificateMaterial};
pub use mtls::{
    ClientAuthMode, TlsClientConfig, MtlsClientVerifier,
    CertificateMetadata, CertificatePinner, CertificateValidationResult,
    calculate_cert_fingerprint
};
pub use middleware::{Middleware, MiddlewareChain, MiddlewareContext, LoggingMiddleware, HeaderInspectionMiddleware};
pub use metrics::{MetricsCollector, MetricsMiddleware};
pub use tracing::TracingMiddleware;
