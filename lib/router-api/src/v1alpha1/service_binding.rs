use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


/// ServiceBinding binds a Kubernetes Service to a VPCService
/// for automatic endpoint synchronization across VPCs
#[derive(CustomResource, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "router.datum.net",
    version = "v1alpha1",
    kind = "ServiceBinding",
    plural = "servicebindings",
    derive = "Default",
    status = "ServiceBindingStatus",
)]
pub struct ServiceBindingSpec {
    /// Reference to a Kubernetes Service
    pub service_ref: KubernetesServiceRef,

    /// Reference to the target VPCService
    pub vpc_service_ref: VPCServiceRef,

    /// Port mapping from Kubernetes Service to VPCService
    #[serde(default)]
    pub port_mappings: Vec<PortMapping>,

    /// Pod selector for endpoint discovery
    /// If not specified, uses Service's selector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_selector: Option<PodSelector>,

    /// Whether to automatically sync endpoints
    #[serde(default = "bool::default")]
    pub auto_sync: bool,

    /// Update interval (seconds) for syncing endpoints
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u32,
}

/// Reference to a Kubernetes Service
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct KubernetesServiceRef {
    /// Name of the Kubernetes Service
    pub name: String,

    /// Namespace of the Kubernetes Service
    pub namespace: String,
}

/// Reference to a VPCService
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct VPCServiceRef {
    /// Name of the VPCService
    pub name: String,

    /// Namespace of the VPCService
    pub namespace: String,
}

/// Port mapping configuration
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct PortMapping {
    /// Port on the Kubernetes Service
    pub service_port: u16,

    /// Port on the VPCService (defaults to service_port)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_port: Option<u16>,

    /// Protocol (TCP, UDP)
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

/// Pod selector for endpoint discovery
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct PodSelector {
    /// Label selectors for matching pods
    pub match_labels: std::collections::BTreeMap<String, String>,

    /// Label expressions for pod selection
    #[serde(default)]
    pub match_expressions: Vec<LabelExpression>,
}

/// Label expression for pod matching
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct LabelExpression {
    /// Label key
    pub key: String,

    /// Operator (In, NotIn, Exists, DoesNotExist)
    pub operator: String,

    /// Values to match (for In/NotIn operators)
    #[serde(default)]
    pub values: Vec<String>,
}

/// Status of a ServiceBinding
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceBindingStatus {
    /// Whether this binding is active
    #[serde(default)]
    pub active: bool,

    /// Number of synchronized endpoints
    #[serde(default)]
    pub synced_endpoints: u32,

    /// Last sync time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_time: Option<String>,

    /// Current conditions
    #[serde(default)]
    pub conditions: Vec<BindingCondition>,

    /// Synchronized endpoint addresses
    #[serde(default)]
    pub endpoints: Vec<String>,
}

/// Condition for ServiceBinding status
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[derive(Default)]
pub struct BindingCondition {
    /// Type of condition
    pub condition_type: String,

    /// Status: "True", "False"
    pub status: String,

    /// Reason for the condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn default_protocol() -> String {
    "TCP".to_string()
}

fn default_sync_interval() -> u32 {
    30
}
