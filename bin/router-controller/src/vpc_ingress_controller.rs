//! VPCIngress controller for reconciling VPCIngress resources

use kube::{Api, Client};
use kube_runtime::{Controller, controller::Action};
use futures::StreamExt;
use router_api::VPCIngress;
use router_core::ServiceRegistry;
use std::sync::Arc;
use std::time::Duration;
use std::error::Error;
use std::fmt;
use tracing::{info, debug, error};

#[derive(Debug)]
pub struct ReconcileError(pub String);

impl fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Reconciliation error: {}", self.0)
    }
}

impl Error for ReconcileError {}

pub struct VPCIngressController {
    client: Client,
    registry: Arc<ServiceRegistry>,
}

impl VPCIngressController {
    pub async fn new(client: Client) -> anyhow::Result<Self> {
        let registry = Arc::new(ServiceRegistry::new());
        Ok(Self { client, registry })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Starting VPCIngress reconciliation");

        let vpc_ingresses: Api<VPCIngress> = Api::all(self.client.clone());

        // Watch for VPCIngress changes
        let controller = Controller::new(vpc_ingresses.clone(), Default::default());

        let mut stream = controller
            .run(
                |vpc_ingress, _ctx| async move {
                    let name = &vpc_ingress.metadata.name;
                    let namespace = &vpc_ingress.metadata.namespace;
                    info!(
                        "Reconciling VPCIngress: {}/{}",
                        namespace.as_ref().unwrap_or(&"default".to_string()),
                        name.as_ref().unwrap_or(&"unknown".to_string())
                    );

                    // Log ingress configuration
                    debug!("VPCIngress spec: {:?}", vpc_ingress.spec);

                    // Future: Create/update router configuration for this ingress
                    // - Set up hostname/path -> VPCService routing
                    // - Configure TLS if specified
                    // - Update load balancer configuration

                    Ok(Action::requeue(Duration::from_secs(300)))
                },
                |_vpc_ingress, _e: &ReconcileError, _ctx| {
                    error!("Error reconciling VPCIngress");
                    Action::requeue(Duration::from_secs(60))
                },
                Arc::new(()),
            )
            .boxed();

        // Process the reconciliation stream
        while let Some(item) = stream.next().await {
            match item {
                Ok(_) => debug!("Reconciled VPCIngress successfully"),
                Err(e) => error!("Error in reconciliation stream: {}", e),
            }
        }

        Ok(())
    }
}
