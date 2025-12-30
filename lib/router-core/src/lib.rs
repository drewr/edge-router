//! Core routing and service registry functionality
//!
//! This library provides:
//! - Service registry for managing VPCServices and their endpoints
//! - Endpoint discovery and synchronization
//! - Traffic policy engine

pub mod registry;
pub mod endpoint;
pub mod error;

pub use registry::ServiceRegistry;
pub use endpoint::Endpoint;
pub use error::{CoreError, Result};
