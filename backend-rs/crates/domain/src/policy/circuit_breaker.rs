//! Per-source circuit breaker with exponential backoff. Pure state machine —
//! callers pass the current time on every transition, which makes the type
//! deterministic under test.

use std::time::Duration;

use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub base_reset: Duration,
    pub max_reset: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            base_reset: Duration::from_secs(60),
            max_reset: Duration::from_secs(1800),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub total_failures: u64,
    pub total_successes: u64,
    pub trip_count: u32,
    pub current_backoff: Duration,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitState,
    consecutive_failures: u32,
    total_failures: u64,
    total_successes: u64,
    trip_count: u32,
    last_failure_at: Option<OffsetDateTime>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            consecutive_failures: 0,
            total_failures: 0,
            total_successes: 0,
            trip_count: 0,
            last_failure_at: None,
        }
    }

    fn current_backoff(&self) -> Duration {
        if self.trip_count == 0 {
            return self.config.base_reset;
        }
        let exp = self.trip_count.saturating_sub(1);
        let scaled = self
            .config
            .base_reset
            .saturating_mul(2_u32.saturating_pow(exp));
        std::cmp::min(scaled, self.config.max_reset)
    }

    /// Returns `true` if calls should be allowed. May mutate the breaker
    /// (`Open` → `HalfOpen` transition once backoff elapses).
    pub fn allow(&mut self, now: OffsetDateTime) -> bool {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                let elapsed_ok = self.last_failure_at.is_some_and(|t| {
                    let backoff = self.current_backoff();
                    now - t >= time::Duration::try_from(backoff).unwrap_or(time::Duration::ZERO)
                });
                if elapsed_ok {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn record_success(&mut self) {
        self.total_successes += 1;
        if self.state == CircuitState::HalfOpen {
            self.trip_count = 0; // reset exponential backoff
        }
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
    }

    pub fn record_failure(&mut self, now: OffsetDateTime) {
        self.total_failures += 1;
        self.consecutive_failures += 1;
        self.last_failure_at = Some(now);

        match self.state {
            CircuitState::HalfOpen => {
                self.trip_count = self.trip_count.saturating_add(1);
                self.state = CircuitState::Open;
            }
            CircuitState::Closed if self.consecutive_failures >= self.config.failure_threshold => {
                self.trip_count = self.trip_count.saturating_add(1);
                self.state = CircuitState::Open;
            }
            _ => {}
        }
    }

    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: self.state,
            consecutive_failures: self.consecutive_failures,
            total_failures: self.total_failures,
            total_successes: self.total_successes,
            trip_count: self.trip_count,
            current_backoff: self.current_backoff(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cb() -> CircuitBreaker {
        CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            base_reset: Duration::from_secs(10),
            max_reset: Duration::from_secs(60),
        })
    }

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    #[test]
    fn closed_allows_calls() {
        let mut c = cb();
        assert!(c.allow(t(0)));
        assert_eq!(c.stats().state, CircuitState::Closed);
    }

    #[test]
    fn opens_after_threshold_failures() {
        let mut c = cb();
        for i in 0..3 {
            c.record_failure(t(i));
        }
        assert_eq!(c.stats().state, CircuitState::Open);
        assert!(!c.allow(t(3)));
    }

    #[test]
    fn half_open_after_backoff_then_closes_on_success() {
        let mut c = cb();
        for i in 0..3 {
            c.record_failure(t(i));
        }
        assert!(!c.allow(t(5))); // still within base_reset (10s)
        assert!(c.allow(t(15))); // base_reset elapsed
        assert_eq!(c.stats().state, CircuitState::HalfOpen);

        c.record_success();
        assert_eq!(c.stats().state, CircuitState::Closed);
        assert_eq!(c.stats().trip_count, 0); // exponential reset
    }

    #[test]
    fn half_open_failure_reopens_with_longer_backoff() {
        let mut c = cb();
        for i in 0..3 {
            c.record_failure(t(i));
        }
        let _ = c.allow(t(15)); // → HalfOpen
        c.record_failure(t(15));
        assert_eq!(c.stats().state, CircuitState::Open);
        assert_eq!(c.stats().trip_count, 2);
        assert_eq!(c.stats().current_backoff, Duration::from_secs(20));
    }

    #[test]
    fn backoff_capped_at_max() {
        let mut c = cb();
        for _ in 0..20 {
            for _ in 0..3 {
                c.record_failure(t(0));
            }
            let _ = c.allow(t(1_000_000));
            c.record_failure(t(1_000_000));
        }
        assert_eq!(c.stats().current_backoff, Duration::from_secs(60));
    }

    #[test]
    fn success_resets_consecutive_counter() {
        let mut c = cb();
        c.record_failure(t(0));
        c.record_failure(t(1));
        c.record_success();
        assert_eq!(c.stats().consecutive_failures, 0);
        assert_eq!(c.stats().total_failures, 2);
        assert_eq!(c.stats().total_successes, 1);
    }
}
