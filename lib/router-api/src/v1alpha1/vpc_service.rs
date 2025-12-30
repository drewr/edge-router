use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPCService represents a service running inside a Galactic VPC
/// that should be discoverable and routable across VPCs
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "router.datum.net",
    version = "v1alpha1",
    kind = "VPCService",
    plural = "vpcservices",
    derive = "Default",
    status = "VPCServiceStatus",
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Endpoints","type":"integer","jsonPath":".status.endpointCount"}"#,
)]
pub struct VPCServiceSpec {
    /// Reference to the VPCAttachment where this service runs
    pub vpc_attachment_ref: VPCAttachmentRef,

    /// Protocol: HTTP, HTTPS, gRPC, or TCP
    #[serde(default = "default_protocol")]
    pub protocol: String,

    /// Port where the service listens
    pub port: u16,

    /// Optional: Target port if different from port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u16>,

    /// Health check configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// Service discovery settings (DNS name, discovery method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery: Option<DiscoveryConfig>,

    /// Labels for selecting this service
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

/// Status of a VPCService
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VPCServiceStatus {
    /// Whether this service is ready for traffic
    #[serde(default)]
    pub ready: bool,

    /// Number of active endpoints
    #[serde(default)]
    pub endpoint_count: u32,

    /// Last update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_time: Option<String>,

    /// Current endpoints
    #[serde(default)]
    pub endpoints: Vec<EndpointStatus>,

    /// Conditions describing the status
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

impl Default for VPCServiceStatus {
    fn default() -> Self {
        Self {
            ready: false,
            endpoint_count: 0,
            last_update_time: None,
            endpoints: Vec::new(),
            conditions: Vec::new(),
        }
    }
}

/// Reference to a VPCAttachment
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct VPCAttachmentRef {
    /// Name of the VPCAttachment
    pub name: String,
    /// Namespace of the VPCAttachment (defaults to "default")
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

/// Health check configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct HealthCheckConfig {
    /// Path for HTTP health checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_path: Option<String>,

    /// Interval between health checks (seconds)
    #[serde(default = "default_health_check_interval")]
    pub interval_seconds: u32,

    /// Timeout for health check (seconds)
    #[serde(default = "default_health_check_timeout")]
    pub timeout_seconds: u32,

    /// Consecutive failures before marking unhealthy
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,

    /// Consecutive successes before marking healthy
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,
}

/// Service discovery configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct DiscoveryConfig {
    /// DNS name for this service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_name: Option<String>,

    /// Discovery method: "manual", "kubernetes", "consul", etc.
    #[serde(default = "default_discovery_method")]
    pub method: String,
}

/// Status of an endpoint
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct EndpointStatus {
    /// IP address of the endpoint (VPC IP)
    pub ip: String,

    /// Port of the endpoint
    pub port: u16,

    /// Whether this endpoint is ready
    #[serde(default = "bool::default")]
    pub ready: bool,

    /// Last heartbeat/update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_heartbeat: Option<String>,
}

/// Condition for VPCService status
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct Condition {
    /// Type of condition
    pub condition_type: String,

    /// Status: "True", "False", "Unknown"
    pub status: String,

    /// Reason for the condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Last update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_time: Option<String>,
}

// Default values
fn default_protocol() -> String {
    "HTTP".to_string()
}

fn default_namespace() -> String {
    "default".to_string()
}

fn default_health_check_interval() -> u32 {
    10
}

fn default_health_check_timeout() -> u32 {
    5
}

fn default_unhealthy_threshold() -> u32 {
    3
}

fn default_healthy_threshold() -> u32 {
    2
}

fn default_discovery_method() -> String {
    "manual".to_string()
}
