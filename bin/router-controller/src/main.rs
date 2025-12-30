use anyhow::Result;
use kube::Client;
use tracing::{info, error};
use tracing_subscriber::fmt::init as tracing_init;

mod vpc_service_controller;
mod vpc_route_controller;

use vpc_service_controller::VPCServiceController;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init();

    info!("Starting router-controller...");

    let client = Client::try_default().await?;
    let controller = VPCServiceController::new(client.clone()).await?;

    // Start VPCService reconciliation
    tokio::spawn(async move {
        if let Err(e) = controller.run().await {
            error!("VPCService controller error: {}", e);
        }
    });

    // Keep the process alive
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, exiting...");

    Ok(())
}
