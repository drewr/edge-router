//! VPCRoute controller for reconciling VPCRoute resources

use kube::{Api, Client};
use router_api::VPCRoute;
use tracing::info;

pub struct VPCRouteController {
    client: Client,
}

impl VPCRouteController {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Starting VPCRoute reconciliation");

        let _vpc_routes: Api<VPCRoute> = Api::all(self.client.clone());

        // TODO: Implement VPCRoute reconciliation
        // Watch for VPCRoute changes and update routing tables

        Ok(())
    }
}
