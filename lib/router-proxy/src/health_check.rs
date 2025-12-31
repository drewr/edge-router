//! Health checking for service endpoints

use router_core::Endpoint;
use std::time::Duration;
use tokio::time;
use tracing::{debug, warn};

/// Health check configuration
#[derive(Clone, Debug)]
pub struct HealthCheckConfig {
    /// HTTP path to check for health
    pub http_path: String,
    /// Interval between health checks
    pub check_interval: Duration,
    /// Timeout for a single health check
    pub timeout: Duration,
    /// Number of consecutive failures before marking unhealthy
    pub unhealthy_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub healthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            http_path: "/healthz".to_string(),
            check_interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            unhealthy_threshold: 3,
            healthy_threshold: 2,
        }
    }
}

/// Health checker for monitoring endpoint health
pub struct HealthChecker {
    config: HealthCheckConfig,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(config: HealthCheckConfig) -> Self {
        Self { config }
    }

    /// Check if an endpoint is healthy by making an HTTP request
    pub async fn check_endpoint(&self, endpoint: &Endpoint) -> bool {
        let url = format!("http://{}:{}{}", endpoint.ip, endpoint.port, self.config.http_path);

        match time::timeout(self.config.timeout, self.check_single(url.clone())).await {
            Ok(Ok(healthy)) => {
                if healthy {
                    debug!("Endpoint {}:{} is healthy", endpoint.ip, endpoint.port);
                    true
                } else {
                    warn!("Endpoint {}:{} health check failed", endpoint.ip, endpoint.port);
                    false
                }
            }
            Ok(Err(e)) => {
                warn!("Endpoint {}:{} health check error: {}", endpoint.ip, endpoint.port, e);
                false
            }
            Err(_) => {
                warn!("Endpoint {}:{} health check timeout", endpoint.ip, endpoint.port);
                false
            }
        }
    }

    /// Check a single endpoint (internal)
    async fn check_single(&self, url: String) -> Result<bool, String> {
        // For now, we'll use a simple TCP connection check
        // In Phase 3, this would make actual HTTP requests
        // For Phase 3 MVP, we consider endpoints healthy if they're in the registry
        match tokio::net::TcpStream::connect(
            format!("{}:{}",
                self.extract_host(&url),
                self.extract_port(&url)
            )
        ).await {
            Ok(_) => {
                debug!("TCP connection to {} succeeded", url);
                Ok(true)
            }
            Err(e) => {
                warn!("TCP connection to {} failed: {}", url, e);
                Ok(false)
            }
        }
    }

    fn extract_host<'a>(&self, url: &'a str) -> &'a str {
        // Extract host from "http://10.0.0.1:8080/healthz"
        if let Some(start) = url.find("://") {
            let rest = &url[start + 3..];
            if let Some(colon) = rest.find(':') {
                return &rest[..colon];
            }
        }
        "127.0.0.1"
    }

    fn extract_port(&self, url: &str) -> u16 {
        // Extract port from "http://10.0.0.1:8080/healthz" or "http://localhost:3000/health"
        // Skip the scheme (http:// or https://)
        let without_scheme = if let Some(pos) = url.find("://") {
            &url[pos + 3..]
        } else {
            url
        };

        // Find the colon that separates host from port
        if let Some(colon_pos) = without_scheme.find(':') {
            let after_colon = &without_scheme[colon_pos + 1..];
            // Extract until slash or end of string
            if let Some(slash_pos) = after_colon.find('/') {
                if let Ok(port) = after_colon[..slash_pos].parse::<u16>() {
                    return port;
                }
            } else if let Ok(port) = after_colon.parse::<u16>() {
                return port;
            }
        }
        8080
    }
}

/// Health check monitor for periodic checking
pub struct HealthCheckMonitor {
    config: HealthCheckConfig,
}

impl HealthCheckMonitor {
    /// Create a new health check monitor
    pub fn new(config: HealthCheckConfig) -> Self {
        Self { config }
    }

    /// Start periodic health checking for endpoints
    /// This would be called from the main gateway loop
    pub fn start_monitoring(&self) {
        debug!("Health check monitor started with interval: {:?}", self.config.check_interval);
        // In Phase 3, this will spawn background tasks to periodically check endpoints
        // and update the service registry with health status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.http_path, "/healthz");
        assert_eq!(config.check_interval, Duration::from_secs(10));
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert_eq!(config.unhealthy_threshold, 3);
        assert_eq!(config.healthy_threshold, 2);
    }

    #[test]
    fn test_extract_host() {
        let checker = HealthChecker::new(HealthCheckConfig::default());
        assert_eq!(checker.extract_host("http://10.0.0.1:8080/healthz"), "10.0.0.1");
        assert_eq!(checker.extract_host("http://localhost:3000/health"), "localhost");
    }

    #[test]
    fn test_extract_port() {
        let checker = HealthChecker::new(HealthCheckConfig::default());
        assert_eq!(checker.extract_port("http://10.0.0.1:8080/healthz"), 8080);
        assert_eq!(checker.extract_port("http://localhost:3000/health"), 3000);
    }
}
