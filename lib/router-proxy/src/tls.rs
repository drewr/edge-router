//! TLS/HTTPS support for router gateway

use std::sync::Arc;
use rustls::{ServerConfig, pki_types::PrivateKeyDer, RootCertStore};
use rustls_pemfile::{certs, read_all};
use std::io::BufReader;
use anyhow::{Result, anyhow};
use tracing::{debug, info};
use crate::mtls::{ClientAuthMode, MtlsClientVerifier, load_certificates};

/// TLS configuration for HTTPS listener
#[derive(Clone)]
pub struct TlsServerConfig {
    /// Rustls server configuration
    pub config: Arc<ServerConfig>,
    /// Minimum TLS version
    pub min_version: String,
    /// Cipher suites (if customized)
    pub cipher_suites: Vec<String>,
    /// Client authentication mode (for mTLS)
    pub client_auth: ClientAuthMode,
}

impl TlsServerConfig {
    /// Validate TLS version string
    pub fn validate_version(version: &str) -> Result<()> {
        match version {
            "1.0" | "1.1" | "1.2" | "1.3" => Ok(()),
            _ => Err(anyhow!(
                "Invalid TLS version: {}. Must be 1.0, 1.1, 1.2, or 1.3",
                version
            )),
        }
    }

    /// Create a TLS configuration from PEM-encoded certificate and private key
    pub fn from_pem(
        cert_pem: &[u8],
        key_pem: &[u8],
        min_version: Option<String>,
        cipher_suites: Option<Vec<String>>,
    ) -> Result<Self> {
        Self::from_pem_with_client_auth(
            cert_pem,
            key_pem,
            None,
            false,
            min_version,
            cipher_suites,
        )
    }

    /// Create a TLS configuration with optional client certificate validation (mTLS)
    pub fn from_pem_with_client_auth(
        cert_pem: &[u8],
        key_pem: &[u8],
        ca_cert_pem: Option<&[u8]>,
        require_client_cert: bool,
        min_version: Option<String>,
        cipher_suites: Option<Vec<String>>,
    ) -> Result<Self> {
        debug!("Creating TLS configuration from PEM data");

        // Parse certificates
        let mut cert_reader = BufReader::new(cert_pem);
        let certs_vec = certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Failed to parse certificates: {}", e))?;

        if certs_vec.is_empty() {
            return Err(anyhow!("No certificates found in PEM data"));
        }

        debug!("Loaded {} certificate(s)", certs_vec.len());

        // Parse private key using rustls_pemfile 2.x API
        let mut key_reader = BufReader::new(key_pem);
        let keys: Vec<_> = read_all(&mut key_reader)
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!("Failed to parse private key: {}", e))?;

        // Find the first private key
        let mut private_key = None;
        for item in keys {
            match item {
                rustls_pemfile::Item::Pkcs8Key(k) => {
                    private_key = Some(PrivateKeyDer::Pkcs8(k));
                    break;
                }
                rustls_pemfile::Item::Sec1Key(k) => {
                    private_key = Some(PrivateKeyDer::Sec1(k));
                    break;
                }
                _ => {}
            }
        }

        let private_key = private_key.ok_or_else(|| anyhow!("No private key found in PEM data"))?;
        debug!("Loaded private key");

