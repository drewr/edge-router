//! mTLS (Mutual TLS) support for client certificate validation and service authentication
//!
//! Phase 4.8: Advanced certificate validation with metadata extraction and pinning

use rustls::pki_types::CertificateDer;
use rustls_pemfile::certs;
use std::io::BufReader;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::sync::Arc;
use lru::LruCache;
use std::sync::Mutex;

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

/// Extracted metadata from an X.509 certificate
#[derive(Clone, Debug, PartialEq)]
pub struct CertificateMetadata {
    /// Certificate's Common Name (CN)
    pub common_name: Option<String>,
    /// Subject Alternative Names (SANs)
    pub subject_alt_names: Vec<String>,
    /// Not before timestamp (seconds since epoch)
    pub not_before: u64,
    /// Not after timestamp (seconds since epoch)
    pub not_after: u64,
    /// Issuer's Common Name
    pub issuer_cn: Option<String>,
    /// SHA256 fingerprint of the certificate (hex-encoded)
    pub sha256_fingerprint: String,
}

impl CertificateMetadata {
    /// Check if the certificate is currently valid (not expired)
    pub fn is_valid_now(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now >= self.not_before && now <= self.not_after
    }

    /// Get days until certificate expiration (or negative if already expired)
    pub fn days_until_expiry(&self) -> i32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let seconds_until_expiry = (self.not_after as i64) - (now as i64);
        (seconds_until_expiry / 86400) as i32
    }
}

/// Certificate pinning configuration for service-to-service authentication
#[derive(Clone, Debug, PartialEq)]
pub struct CertificatePinner {
    /// Map of service names to acceptable certificate SHA256 fingerprints
    /// Multiple fingerprints per service (for key rotation)
    pins: HashMap<String, Vec<String>>,
}

impl CertificatePinner {
    /// Create a new empty certificate pinner
    pub fn new() -> Self {
        Self {
            pins: HashMap::new(),
        }
    }

    /// Add a certificate pin for a service
    /// Multiple pins per service support key rotation
    pub fn add_pin(&mut self, service: String, fingerprint: String) {
        debug!("Added certificate pin for service: {}", service);
        self.pins
            .entry(service)
            .or_insert_with(Vec::new)
            .push(fingerprint);
    }

    /// Load pins from a map (useful for environment-based config)
    pub fn load_from_map(pin_map: HashMap<String, Vec<String>>) -> Self {
        info!("Loaded {} certificate pins", pin_map.len());
        Self { pins: pin_map }
    }

    /// Verify a certificate against pinned fingerprints for a service
    pub fn verify(&self, service: &str, fingerprint: &str) -> Result<()> {
        match self.pins.get(service) {
            Some(allowed_fingerprints) => {
                if allowed_fingerprints.contains(&fingerprint.to_string()) {
                    debug!("Certificate pin verified for service: {}", service);
                    Ok(())
                } else {
                    warn!("Certificate pin verification failed for service: {} (fingerprint mismatch)", service);
                    Err(anyhow!(
                        "Certificate pin mismatch for service {}: fingerprint not in pin list",
                        service
                    ))
                }
            }
            None => {
                // No pins configured for this service, allow it
                debug!("No certificate pins configured for service: {}", service);
                Ok(())
            }
        }
    }

    /// Get the number of services with pinned certificates
    pub fn service_count(&self) -> usize {
        self.pins.len()
    }

    /// Check if pins are configured for a specific service
    pub fn has_pins_for_service(&self, service: &str) -> bool {
        self.pins.contains_key(service)
    }
}

impl Default for CertificatePinner {
    fn default() -> Self {
        Self::new()
    }
}

/// Certificate validation result with detailed information
#[derive(Clone, Debug, PartialEq)]
pub struct CertificateValidationResult {
    /// Whether the certificate passed validation
    pub valid: bool,
    /// Certificate metadata
    pub metadata: Option<CertificateMetadata>,
    /// Validation errors (if any)
    pub errors: Vec<String>,
    /// Validation warnings (e.g., expiring soon)
    pub warnings: Vec<String>,
}

