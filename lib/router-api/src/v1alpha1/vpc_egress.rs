use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPCEgress controls outbound traffic from VPCs to external services
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "router.datum.net",
    version = "v1alpha1",
    kind = "VPCEgress",
    plural = "vpcegresses",
    derive = "Default",
    status = "VPCEgressStatus",
)]
pub struct VPCEgressSpec {
    /// VPC attachment where traffic originates
    pub source_vpc_attachment: String,

    /// Match conditions for egress traffic
    pub r#match: EgressMatch,

    /// Destination rules
    pub destinations: Vec<EgressDestination>,

    /// Policy: Allow or Deny
    #[serde(default = "default_policy")]
    pub policy: String,

    /// Rate limiting (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
}

/// Match conditions for egress traffic
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct EgressMatch {
    /// Destination IP ranges (CIDR)
    #[serde(default)]
    pub destination_cidrs: Vec<String>,

    /// Destination ports
    #[serde(default)]
    pub destination_ports: Vec<u16>,

    /// Protocols (TCP, UDP, etc)
    #[serde(default)]
    pub protocols: Vec<String>,

    /// Source labels for pod selection
    #[serde(default)]
    pub source_labels: std::collections::BTreeMap<String, String>,
}

/// Egress destination rule
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct EgressDestination {
    /// External endpoint (IP or hostname)
    pub endpoint: String,

    /// Port (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// TLS for outbound connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<OutboundTls>,
}

/// TLS configuration for outbound connections
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct OutboundTls {
    /// Whether to use TLS
    #[serde(default)]
    pub enabled: bool,

    /// Server name for SNI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,

    /// Skip certificate verification (not recommended)
    #[serde(default)]
    pub insecure: bool,
}

/// Rate limiting configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct RateLimitConfig {
    /// Requests per second
    pub requests_per_second: u32,

    /// Burst size
    #[serde(default)]
    pub burst_size: u32,
}

/// Status of a VPCEgress
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VPCEgressStatus {
    /// Whether this egress rule is active
    #[serde(default)]
    pub active: bool,

    /// Number of connections
    #[serde(default)]
    pub connection_count: u32,
}

fn default_policy() -> String {
    "Allow".to_string()
}
