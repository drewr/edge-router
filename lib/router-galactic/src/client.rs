//! Kubernetes client for Galactic VPC resources

use kube::Client;

/// GalacticClient wraps the Kubernetes client for VPC operations
pub struct GalacticClient {
    client: Client,
}

impl GalacticClient {
    /// Create a new Galactic client
    pub async fn new() -> anyhow::Result<Self> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    /// Get the underlying Kubernetes client
    pub fn inner(&self) -> &Client {
        &self.client
    }

    /// Get a clone of the Kubernetes client
    pub fn clone_client(&self) -> Client {
        self.client.clone()
    }
}
