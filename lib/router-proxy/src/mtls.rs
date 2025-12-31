//! mTLS (Mutual TLS) support for client certificate validation and service authentication

use rustls::pki_types::CertificateDer;
use rustls_pemfile::certs;
use std::io::BufReader;
use anyhow::{Result, anyhow};
use tracing::{debug, info};

/// Client authentication mode for incoming TLS connections
#[derive(Clone, Debug, PartialEq)]
pub enum ClientAuthMode {
    /// No client certificate required
    NoClientAuth,
    /// Client certificate is optional
    Optional,
    /// Client certificate is required
    Required,
}

impl ClientAuthMode {
    /// Parse client auth mode from string
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "required" => ClientAuthMode::Required,
            "optional" => ClientAuthMode::Optional,
            _ => ClientAuthMode::NoClientAuth,
        }
    }

    /// Check if client auth is required
    pub fn is_required(&self) -> bool {
        matches!(self, ClientAuthMode::Required)
    }

    /// Check if client auth is optional or required
    pub fn is_enabled(&self) -> bool {
        !matches!(self, ClientAuthMode::NoClientAuth)
    }
}

/// TLS configuration for client authentication (outbound mTLS for service-to-service)
#[derive(Clone, Debug, PartialEq)]
pub struct TlsClientConfig {
    /// PEM-encoded client certificate
    pub cert_pem: Vec<u8>,
    /// PEM-encoded client private key
    pub key_pem: Vec<u8>,
    /// Optional PEM-encoded CA certificate for server verification
    pub ca_cert_pem: Option<Vec<u8>>,
    /// Whether to verify the server certificate
    pub verify_server_cert: bool,
}

impl TlsClientConfig {
    /// Create a new TLS client configuration
    pub fn new(
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        ca_cert_pem: Option<Vec<u8>>,
        verify_server_cert: bool,
    ) -> Self {
        Self {
            cert_pem,
            key_pem,
            ca_cert_pem,
            verify_server_cert,
        }
    }

    /// Load TLS client configuration from PEM data
    pub fn from_pem(
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        ca_cert_pem: Option<Vec<u8>>,
        verify_server_cert: bool,
    ) -> Result<Self> {
        // Validate certificate format
        load_certificates(&cert_pem)?;
        debug!("Client certificate loaded successfully");

        Ok(Self {
            cert_pem,
            key_pem,
            ca_cert_pem,
            verify_server_cert,
        })
    }
}

/// Load certificates from PEM-encoded data
pub fn load_certificates(pem_data: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(pem_data);
    certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow!("Failed to parse certificates: {}", e))
}

/// Verifier for client certificates in mTLS
/// Holds CA certificates used for validating incoming client certificates
#[derive(Debug)]
pub struct MtlsClientVerifier {
    /// CA certificates for validating client certificates
    ca_certs: Vec<CertificateDer<'static>>,
}

impl MtlsClientVerifier {
    /// Create a new mTLS client certificate verifier
    pub fn new(ca_certs: Vec<CertificateDer<'static>>) -> Self {
        Self { ca_certs }
    }

    /// Load verifier from PEM-encoded CA certificates
    pub fn from_pem(pem_data: &[u8]) -> Result<Self> {
        let ca_certs = load_certificates(pem_data)?;
        if ca_certs.is_empty() {
            return Err(anyhow!("No CA certificates found in PEM data"));
        }
        info!("mTLS client verifier loaded {} CA certificate(s)", ca_certs.len());
        Ok(Self { ca_certs })
    }

    /// Get the number of CA certificates loaded
    pub fn ca_cert_count(&self) -> usize {
        self.ca_certs.len()
    }

    /// Get the CA certificates
    pub fn ca_certificates(&self) -> &[CertificateDer<'static>] {
        &self.ca_certs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_auth_mode_from_string() {
        assert_eq!(ClientAuthMode::from_string("required"), ClientAuthMode::Required);
        assert_eq!(ClientAuthMode::from_string("optional"), ClientAuthMode::Optional);
        assert_eq!(ClientAuthMode::from_string("none"), ClientAuthMode::NoClientAuth);
        assert_eq!(ClientAuthMode::from_string("REQUIRED"), ClientAuthMode::Required);
        assert_eq!(ClientAuthMode::from_string("unknown"), ClientAuthMode::NoClientAuth);
    }

    #[test]
    fn test_client_auth_mode_is_required() {
        assert!(ClientAuthMode::Required.is_required());
        assert!(!ClientAuthMode::Optional.is_required());
        assert!(!ClientAuthMode::NoClientAuth.is_required());
    }

    #[test]
    fn test_client_auth_mode_is_enabled() {
        assert!(ClientAuthMode::Required.is_enabled());
        assert!(ClientAuthMode::Optional.is_enabled());
        assert!(!ClientAuthMode::NoClientAuth.is_enabled());
    }

    #[test]
    fn test_tls_client_config_creation() {
        let cert = vec![1, 2, 3];
        let key = vec![4, 5, 6];
        let config = TlsClientConfig::new(cert.clone(), key.clone(), None, true);

        assert_eq!(config.cert_pem, cert);
        assert_eq!(config.key_pem, key);
        assert_eq!(config.ca_cert_pem, None);
        assert!(config.verify_server_cert);
    }

    #[test]
    fn test_tls_client_config_with_ca() {
        let cert = vec![1, 2, 3];
        let key = vec![4, 5, 6];
        let ca = vec![7, 8, 9];
        let config = TlsClientConfig::new(cert.clone(), key.clone(), Some(ca.clone()), true);

        assert_eq!(config.ca_cert_pem, Some(ca));
    }

    #[test]
    fn test_mtls_client_verifier_creation() {
        let certs = vec![];
        let verifier = MtlsClientVerifier::new(certs);
        assert_eq!(verifier.ca_cert_count(), 0);
    }

    #[test]
    fn test_mtls_client_verifier_count() {
        let certs = vec![CertificateDer::from(vec![1, 2, 3])];
        let verifier = MtlsClientVerifier::new(certs);
        assert_eq!(verifier.ca_cert_count(), 1);
    }

    #[test]
    fn test_mtls_client_verifier_get_certs() {
        let cert_data = vec![CertificateDer::from(vec![1, 2, 3])];
        let verifier = MtlsClientVerifier::new(cert_data.clone());
        let certs = verifier.ca_certificates();
        assert_eq!(certs.len(), 1);
    }
}
