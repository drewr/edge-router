/// Bindings to Galactic VPC CRDs from galactic-operator
///
/// This module provides type-safe Rust bindings to the Galactic VPC
/// custom resources (VPC and VPCAttachment) to enable discovery and
/// integration with the Galactic VPC Layer 3 overlay network.

pub mod vpc;
pub mod vpc_attachment;

pub use vpc::VPC;
pub use vpc_attachment::VPCAttachment;