impl CertificateValidationResult {
    /// Create a successful validation result
    pub fn success(metadata: CertificateMetadata) -> Self {
        Self {
            valid: true,
            metadata: Some(metadata),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failed validation result
    pub fn failure(error: String) -> Self {
        Self {
            valid: false,
            metadata: None,
            errors: vec![error],
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the validation result
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Add an error to the validation result
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.valid = false;
    }
}

/// Calculate SHA256 fingerprint of a certificate (hex-encoded)
pub fn calculate_cert_fingerprint(cert_der: &CertificateDer) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(&cert_der.as_ref());
    let hash = hasher.finalize();
    hex::encode(hash)
}

/// Certificate revocation status
#[derive(Clone, Debug, PartialEq)]
pub enum RevocationStatus {
    /// Certificate is valid (not revoked)
    Valid,
    /// Certificate has been revoked
    Revoked { reason: String, revoked_at: u64 },
    /// Revocation status unknown (no CRL/OCSP available)
    Unknown,
}

impl RevocationStatus {
    /// Check if the certificate is valid (not revoked)
    pub fn is_valid(&self) -> bool {
        matches!(self, RevocationStatus::Valid)
    }

    /// Check if the certificate is revoked
    pub fn is_revoked(&self) -> bool {
        matches!(self, RevocationStatus::Revoked { .. })
    }
}

/// CRL (Certificate Revocation List) Cache
/// Caches revocation status to avoid repeated CRL checks
#[derive(Clone)]
pub struct RevocationCache {
    /// LRU cache mapping certificate fingerprints to revocation status
    cache: Arc<Mutex<LruCache<String, RevocationStatus>>>,
}

impl RevocationCache {
    /// Create a new revocation cache with a given capacity
    pub fn new(capacity: usize) -> Result<Self> {
        Ok(Self {
            cache: Arc::new(Mutex::new(
                LruCache::new(std::num::NonZeroUsize::new(capacity)
                    .ok_or_else(|| anyhow!("Cache capacity must be greater than 0"))?),
            )),
        })
    }

    /// Get the revocation status for a certificate (from cache)
    pub fn get(&self, fingerprint: &str) -> Option<RevocationStatus> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.get(fingerprint).cloned()
    }

    /// Put a revocation status in the cache
    pub fn put(&self, fingerprint: String, status: RevocationStatus) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.put(fingerprint, status);
        debug!("Added revocation status to cache");
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.clear();
        info!("Revocation cache cleared");
    }

    /// Get cache statistics (for monitoring)
    pub fn stats(&self) -> (usize, usize) {
        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        (cache.len(), cache.cap().get())
    }
}

/// OCSP (Online Certificate Status Protocol) Configuration
#[derive(Clone, Debug)]
pub struct OcspConfig {
    /// OCSP responder URL (extracted from certificate)
    pub responder_url: String,
    /// Timeout for OCSP requests (in seconds)
    pub timeout: Duration,
    /// Whether to require OCSP response
    pub require_response: bool,
}

/// Revocation Status Request
#[derive(Clone, Debug)]
pub struct RevocationRequest {
    /// Certificate fingerprint (SHA256)
    pub cert_fingerprint: String,
    /// Issuer certificate fingerprint
    pub issuer_fingerprint: Option<String>,
    /// CRL distribution points from certificate
    pub crl_distribution_points: Vec<String>,
    /// OCSP configuration (if applicable)
    pub ocsp_config: Option<OcspConfig>,
}

/// Comprehensive revocation checker
#[derive(Clone)]
pub struct RevocationChecker {
    /// Cache for revocation results
    cache: RevocationCache,
    /// Maximum cache size
    cache_capacity: usize,
    /// Whether CRL checking is enabled
    enable_crl: bool,
    /// Whether OCSP checking is enabled
    enable_ocsp: bool,
}

