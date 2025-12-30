use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPC from galactic-operator - Represents a virtual private cloud network
/// that spans multiple Kubernetes clusters
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "galactic.datumapis.com",
    version = "v1alpha",
    kind = "VPC",
    plural = "vpcs",
    derive = "Default",
    status = "VPCStatus",
)]
pub struct VPCSpec {
    /// Networks (CIDRs) associated with this VPC
    /// Can include both IPv4 and IPv6 networks
    pub networks: Vec<String>,
}

/// Status of a Galactic VPC
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VPCStatus {
    /// Whether the VPC is ready
    #[serde(default)]
    pub ready: bool,

    /// Unique hex identifier for this VPC (48 bits)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,

    /// Number of active attachments
    #[serde(default)]
    pub attachment_count: u32,
}
