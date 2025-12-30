//! Datum Router API types and CRDs for Kubernetes integration
//!
//! This library defines the custom resources for the datum-cloud router:
//! - VPCService: Services running in Galactic VPCs
//! - VPCRoute: Layer 7 routing rules for traffic between VPCs
//! - VPCIngress: External ingress into VPC networks
//! - ServiceBinding: Binds Kubernetes Services to VPCServices
//! - VPCEgress: Controls outbound traffic from VPCs

pub mod v1alpha1;
pub mod galactic;

pub use v1alpha1::{VPCService, VPCRoute, VPCIngress, ServiceBinding, VPCEgress};