impl RevocationChecker {
    /// Create a new revocation checker with given settings
    pub fn new(
        cache_capacity: usize,
        enable_crl: bool,
        enable_ocsp: bool,
    ) -> Result<Self> {
        let cache = RevocationCache::new(cache_capacity)?;
        info!(
            "Created revocation checker (CRL: {}, OCSP: {}, cache size: {})",
            enable_crl, enable_ocsp, cache_capacity
        );

        Ok(Self {
            cache,
            cache_capacity,
            enable_crl,
            enable_ocsp,
        })
    }

    /// Check the revocation status of a certificate
    pub fn check_revocation(&self, request: RevocationRequest) -> Result<RevocationStatus> {
        // Check cache first
        if let Some(cached_status) = self.cache.get(&request.cert_fingerprint) {
            debug!("Revocation status from cache: {:?}", cached_status);
            return Ok(cached_status);
        }

        // Default to Unknown if checking is disabled
        if !self.enable_crl && !self.enable_ocsp {
            debug!("Revocation checking disabled");
            let status = RevocationStatus::Unknown;
            self.cache.put(request.cert_fingerprint, status.clone());
            return Ok(status);
        }

        // Try OCSP first (faster, real-time)
        if self.enable_ocsp {
            if let Some(ocsp_config) = &request.ocsp_config {
                match self.check_ocsp(ocsp_config, &request.cert_fingerprint) {
                    Ok(status) => {
                        self.cache.put(request.cert_fingerprint.clone(), status.clone());
                        return Ok(status);
                    }
                    Err(e) => {
                        warn!("OCSP check failed: {}", e);
                        // Continue to CRL if OCSP fails
                    }
                }
            }
        }

        // Try CRL as fallback
        if self.enable_crl && !request.crl_distribution_points.is_empty() {
            match self.check_crl(&request.crl_distribution_points, &request.cert_fingerprint) {
                Ok(status) => {
                    self.cache.put(request.cert_fingerprint.clone(), status.clone());
                    return Ok(status);
                }
                Err(e) => {
                    warn!("CRL check failed: {}", e);
                }
            }
        }

        // Return Unknown if no checking method succeeded
        debug!("Revocation status unknown");
        let status = RevocationStatus::Unknown;
        self.cache.put(request.cert_fingerprint, status.clone());
        Ok(status)
    }

    /// Check revocation via OCSP (internal)
    fn check_ocsp(&self, _ocsp_config: &OcspConfig, _fingerprint: &str) -> Result<RevocationStatus> {
        // Phase 4.9: OCSP checking infrastructure
        // Full OCSP response parsing would be implemented here
        // For now, return Unknown to allow fallback to CRL
        debug!("OCSP checking not yet fully implemented");
        Ok(RevocationStatus::Unknown)
    }

    /// Check revocation via CRL (internal)
    fn check_crl(&self, _crl_points: &[String], _fingerprint: &str) -> Result<RevocationStatus> {
        // Phase 4.9: CRL checking infrastructure
        // Full CRL fetching and parsing would be implemented here
        // For now, return Unknown
        debug!("CRL checking not yet fully implemented");
        Ok(RevocationStatus::Unknown)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        self.cache.stats()
    }