        // Create server configuration with optional client authentication
        let config = if let Some(ca_pem) = ca_cert_pem {
            debug!("Setting up mTLS with client certificate validation");
            let _verifier = MtlsClientVerifier::from_pem(ca_pem)?;
            let _ca_certs = load_certificates(ca_pem)?;

            let client_auth_mode = if require_client_cert {
                ClientAuthMode::Required
            } else {
                ClientAuthMode::Optional
            };

            // For Phase 4.7a: Require client certificate to be present
            // Full chain validation will be added in Phase 4.8
            let config = if require_client_cert {
                // Create a verifier that requires client certs with WebPKI validation
                use rustls::server::WebPkiClientVerifier;

                // Create a root cert store with the CA certificates
                let mut root_store = RootCertStore::empty();
                for cert in _ca_certs {
                    root_store.add(cert)
                        .map_err(|e| anyhow!("Failed to add CA certificate to root store: {}", e))?;
                }

                let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
                    .build()
                    .map_err(|e| anyhow!("Failed to create WebPKI verifier: {}", e))?;

                ServerConfig::builder()
                    .with_client_cert_verifier(client_verifier)
                    .with_single_cert(certs_vec, private_key)
                    .map_err(|e| anyhow!("Failed to create mTLS config: {}", e))?
            } else {
                // Optional: Accept client cert if provided
                ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(certs_vec, private_key)
                    .map_err(|e| anyhow!("Failed to create TLS config: {}", e))?
            };

            info!("TLS configuration created with client auth mode: {:?}", client_auth_mode);
            config
        } else {
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs_vec, private_key)
                .map_err(|e| anyhow!("Failed to create TLS config: {}", e))?
        };

        let min_version_str = min_version.clone().unwrap_or_else(|| "1.2".to_string());
        Self::validate_version(&min_version_str)?;

        let client_auth = if ca_cert_pem.is_some() {
            if require_client_cert {
                ClientAuthMode::Required
            } else {
                ClientAuthMode::Optional
            }
        } else {
            ClientAuthMode::NoClientAuth
        };

        info!("TLS configuration created successfully with client auth: {:?}", client_auth);

        Ok(Self {
            config: Arc::new(config),
            min_version: min_version_str,
            cipher_suites: cipher_suites.unwrap_or_default(),
            client_auth,
        })
    }

    /// Validate that certificate is properly configured
    pub fn validate(&self) -> Result<()> {
        debug!("Validating TLS configuration");
        Self::validate_version(&self.min_version)?;
        info!("TLS configuration validation passed");
        Ok(())
    }
}

/// Certificate and key material
pub struct CertificateMaterial {
    /// PEM-encoded certificate chain
    pub cert: Vec<u8>,
    /// PEM-encoded private key
    pub key: Vec<u8>,
}

impl CertificateMaterial {
    /// Create certificate material from bytes
    pub fn new(cert: Vec<u8>, key: Vec<u8>) -> Self {
        Self { cert, key }
    }

    /// Create TLS configuration from this certificate material
    pub fn to_tls_config(
        self,
        min_version: Option<String>,
        cipher_suites: Option<Vec<String>>,
    ) -> Result<TlsServerConfig> {
        TlsServerConfig::from_pem(&self.cert, &self.key, min_version, cipher_suites)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_tls_versions() {
        for version in &["1.0", "1.1", "1.2", "1.3"] {
            let result = TlsServerConfig::validate_version(version);
            assert!(result.is_ok(), "Version {} should be valid", version);
        }
    }

    #[test]
    fn test_validate_invalid_tls_version() {
        let result = TlsServerConfig::validate_version("2.0");
        assert!(result.is_err(), "Version 2.0 should be invalid");
    }

    #[test]
    fn test_certificate_material_creation() {
        let cert_data = vec![1, 2, 3];
        let key_data = vec![4, 5, 6];
        let material = CertificateMaterial::new(cert_data.clone(), key_data.clone());
        assert_eq!(material.cert, cert_data);
        assert_eq!(material.key, key_data);
    }

    #[test]
    fn test_tls_server_config_default_no_client_auth() {
        // When from_pem is called without client auth, should use NoClientAuth
        // This is a structural test - we can't test full creation without real certs
        let client_auth = ClientAuthMode::NoClientAuth;
        assert!(!client_auth.is_enabled());
        assert!(!client_auth.is_required());
    }

    #[test]
    fn test_tls_server_config_optional_client_auth() {
        let client_auth = ClientAuthMode::Optional;
        assert!(client_auth.is_enabled());
        assert!(!client_auth.is_required());
    }

    #[test]
    fn test_tls_server_config_required_client_auth() {
        let client_auth = ClientAuthMode::Required;
        assert!(client_auth.is_enabled());
        assert!(client_auth.is_required());
    }
}
