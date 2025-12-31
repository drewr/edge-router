//! Load balancing strategies for distributing traffic across endpoints

use router_core::Endpoint;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Load balancing strategy
#[derive(Debug, Clone, PartialEq)]
pub enum LoadBalancingStrategy {
    /// Round-robin: distribute requests evenly across endpoints
    RoundRobin,
    /// Least connections: route to endpoint with fewest active connections
    LeastConnections,
    /// Source IP hash: route based on source IP for sticky sessions
    SourceIpHash,
    /// Consistent hash: hash-based routing for consistent endpoint selection
    ConsistentHash,
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        LoadBalancingStrategy::RoundRobin
    }
}

/// Load balancer for selecting endpoints based on a strategy
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    round_robin_counter: Arc<AtomicUsize>,
}

impl LoadBalancer {
    /// Create a new load balancer with the specified strategy
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Select an endpoint from the list based on the configured strategy
    pub fn select<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        if endpoints.is_empty() {
            return None;
        }

        // Filter to only ready endpoints
        let ready_endpoints: Vec<&'a Endpoint> = endpoints
            .iter()
            .filter(|e| e.ready)
            .collect();

        if ready_endpoints.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                self.select_round_robin(&ready_endpoints)
            }
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(&ready_endpoints)
            }
            LoadBalancingStrategy::SourceIpHash => {
                // For hash-based selection, we'd need to provide the source IP
                // For now, fall back to round-robin
                self.select_round_robin(&ready_endpoints)
            }
            LoadBalancingStrategy::ConsistentHash => {
                // For consistent hash, we'd need a hash key
                // For now, fall back to round-robin
                self.select_round_robin(&ready_endpoints)
            }
        }
    }

    /// Select endpoint using round-robin
    fn select_round_robin<'a>(&self, endpoints: &[&'a Endpoint]) -> Option<&'a Endpoint> {
        if endpoints.is_empty() {
            return None;
        }

        let current = self.round_robin_counter.fetch_add(1, Ordering::SeqCst);
        endpoints.get(current % endpoints.len()).copied()
    }

    /// Select endpoint with least connections (simplified: just use first ready endpoint)
    fn select_least_connections<'a>(&self, endpoints: &[&'a Endpoint]) -> Option<&'a Endpoint> {
        // Since we don't track active connections yet, just use the first ready endpoint
        endpoints.first().copied()
    }

    /// Hash-based endpoint selection for sticky sessions
    pub fn select_by_hash<'a>(&self, endpoints: &'a [Endpoint], hash_key: &str) -> Option<&'a Endpoint> {
        if endpoints.is_empty() {
            return None;
        }

        // Filter to only ready endpoints
        let ready_endpoints: Vec<&'a Endpoint> = endpoints
            .iter()
            .filter(|e| e.ready)
            .collect();

        if ready_endpoints.is_empty() {
            return None;
        }

        // Simple hash using string hash
        let hash = Self::compute_hash(hash_key);
        ready_endpoints.get(hash % ready_endpoints.len()).copied()
    }

    /// Compute hash for a string
    fn compute_hash(s: &str) -> usize {
        // Simple FNV-1a hash
        const FNV_OFFSET_BASIS: usize = 14695981039346656037;
        const FNV_PRIME: usize = 1099511628211;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.bytes() {
            hash ^= byte as usize;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}
