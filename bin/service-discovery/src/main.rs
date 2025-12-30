use anyhow::Result;
use router_core::ServiceRegistry;
use router_galactic::VPCDiscovery;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error, debug};
use tracing_subscriber::fmt::init as tracing_init;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init();

    info!("Starting service-discovery daemon...");

    let registry = Arc::new(ServiceRegistry::new());
    let discovery = VPCDiscovery::new().await?;

    // Periodic discovery loop
    loop {
        match discover_services(&discovery, &registry).await {
            Ok(count) => {
                info!("Discovered and registered {} services", count);
            }
            Err(e) => {
                error!("Error discovering services: {}", e);
            }
        }

        // Wait before next discovery cycle
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn discover_services(
    discovery: &VPCDiscovery,
    registry: &Arc<ServiceRegistry>,
) -> Result<usize> {
    let mut count = 0;

    // Discover all VPCs
    let vpcs = discovery.discover_vpcs().await?;
    debug!("Found {} VPCs", vpcs.len());

    // Discover all VPCAttachments
    let attachments = discovery.discover_attachments().await?;
    debug!("Found {} VPCAttachments", attachments.len());

    // Build attachment map for quick lookup (for future use)
    let _attachment_map = discovery.vpc_attachment_map().await?;

    // For each attachment, if there's a service running on it, register the service
    for attachment in attachments {
        let _vpc_key = format!("{}/{}", attachment.spec.vpc.namespace, attachment.spec.vpc.name);

        let ipv4_addrs = VPCDiscovery::attachment_ipv4_addresses(&attachment);
        let name = attachment.metadata.name.as_ref().map(|n| n.as_str()).unwrap_or("unknown");
        let namespace = attachment.metadata.namespace.as_ref().map(|n| n.as_str()).unwrap_or("default");
        debug!(
            "Attachment {}/{} has {} IPv4 addresses",
            namespace,
            name,
            ipv4_addrs.len()
        );

        // In a real implementation, we'd discover services running on these IPs
        // For now, this is a placeholder
        count += ipv4_addrs.len();
    }

    // Log current registry state
    let service_count = registry.service_count().await;
    debug!("Service registry has {} services", service_count);

    Ok(count)
}
