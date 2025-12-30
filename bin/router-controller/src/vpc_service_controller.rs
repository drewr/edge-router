//! VPCService controller for reconciling VPCService resources

use kube::{Api, Client};
use kube_runtime::{Controller, controller::Action};
use futures::StreamExt;
use router_api::VPCService;
use router_core::ServiceRegistry;
use router_galactic::VPCDiscovery;
use tracing::{info, debug, error};
use std::sync::Arc;
use std::time::Duration;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ReconcileError(pub String);

impl fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Reconciliation error: {}", self.0)
    }
}

impl Error for ReconcileError {}

pub struct VPCServiceController {
    client: Client,
    registry: Arc<ServiceRegistry>,
}

impl VPCServiceController {
    pub async fn new(client: Client) -> anyhow::Result<Self> {
        let registry = Arc::new(ServiceRegistry::new());
        Ok(Self { client, registry })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Starting VPCService reconciliation");

        let vpc_services: Api<VPCService> = Api::all(self.client.clone());
        let _discovery = VPCDiscovery::new().await?;

        // Watch for VPCService changes
        let controller = Controller::new(vpc_services.clone(), Default::default());

        let mut stream = controller
            .run(
                |vpc_svc, _ctx| async move {
                    let name = &vpc_svc.metadata.name;
                    let namespace = &vpc_svc.metadata.namespace;
                    info!(
                        "Reconciling VPCService: {}/{}",
                        namespace.as_ref().unwrap_or(&"default".to_string()),
                        name.as_ref().unwrap_or(&"unknown".to_string())
                    );
                    Ok(Action::requeue(Duration::from_secs(300)))
                },
                |_vpc_svc, _e: &ReconcileError, _ctx| {
                    error!("Error reconciling VPCService");
                    Action::requeue(Duration::from_secs(60))
                },
                Arc::new(()),
            )
            .boxed();

        // Process the reconciliation stream
        while let Some(item) = stream.next().await {
            match item {
                Ok(_) => debug!("Reconciled VPCService successfully"),
                Err(e) => error!("Error in reconciliation stream: {}", e),
            }
        }

        Ok(())
    }
}
