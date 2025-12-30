//! Service discovery across Galactic VPCs

use kube::{Api, Client};
use router_api::galactic::{VPC, VPCAttachment};
use tracing::debug;
use std::collections::HashMap;

/// VPCDiscovery handles discovery of services across Galactic VPCs
pub struct VPCDiscovery {
    client: Client,
}

impl VPCDiscovery {
    /// Create a new VPC discovery client
    pub async fn new() -> anyhow::Result<Self> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    /// Discover all VPCs in the cluster
    pub async fn discover_vpcs(&self) -> anyhow::Result<Vec<VPC>> {
        let vpcs: Api<VPC> = Api::all(self.client.clone());
        let list = vpcs.list(&Default::default()).await?;

        debug!("Discovered {} VPCs", list.items.len());
        Ok(list.items)
    }

    /// Discover all VPC attachments in the cluster
    pub async fn discover_attachments(&self) -> anyhow::Result<Vec<VPCAttachment>> {
        let attachments: Api<VPCAttachment> = Api::all(self.client.clone());
        let list = attachments.list(&Default::default()).await?;

        debug!("Discovered {} VPC attachments", list.items.len());
        Ok(list.items)
    }

    /// Discover attachments for a specific VPC
    pub async fn discover_vpc_attachments(
        &self,
        vpc_name: &str,
        vpc_namespace: &str,
    ) -> anyhow::Result<Vec<VPCAttachment>> {
        let attachments: Api<VPCAttachment> = Api::all(self.client.clone());
        let list = attachments.list(&Default::default()).await?;

        let matching = list
            .items
            .into_iter()
            .filter(|att| {
                att.spec.vpc.name == vpc_name && att.spec.vpc.namespace == vpc_namespace
            })
            .collect();

        Ok(matching)
    }

    /// Build a map of VPC to attachments
    pub async fn vpc_attachment_map(&self) -> anyhow::Result<HashMap<String, Vec<VPCAttachment>>> {
        let attachments = self.discover_attachments().await?;
        let mut map: HashMap<String, Vec<VPCAttachment>> = HashMap::new();

        for attachment in attachments {
            let key = format!("{}/{}", attachment.spec.vpc.namespace, attachment.spec.vpc.name);
            map.entry(key).or_insert_with(Vec::new).push(attachment);
        }

        Ok(map)
    }

    /// Get the IPv4 addresses for a VPCAttachment
    pub fn attachment_ipv4_addresses(attachment: &VPCAttachment) -> Vec<String> {
        attachment
            .spec
            .interface
            .addresses
            .iter()
            .filter(|addr| !addr.contains(':'))  // Simple IPv4 check
            .cloned()
            .collect()
    }

    /// Get the IPv6 addresses for a VPCAttachment
    pub fn attachment_ipv6_addresses(attachment: &VPCAttachment) -> Vec<String> {
        attachment
            .spec
            .interface
            .addresses
            .iter()
            .filter(|addr| addr.contains(':'))  // Simple IPv6 check
            .cloned()
            .collect()
    }
}
