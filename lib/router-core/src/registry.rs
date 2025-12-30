//! Service registry for managing VPCServices and endpoints

use crate::{Endpoint, Result, CoreError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// ServiceRegistry maintains a registry of services and their endpoints
pub struct ServiceRegistry {
    // Map of service_id (namespace/name) to endpoints
    services: Arc<RwLock<HashMap<String, ServiceInfo>>>,
}

/// Information about a registered service
#[derive(Clone, Debug)]
pub struct ServiceInfo {
    pub service_id: String,
    pub namespace: String,
    pub name: String,
    pub port: u16,
    pub protocol: String,
    pub endpoints: Vec<Endpoint>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register or update a service
    pub async fn register_service(
        &self,
        namespace: String,
        name: String,
        port: u16,
        protocol: String,
        endpoints: Vec<Endpoint>,
    ) -> Result<()> {
        let service_id = format!("{}/{}", namespace, name);

        let mut services = self.services.write().await;
        services.insert(
            service_id.clone(),
            ServiceInfo {
                service_id: service_id.clone(),
                namespace,
                name,
                port,
                protocol,
                endpoints,
            },
        );

        debug!("Registered service: {}", service_id);
        Ok(())
    }

    /// Get service information
    pub async fn get_service(&self, service_id: &str) -> Result<ServiceInfo> {
        let services = self.services.read().await;
        services.get(service_id).cloned().ok_or_else(|| {
            CoreError::ServiceNotFound(service_id.to_string())
        })
    }

    /// Get endpoints for a service
    pub async fn get_endpoints(&self, service_id: &str) -> Result<Vec<Endpoint>> {
        let service = self.get_service(service_id).await?;
        Ok(service.endpoints)
    }

    /// Update endpoints for a service
    pub async fn update_endpoints(
        &self,
        service_id: &str,
        endpoints: Vec<Endpoint>,
    ) -> Result<()> {
        let mut services = self.services.write().await;
        if let Some(service) = services.get_mut(service_id) {
            service.endpoints = endpoints;
            debug!("Updated endpoints for service: {}", service_id);
            Ok(())
        } else {
            Err(CoreError::ServiceNotFound(service_id.to_string()))
        }
    }

    /// List all services
    pub async fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        let services = self.services.read().await;
        Ok(services.values().cloned().collect())
    }

    /// Deregister a service
    pub async fn deregister_service(&self, service_id: &str) -> Result<()> {
        let mut services = self.services.write().await;
        services.remove(service_id);
        debug!("Deregistered service: {}", service_id);
        Ok(())
    }

    /// Get count of registered services
    pub async fn service_count(&self) -> usize {
        let services = self.services.read().await;
        services.len()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
