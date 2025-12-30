/// API version v1alpha1 for Datum Router CRDs

pub mod vpc_service;
pub mod vpc_route;
pub mod vpc_ingress;
pub mod service_binding;
pub mod vpc_egress;

pub use vpc_service::VPCService;
pub use vpc_route::VPCRoute;
pub use vpc_ingress::VPCIngress;
pub use service_binding::ServiceBinding;
pub use vpc_egress::VPCEgress;

/// API group for Datum Router resources
pub const API_GROUP: &str = "router.datum.net";
/// API version for Datum Router resources
pub const API_VERSION: &str = "v1alpha1";
