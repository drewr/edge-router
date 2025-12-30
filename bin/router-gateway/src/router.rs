//! Router for matching requests to VPCRoutes and selecting backends

use router_core::ServiceRegistry;
use std::sync::Arc;
use tracing::debug;

/// Router for matching HTTP requests to VPCRoutes
pub struct Router {
    registry: Arc<ServiceRegistry>,
}

impl Router {
    /// Create a new router with a service registry
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }

    /// Match a request path against route patterns
    pub fn match_path(&self, path: &str, pattern: &str) -> bool {
        // Exact match
        if pattern == path {
            return true;
        }

        // Prefix match (pattern ends with / or /*)
        if pattern.ends_with('/') && path.starts_with(pattern) {
            return true;
        }

        // Prefix match with wildcard
        if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 2];
            return path == prefix || path.starts_with(&format!("{}/", prefix));
        }

        false
    }

    /// Match HTTP method against allowed methods
    pub fn match_method(&self, method: &str, allowed_methods: &[String]) -> bool {
        if allowed_methods.is_empty() {
            return true; // If no methods specified, match all
        }

        allowed_methods.iter().any(|m| m.eq_ignore_ascii_case(method))
    }

    /// Get the service registry
    pub fn registry(&self) -> &Arc<ServiceRegistry> {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_path_match() {
        let router = Router::new(Arc::new(router_core::ServiceRegistry::new()));
        assert!(router.match_path("/api/v1/users", "/api/v1/users"));
        assert!(!router.match_path("/api/v2/users", "/api/v1/users"));
    }

    #[test]
    fn test_prefix_path_match() {
        let router = Router::new(Arc::new(router_core::ServiceRegistry::new()));
        assert!(router.match_path("/api/v1/users", "/api/v1/"));
        assert!(router.match_path("/api/v1/", "/api/v1/"));
        assert!(!router.match_path("/api/v2/users", "/api/v1/"));
    }

    #[test]
    fn test_wildcard_path_match() {
        let router = Router::new(Arc::new(router_core::ServiceRegistry::new()));
        assert!(router.match_path("/api/v1/users", "/api/v1/*"));
        assert!(router.match_path("/api/v1/posts", "/api/v1/*"));
        assert!(router.match_path("/api/v1/", "/api/v1/*"));
        assert!(!router.match_path("/api/v2/users", "/api/v1/*"));
    }

    #[test]
    fn test_method_match() {
        let router = Router::new(Arc::new(router_core::ServiceRegistry::new()));
        let methods = vec!["GET".to_string(), "POST".to_string()];
        assert!(router.match_method("GET", &methods));
        assert!(router.match_method("get", &methods));
        assert!(router.match_method("POST", &methods));
        assert!(!router.match_method("DELETE", &methods));
    }

    #[test]
    fn test_method_match_empty() {
        let router = Router::new(Arc::new(router_core::ServiceRegistry::new()));
        let methods = vec![];
        assert!(router.match_method("GET", &methods));
        assert!(router.match_method("POST", &methods));
        assert!(router.match_method("ANY", &methods));
    }
}
