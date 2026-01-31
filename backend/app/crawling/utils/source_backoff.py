import time
import logging
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict

logger = logging.getLogger(__name__)


class CircuitState(Enum):
    """State of a circuit breaker"""
    CLOSED = "closed"  # Normal operation, requests allowed
    OPEN = "open"      # Failing, requests blocked
    HALF_OPEN = "half_open"  # Testing if service recovered


@dataclass
class SourceBackoff:
    """Tracks exponential backoff for a specific metadata source"""
    retry_count: int = 0
    last_attempt: float = 0.0

    def can_retry_now(self) -> bool:
        """Check if enough time has passed for exponential backoff"""
        if self.retry_count == 0:
            return True

        # Exponential backoff: 2^retry_count seconds (capped at 300 seconds = 5 minutes)
        backoff_seconds = min(2 ** self.retry_count, 300)
        return time.time() - self.last_attempt >= backoff_seconds

    def record_failure(self) -> None:
        """Record a failed attempt"""
        self.retry_count += 1
        self.last_attempt = time.time()

    def reset(self) -> None:
        """Reset backoff on success"""
        self.retry_count = 0
        self.last_attempt = 0.0


@dataclass
class CircuitBreaker:
    """
    Circuit breaker for external metadata sources.

    Prevents hammering failing services by tracking consecutive failures
    and temporarily blocking requests when a threshold is reached.

    Uses exponential backoff: each time the circuit trips, the reset time
    doubles (starting from base_reset_seconds, capped at max_reset_seconds).

    States:
    - CLOSED: Normal operation, requests allowed
    - OPEN: Service failing, requests blocked until backoff elapsed
    - HALF_OPEN: Testing if service recovered (one request allowed)
    """
    failure_threshold: int = 5      # Failures before opening circuit
    base_reset_seconds: int = 60    # Initial backoff time (1 min)
    max_reset_seconds: int = 1800   # Maximum backoff time (30 min)

    consecutive_failures: int = field(default=0, init=False)
    last_failure_time: float = field(default=0.0, init=False)
    state: CircuitState = field(default=CircuitState.CLOSED, init=False)
    total_failures: int = field(default=0, init=False)
    total_successes: int = field(default=0, init=False)
    trip_count: int = field(default=0, init=False)  # Number of times circuit has opened

    def _get_current_reset_seconds(self) -> int:
        """Calculate current reset time with exponential backoff"""
        if self.trip_count == 0:
            return self.base_reset_seconds
        # Exponential backoff: base * 2^(trip_count-1), capped at max
        backoff = self.base_reset_seconds * (2 ** (self.trip_count - 1))
        return min(backoff, self.max_reset_seconds)

    def is_available(self) -> bool:
        """Check if requests should be allowed through"""
        if self.state == CircuitState.CLOSED:
            return True

        if self.state == CircuitState.OPEN:
            # Check if enough time has passed to try again (with exponential backoff)
            reset_seconds = self._get_current_reset_seconds()
            if time.time() - self.last_failure_time >= reset_seconds:
                self.state = CircuitState.HALF_OPEN
                logger.info(f"Circuit breaker entering HALF_OPEN state after {reset_seconds}s backoff, testing service")
                return True
            return False

        # HALF_OPEN - allow one request to test
        return True

    def record_success(self) -> None:
        """Record a successful request"""
        self.total_successes += 1
        if self.state == CircuitState.HALF_OPEN:
            logger.info("Circuit breaker closing after successful test, resetting backoff")
            self.trip_count = 0  # Reset exponential backoff on successful recovery
        self.state = CircuitState.CLOSED
        self.consecutive_failures = 0

    def record_failure(self) -> None:
        """Record a failed request"""
        self.total_failures += 1
        self.consecutive_failures += 1
        self.last_failure_time = time.time()

        if self.state == CircuitState.HALF_OPEN:
            # Failed during test, reopen circuit with increased backoff
            self.trip_count += 1
            self.state = CircuitState.OPEN
            reset_seconds = self._get_current_reset_seconds()
            logger.warning(f"Circuit breaker reopening after failed test, backoff now {reset_seconds}s (trip #{self.trip_count})")
        elif self.consecutive_failures >= self.failure_threshold:
            if self.state != CircuitState.OPEN:
                self.trip_count += 1
                reset_seconds = self._get_current_reset_seconds()
                logger.warning(
                    f"Circuit breaker opening after {self.consecutive_failures} consecutive failures, "
                    f"backoff {reset_seconds}s (trip #{self.trip_count})"
                )
            self.state = CircuitState.OPEN

    def get_stats(self) -> dict:
        """Get circuit breaker statistics"""
        reset_seconds = self._get_current_reset_seconds()
        return {
            "state": self.state.value,
            "consecutive_failures": self.consecutive_failures,
            "total_failures": self.total_failures,
            "total_successes": self.total_successes,
            "trip_count": self.trip_count,
            "current_backoff_seconds": reset_seconds,
            "seconds_until_retry": max(0, reset_seconds - (time.time() - self.last_failure_time))
            if self.state == CircuitState.OPEN else 0
        }


class CircuitBreakerRegistry:
    """
    Registry of circuit breakers for multiple sources.

    Each metadata source gets its own circuit breaker to track
    failures independently.
    """

    def __init__(
        self,
        failure_threshold: int = 5,
        base_reset_seconds: int = 60,
        max_reset_seconds: int = 1800
    ):
        self.failure_threshold = failure_threshold
        self.base_reset_seconds = base_reset_seconds
        self.max_reset_seconds = max_reset_seconds
        self._breakers: Dict[str, CircuitBreaker] = {}

    def get_breaker(self, source_name: str) -> CircuitBreaker:
        """Get or create a circuit breaker for a source"""
        if source_name not in self._breakers:
            self._breakers[source_name] = CircuitBreaker(
                failure_threshold=self.failure_threshold,
                base_reset_seconds=self.base_reset_seconds,
                max_reset_seconds=self.max_reset_seconds
            )
        return self._breakers[source_name]

    def is_source_available(self, source_name: str) -> bool:
        """Check if a source is available (circuit not open)"""
        return self.get_breaker(source_name).is_available()

    def record_success(self, source_name: str) -> None:
        """Record a successful request to a source"""
        self.get_breaker(source_name).record_success()

    def record_failure(self, source_name: str) -> None:
        """Record a failed request to a source"""
        self.get_breaker(source_name).record_failure()

    def get_all_stats(self) -> Dict[str, dict]:
        """Get statistics for all circuit breakers"""
        return {name: breaker.get_stats() for name, breaker in self._breakers.items()}