//! Traffic policies for request handling

use std::time::Duration;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tracing::debug;

/// Timeout policy for requests
#[derive(Clone, Debug)]
pub struct TimeoutPolicy {
    /// Total timeout for the request
    pub request_timeout: Duration,
    /// Connection timeout
    pub connect_timeout: Duration,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
        }
    }
}

/// Retry policy for failed requests
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: u32,
    /// HTTP status codes that trigger a retry
    pub retryable_status_codes: Vec<u16>,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retryable_status_codes: vec![502, 503, 504], // Bad Gateway, Service Unavailable, Gateway Timeout
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
        }
    }
}

impl RetryPolicy {
    /// Check if a status code should trigger a retry
    pub fn should_retry(&self, status: u16) -> bool {
        self.retryable_status_codes.contains(&status)
    }

    /// Calculate backoff duration for the given retry count
    pub fn backoff_duration(&self, retry_count: u32) -> Duration {
        let base = self.initial_backoff.as_millis() as u64;
        let exponential = 2u64.pow(retry_count);
        let backoff_ms = (base * exponential).min(self.max_backoff.as_millis() as u64);
        Duration::from_millis(backoff_ms)
    }
}

/// Circuit breaker states
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed - requests flow normally
    Closed,
    /// Circuit is open - requests are rejected
    Open,
    /// Circuit is half-open - test requests are allowed
    HalfOpen,
}

/// Circuit breaker for preventing cascading failures
pub struct CircuitBreaker {
    /// Current state
    state: Arc<AtomicU32>,
    /// Failure count
    failure_count: Arc<AtomicU32>,
    /// Success count (for half-open state)
    success_count: Arc<AtomicU32>,
    /// Configuration
    config: CircuitBreakerConfig,
}

/// Circuit breaker configuration
#[derive(Clone, Debug)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Success threshold before closing circuit (from half-open)
    pub success_threshold: u32,
    /// Duration to wait before trying half-open
    pub timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
        }
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(AtomicU32::new(CircuitState::Closed as u32)),
            failure_count: Arc::new(AtomicU32::new(0)),
            success_count: Arc::new(AtomicU32::new(0)),
            config,
        }
    }

    /// Get the current state
    pub fn state(&self) -> CircuitState {
        let state_u32 = self.state.load(Ordering::SeqCst);
        match state_u32 {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        let current_state = self.state();
        match current_state {
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if success_count >= self.config.success_threshold {
                    debug!("Circuit breaker: Closing circuit after {} successes", success_count);
                    self.state.store(CircuitState::Closed as u32, Ordering::SeqCst);
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        let current_state = self.state();
        match current_state {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if failure_count >= self.config.failure_threshold {
                    debug!("Circuit breaker: Opening circuit after {} failures", failure_count);
                    self.state.store(CircuitState::Open as u32, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::HalfOpen => {
                debug!("Circuit breaker: Opening circuit - failure during half-open");
                self.state.store(CircuitState::Open as u32, Ordering::SeqCst);
                self.failure_count.store(0, Ordering::SeqCst);
                self.success_count.store(0, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    /// Check if requests should be allowed
    pub fn can_attempt(&self) -> bool {
        self.state() != CircuitState::Open
    }

    /// Attempt to transition from Open to HalfOpen
    pub fn try_half_open(&self) {
        if self.state() == CircuitState::Open {
            debug!("Circuit breaker: Transitioning to half-open");
            self.state.store(CircuitState::HalfOpen as u32, Ordering::SeqCst);
        }
    }
}

/// Complete traffic policy configuration
#[derive(Clone, Debug)]
pub struct TrafficPolicy {
    pub timeout: TimeoutPolicy,
    pub retry: RetryPolicy,
    pub circuit_breaker: CircuitBreakerConfig,
}

impl Default for TrafficPolicy {
    fn default() -> Self {
        Self {
            timeout: TimeoutPolicy::default(),
            retry: RetryPolicy::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(502));
        assert!(policy.should_retry(503));
        assert!(policy.should_retry(504));
        assert!(!policy.should_retry(200));
        assert!(!policy.should_retry(404));
    }

    #[test]
    fn test_retry_policy_backoff() {
        let policy = RetryPolicy::default();
        let backoff1 = policy.backoff_duration(0);
        let backoff2 = policy.backoff_duration(1);
        let backoff3 = policy.backoff_duration(2);

        // Each backoff should be exponentially longer
        assert!(backoff2 > backoff1);
        assert!(backoff3 > backoff2);
    }

    #[test]
    fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        assert_eq!(cb.state(), CircuitState::Closed);

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Third failure should open circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Transition to half-open
        cb.try_half_open();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record success
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
