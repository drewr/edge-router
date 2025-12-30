use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPCIngress defines external ingress into VPC networks via the router-gateway
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "router.datum.net",
    version = "v1alpha1",
    kind = "VPCIngress",
    plural = "vpcingresses",
    derive = "Default",
    status = "VPCIngressStatus",
)]
pub struct VPCIngressSpec {
    /// Hostname for this ingress (e.g., api.example.com)
    pub host: String,

    /// Ingress routes
    pub rules: Vec<IngressRule>,

    /// TLS configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,

    /// Target VPC (optional, can be per-rule)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_attachment_name: Option<String>,

    /// Annotations for the ingress
    #[serde(default)]
    pub annotations: std::collections::BTreeMap<String, String>,
}

/// Ingress rule for path-based routing
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct IngressRule {
    /// HTTP path prefix (e.g., "/api/v1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Target VPCService
    pub service: ServiceBackend,

    /// Optional VPC attachment override for this rule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_attachment_name: Option<String>,
}

/// Service backend for a route
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct ServiceBackend {
    /// Name of the VPCService
    pub name: String,

    /// Namespace of the VPCService
    pub namespace: String,

    /// Port on the VPCService
    pub port: u16,
}

/// TLS configuration for HTTPS
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct TlsConfig {
    /// TLS certificate secret name (in same namespace as ingress)
    pub secret_name: String,

    /// Certificate version (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_version: Option<String>,

    /// TLS mode: "passthrough" or "terminate"
    #[serde(default = "default_tls_mode")]
    pub mode: String,

    /// Minimum TLS version ("1.2", "1.3")
    #[serde(default = "default_min_tls_version")]
    pub min_version: String,

    /// Cipher suites (optional)
    #[serde(default)]
    pub cipher_suites: Vec<String>,
}

/// Status of a VPCIngress
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VPCIngressStatus {
    /// Whether this ingress is ready
    #[serde(default)]
    pub ready: bool,

    /// IP address of the router-gateway
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_balancer_ip: Option<String>,

    /// Number of active backends
    #[serde(default)]
    pub active_backends: u32,

    /// Current ingress addresses
    #[serde(default)]
    pub ingress_addresses: Vec<IngressAddress>,
}

/// Ingress address information
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct IngressAddress {
    /// IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,

    /// Hostname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

fn default_tls_mode() -> String {
    "terminate".to_string()
}

fn default_min_tls_version() -> String {
    "1.2".to_string()
}
