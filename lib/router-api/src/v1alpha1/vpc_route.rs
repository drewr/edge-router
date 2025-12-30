use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPCRoute defines Layer 7 routing rules for traffic between VPCs
/// or from external clients to VPCServices
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "router.datum.net",
    version = "v1alpha1",
    kind = "VPCRoute",
    plural = "vpcroutes",
    derive = "Default",
    status = "VPCRouteStatus",
)]
pub struct VPCRouteSpec {
    /// Name of this route (for reference)
    pub name: String,

    /// Match conditions for routing
    pub r#match: RouteMatch,

    /// Destination service(s)
    pub destinations: Vec<RouteDestination>,

    /// Load balancing strategy
    #[serde(default = "default_load_balancing")]
    pub load_balancing: LoadBalancingPolicy,

    /// Request timeout (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,

    /// Retry policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<RetryPolicy>,

    /// CORS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors: Option<CorsPolicy>,

    /// Optional: restrict this route to specific source VPC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_vpc_attachment: Option<String>,
}

/// Route matching conditions
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct RouteMatch {
    /// HTTP path prefix to match (e.g., "/api/v1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,

    /// Exact HTTP path match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_path: Option<String>,

    /// HTTP headers to match
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,

    /// Query parameters to match
    #[serde(default)]
    pub query_params: std::collections::BTreeMap<String, String>,

    /// HTTP methods (GET, POST, etc)
    #[serde(default)]
    pub methods: Vec<String>,

    /// gRPC service name (for gRPC routes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_service: Option<String>,

    /// gRPC method name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_method: Option<String>,
}

/// Destination for a route
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct RouteDestination {
    /// Reference to a VPCService
    pub vpc_service_ref: ServiceRef,

    /// Weight for weighted load balancing (0-100)
    #[serde(default = "default_weight")]
    pub weight: u32,

    /// Port override (if different from VPCService port)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// Reference to a VPCService
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct ServiceRef {
    /// Name of the VPCService
    pub name: String,
    /// Namespace of the VPCService (defaults to same namespace as route)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Load balancing policy
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LoadBalancingPolicy {
    /// Round-robin distribution
    RoundRobin,
    /// Send to least-connections endpoint
    LeastConnections,
    /// Sticky session using source IP
    SourceIp,
    /// Consistent hashing (for stateful services)
    ConsistentHash,
}

impl Default for LoadBalancingPolicy {
    fn default() -> Self {
        LoadBalancingPolicy::RoundRobin
    }
}

/// Retry policy
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: u32,

    /// Retry on these HTTP status codes
    #[serde(default)]
    pub retry_on_status: Vec<u16>,

    /// Backoff configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backoff: Option<BackoffConfig>,
}

/// Exponential backoff configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct BackoffConfig {
    /// Initial backoff duration (ms)
    #[serde(default = "default_initial_backoff")]
    pub initial_ms: u32,

    /// Maximum backoff duration (ms)
    #[serde(default = "default_max_backoff")]
    pub max_ms: u32,
}

/// CORS policy
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct CorsPolicy {
    /// Allowed origins (use "*" for all)
    #[serde(default)]
    pub allowed_origins: Vec<String>,

    /// Allowed HTTP methods
    #[serde(default)]
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    #[serde(default)]
    pub allowed_headers: Vec<String>,

    /// Allow credentials
    #[serde(default)]
    pub allow_credentials: bool,

    /// Max age for preflight caching (seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_seconds: Option<u32>,
}

/// Status of a VPCRoute
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VPCRouteStatus {
    /// Whether this route is ready
    #[serde(default)]
    pub ready: bool,

    /// Number of active destination endpoints
    #[serde(default)]
    pub active_destinations: u32,
}

fn default_load_balancing() -> LoadBalancingPolicy {
    LoadBalancingPolicy::RoundRobin
}

fn default_weight() -> u32 {
    100
}

fn default_initial_backoff() -> u32 {
    100
}

fn default_max_backoff() -> u32 {
    10000
}
