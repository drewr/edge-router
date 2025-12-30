use anyhow::Result;
use kube::Client;
use tracing::{info, error};
use tracing_subscriber::fmt::init as tracing_init;

mod vpc_service_controller;
mod vpc_route_controller;
mod vpc_ingress_controller;

use vpc_service_controller::VPCServiceController;
use vpc_route_controller::VPCRouteController;
use vpc_ingress_controller::VPCIngressController;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init();

    info!("Starting router-controller...");

    let client = Client::try_default().await?;

    // Start VPCService reconciliation controller
    let vpc_service_controller = VPCServiceController::new(client.clone()).await?;
    tokio::spawn(async move {
        if let Err(e) = vpc_service_controller.run().await {
            error!("VPCService controller error: {}", e);
        }
    });

    // Start VPCRoute reconciliation controller
    let vpc_route_controller = VPCRouteController::new(client.clone()).await?;
    tokio::spawn(async move {
        if let Err(e) = vpc_route_controller.run().await {
            error!("VPCRoute controller error: {}", e);
        }
    });

    // Start VPCIngress reconciliation controller
    let vpc_ingress_controller = VPCIngressController::new(client.clone()).await?;
    tokio::spawn(async move {
        if let Err(e) = vpc_ingress_controller.run().await {
            error!("VPCIngress controller error: {}", e);
        }
    });

    // Keep the process alive
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, exiting...");

    Ok(())
}