    /// Clear the revocation cache
    pub fn clear_cache(&self) {
        self.cache.clear();
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

    // Phase 4.8: Advanced Certificate Validation Tests

    #[test]
    fn test_certificate_metadata_creation() {
        let metadata = CertificateMetadata {
            common_name: Some("example.com".to_string()),
            subject_alt_names: vec!["www.example.com".to_string(), "api.example.com".to_string()],
            not_before: 1000000,
            not_after: 2000000,
            issuer_cn: Some("Let's Encrypt".to_string()),
            sha256_fingerprint: "abc123def456".to_string(),
        };

        assert_eq!(metadata.common_name, Some("example.com".to_string()));
        assert_eq!(metadata.subject_alt_names.len(), 2);
        assert!(!metadata.sha256_fingerprint.is_empty());
    }

    #[test]
    fn test_certificate_metadata_validity() {
        // Create a metadata with validity in the past and future
        let far_past = 1000000u64;
        let far_future = 9999999999u64;

        let metadata = CertificateMetadata {
            common_name: None,
            subject_alt_names: vec![],
            not_before: far_past,
            not_after: far_future,
            issuer_cn: None,
            sha256_fingerprint: "test".to_string(),
        };

        // Should be valid (assuming current time is between far_past and far_future)
        assert!(metadata.is_valid_now());
    }

    #[test]
    fn test_certificate_metadata_expiry() {
        let metadata = CertificateMetadata {
            common_name: None,
            subject_alt_names: vec![],
            not_before: 0,
            not_after: 0,
            issuer_cn: None,
            sha256_fingerprint: "test".to_string(),
        };

        // Should be expired (not_after is 0)
        assert!(!metadata.is_valid_now());
        let days = metadata.days_until_expiry();
        assert!(days < 0); // Already expired
    }

    #[test]
    fn test_certificate_pinner_creation() {
        let pinner = CertificatePinner::new();
        assert_eq!(pinner.service_count(), 0);
    }

    #[test]
    fn test_certificate_pinner_add_pin() {
        let mut pinner = CertificatePinner::new();
        pinner.add_pin("service-a".to_string(), "fingerprint1".to_string());
        assert!(pinner.has_pins_for_service("service-a"));
        assert!(!pinner.has_pins_for_service("service-b"));
    }

    #[test]
    fn test_certificate_pinner_multiple_pins() {
        let mut pinner = CertificatePinner::new();
        pinner.add_pin("service-a".to_string(), "fingerprint1".to_string());
        pinner.add_pin("service-a".to_string(), "fingerprint2".to_string());

        // Should accept either fingerprint
        assert!(pinner.verify("service-a", "fingerprint1").is_ok());
        assert!(pinner.verify("service-a", "fingerprint2").is_ok());
        assert!(pinner.verify("service-a", "fingerprint3").is_err());
    }

    #[test]
    fn test_certificate_pinner_verify_success() {
        let mut pinner = CertificatePinner::new();
        pinner.add_pin("service-a".to_string(), "abc123".to_string());

        let result = pinner.verify("service-a", "abc123");
        assert!(result.is_ok());
    }

    #[test]
    fn test_certificate_pinner_verify_failure() {
        let mut pinner = CertificatePinner::new();
        pinner.add_pin("service-a".to_string(), "abc123".to_string());

        let result = pinner.verify("service-a", "wrong_fingerprint");
        assert!(result.is_err());
    }

    #[test]
    fn test_certificate_pinner_no_pins_configured() {
        let pinner = CertificatePinner::new();

        // Should succeed when no pins are configured
        let result = pinner.verify("service-a", "any_fingerprint");
        assert!(result.is_ok());
    }

    #[test]
    fn test_certificate_pinner_load_from_map() {
        let mut pin_map = HashMap::new();
        pin_map.insert("service-a".to_string(), vec!["fp1".to_string(), "fp2".to_string()]);
        pin_map.insert("service-b".to_string(), vec!["fp3".to_string()]);

        let pinner = CertificatePinner::load_from_map(pin_map);
        assert_eq!(pinner.service_count(), 2);
        assert!(pinner.has_pins_for_service("service-a"));
        assert!(pinner.has_pins_for_service("service-b"));
    }

    #[test]
    fn test_certificate_validation_result_success() {
        let metadata = CertificateMetadata {
            common_name: Some("test.com".to_string()),
            subject_alt_names: vec![],
            not_before: 0,
            not_after: 9999999999,
            issuer_cn: None,
            sha256_fingerprint: "test".to_string(),
        };

        let result = CertificateValidationResult::success(metadata.clone());
        assert!(result.valid);
        assert_eq!(result.metadata, Some(metadata));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_certificate_validation_result_failure() {
        let result = CertificateValidationResult::failure("Test error".to_string());
        assert!(!result.valid);
        assert_eq!(result.metadata, None);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_certificate_validation_result_with_warnings() {
        let metadata = CertificateMetadata {
            common_name: None,
            subject_alt_names: vec![],
            not_before: 0,
            not_after: 9999999999,
            issuer_cn: None,
            sha256_fingerprint: "test".to_string(),
        };

        let mut result = CertificateValidationResult::success(metadata);
        result.add_warning("Certificate expires in 30 days".to_string());

        assert!(result.valid);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_certificate_fingerprint_consistency() {
        let cert = CertificateDer::from(vec![1, 2, 3, 4, 5]);
        let fp1 = calculate_cert_fingerprint(&cert);
        let fp2 = calculate_cert_fingerprint(&cert);

        // Should be deterministic
        assert_eq!(fp1, fp2);
        // Should be hex-encoded
        assert!(fp1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_certificate_fingerprint_different() {
        let cert1 = CertificateDer::from(vec![1, 2, 3]);
        let cert2 = CertificateDer::from(vec![4, 5, 6]);

        let fp1 = calculate_cert_fingerprint(&cert1);
        let fp2 = calculate_cert_fingerprint(&cert2);

        // Different certificates should have different fingerprints
        assert_ne!(fp1, fp2);
    }

    // Phase 4.9: CRL/OCSP Revocation Checking Tests

    #[test]
    fn test_revocation_status_valid() {
        let status = RevocationStatus::Valid;
        assert!(status.is_valid());
        assert!(!status.is_revoked());
    }

    #[test]
    fn test_revocation_status_revoked() {
        let status = RevocationStatus::Revoked {
            reason: "Superseded".to_string(),
            revoked_at: 1000000,
        };
        assert!(!status.is_valid());
        assert!(status.is_revoked());
    }

    #[test]
    fn test_revocation_status_unknown() {
        let status = RevocationStatus::Unknown;
        assert!(!status.is_valid());
        assert!(!status.is_revoked());
    }

    #[test]
    fn test_revocation_cache_creation() {
        let cache = RevocationCache::new(100).expect("Cache creation failed");
        let (len, cap) = cache.stats();
        assert_eq!(len, 0);
        assert_eq!(cap, 100);
    }

    #[test]
    fn test_revocation_cache_put_get() {
        let cache = RevocationCache::new(100).expect("Cache creation failed");
        let fp = "abc123".to_string();
        let status = RevocationStatus::Valid;

        cache.put(fp.clone(), status.clone());
        let retrieved = cache.get(&fp);

        assert_eq!(retrieved, Some(status));
    }

    #[test]
    fn test_revocation_cache_lru_eviction() {
        let cache = RevocationCache::new(2).expect("Cache creation failed");

        cache.put("cert1".to_string(), RevocationStatus::Valid);
        cache.put("cert2".to_string(), RevocationStatus::Unknown);
        cache.put("cert3".to_string(), RevocationStatus::Valid);

        // cert1 should be evicted due to LRU
        assert_eq!(cache.get("cert1"), None);
        assert_eq!(cache.get("cert2"), Some(RevocationStatus::Unknown));
        assert_eq!(cache.get("cert3"), Some(RevocationStatus::Valid));
    }

    #[test]
    fn test_revocation_cache_clear() {
        let cache = RevocationCache::new(100).expect("Cache creation failed");
        cache.put("cert1".to_string(), RevocationStatus::Valid);
        cache.clear();

        assert_eq!(cache.get("cert1"), None);
        let (len, _) = cache.stats();
        assert_eq!(len, 0);
    }

    #[test]
    fn test_ocsp_config_creation() {
        let config = OcspConfig {
            responder_url: "http://ocsp.example.com".to_string(),
            timeout: Duration::from_secs(5),
            require_response: true,
        };

        assert_eq!(config.responder_url, "http://ocsp.example.com");
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert!(config.require_response);
    }

    #[test]
    fn test_revocation_request_creation() {
        let request = RevocationRequest {
            cert_fingerprint: "abc123".to_string(),
            issuer_fingerprint: Some("def456".to_string()),
            crl_distribution_points: vec!["http://crl.example.com/crl.pem".to_string()],
            ocsp_config: Some(OcspConfig {
                responder_url: "http://ocsp.example.com".to_string(),
                timeout: Duration::from_secs(5),
                require_response: true,
            }),
        };

        assert_eq!(request.cert_fingerprint, "abc123");
        assert_eq!(request.issuer_fingerprint, Some("def456".to_string()));
        assert_eq!(request.crl_distribution_points.len(), 1);
        assert!(request.ocsp_config.is_some());
    }

    #[test]
    fn test_revocation_checker_creation() {
        let checker = RevocationChecker::new(100, true, true)
            .expect("Revocation checker creation failed");
        let (len, cap) = checker.cache_stats();
        assert_eq!(len, 0);
        assert_eq!(cap, 100);
    }

    #[test]
    fn test_revocation_checker_disabled() {
        let checker = RevocationChecker::new(100, false, false)
            .expect("Revocation checker creation failed");

        let request = RevocationRequest {
            cert_fingerprint: "abc123".to_string(),
            issuer_fingerprint: None,
            crl_distribution_points: vec![],
            ocsp_config: None,
        };

        let status = checker
            .check_revocation(request)
            .expect("Check failed");
        assert_eq!(status, RevocationStatus::Unknown);
    }

    #[test]
    fn test_revocation_checker_caching() {
        let checker = RevocationChecker::new(100, false, false)
            .expect("Revocation checker creation failed");

        let request = RevocationRequest {
            cert_fingerprint: "abc123".to_string(),
            issuer_fingerprint: None,
            crl_distribution_points: vec![],
            ocsp_config: None,
        };

        // First call should return Unknown and cache it
        let status1 = checker
            .check_revocation(request.clone())
            .expect("Check failed");
        assert_eq!(status1, RevocationStatus::Unknown);

        // Second call should return from cache
        let status2 = checker
            .check_revocation(request)
            .expect("Check failed");
        assert_eq!(status2, RevocationStatus::Unknown);

        let (len, _) = checker.cache_stats();
        assert_eq!(len, 1); // Should be in cache
    }

    #[test]
    fn test_revocation_checker_clear_cache() {
        let checker = RevocationChecker::new(100, true, false)
            .expect("Revocation checker creation failed");

        let request = RevocationRequest {
            cert_fingerprint: "abc123".to_string(),
            issuer_fingerprint: None,
            crl_distribution_points: vec![],
            ocsp_config: None,
        };

        // Add to cache
        let _ = checker
            .check_revocation(request)
            .expect("Check failed");
        let (len_before, _) = checker.cache_stats();
        assert_eq!(len_before, 1);

        // Clear cache
        checker.clear_cache();
        let (len_after, _) = checker.cache_stats();
        assert_eq!(len_after, 0);
    }

    #[test]
    fn test_revocation_request_with_multiple_crl_points() {
        let request = RevocationRequest {
            cert_fingerprint: "abc123".to_string(),
            issuer_fingerprint: None,
            crl_distribution_points: vec![
                "http://crl1.example.com/crl.pem".to_string(),
                "http://crl2.example.com/crl.pem".to_string(),
            ],
            ocsp_config: None,
        };

        assert_eq!(request.crl_distribution_points.len(), 2);
    }
}
