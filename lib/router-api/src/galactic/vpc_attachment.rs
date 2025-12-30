use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// VPCAttachment from galactic-operator - Defines how pods attach to a Galactic VPC
/// and receive network interfaces within the VPC
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "galactic.datumapis.com",
    version = "v1alpha",
    kind = "VPCAttachment",
    plural = "vpcattachments",
    derive = "Default",
    status = "VPCAttachmentStatus",
)]
pub struct VPCAttachmentSpec {
    /// Reference to the VPC
    pub vpc: VPCRef,

    /// Interface configuration
    pub interface: InterfaceConfig,

    /// Additional routes for this attachment
    #[serde(default)]
    pub routes: Vec<AttachmentRoute>,
}

/// Reference to a VPC
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct VPCRef {
    /// API version (typically "galactic.datumapis.com/v1alpha")
    pub api_version: String,

    /// Kind (always "VPC")
    pub kind: String,

    /// Name of the VPC
    pub name: String,

    /// Namespace of the VPC
    pub namespace: String,
}

/// Network interface configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct InterfaceConfig {
    /// Name of the interface (e.g., "galactic0")
    pub name: String,

    /// IP addresses assigned to this attachment
    pub addresses: Vec<String>,
}

/// Route in a VPCAttachment
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct AttachmentRoute {
    /// Destination CIDR
    pub destination: String,

    /// Via (next hop IP)
    pub via: String,
}

/// Status of a VPCAttachment
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VPCAttachmentStatus {
    /// Whether the attachment is ready
    #[serde(default)]
    pub ready: bool,

    /// Unique hex identifier for this attachment (16 bits)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,

    /// Assigned interface name on the host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,

    /// Assigned interface name in the pod
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_interface_name: Option<String>,

    /// SRv6 endpoint address for this attachment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub srv6_endpoint: Option<String>,
}
