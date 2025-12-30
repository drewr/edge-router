use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Endpoint not found: {0}")]
    EndpointNotFound(String),

    #[error("Invalid service configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Kubernetes error: {0}")]
    KubernetesError(#[from] kube::error::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
