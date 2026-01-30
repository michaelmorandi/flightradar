from dataclasses import dataclass
from enum import Enum
from typing import Optional
from ....core.models.aircraft import Aircraft


class QueryStatus(Enum):
    """Status of a metadata query attempt"""
    SUCCESS = "success"           # Data found and returned
    NOT_FOUND = "not_found"       # Aircraft definitely not in source database (HTTP 404)
    SERVICE_ERROR = "service_error"  # Temporary error (5xx, timeout, rate limit, network)
    PARTIAL_DATA = "partial_data"    # Some data found but incomplete


@dataclass
class QueryResult:
    """
    Result of a metadata source query.

    Distinguishes between:
    - SUCCESS: Complete or partial data found
    - NOT_FOUND: Aircraft doesn't exist in the source (don't retry)
    - SERVICE_ERROR: Temporary failure, should retry later
    - PARTIAL_DATA: Some data found but incomplete
    """
    status: QueryStatus
    aircraft: Optional[Aircraft] = None
    error_message: Optional[str] = None

    @classmethod
    def success(cls, aircraft: Aircraft) -> 'QueryResult':
        """Create a successful result with aircraft data"""
        return cls(status=QueryStatus.SUCCESS, aircraft=aircraft)

    @classmethod
    def partial(cls, aircraft: Aircraft) -> 'QueryResult':
        """Create a result with partial aircraft data"""
        return cls(status=QueryStatus.PARTIAL_DATA, aircraft=aircraft)

    @classmethod
    def not_found(cls) -> 'QueryResult':
        """Create a not-found result (aircraft doesn't exist in source)"""
        return cls(status=QueryStatus.NOT_FOUND)

    @classmethod
    def service_error(cls, message: str = None) -> 'QueryResult':
        """Create a service error result (temporary failure, should retry)"""
        return cls(status=QueryStatus.SERVICE_ERROR, error_message=message)

    @property
    def is_success(self) -> bool:
        return self.status in (QueryStatus.SUCCESS, QueryStatus.PARTIAL_DATA)

    @property
    def is_retriable(self) -> bool:
        """Returns True if this error type should be retried later"""
        return self.status == QueryStatus.SERVICE_ERROR

    @property
    def is_permanent_failure(self) -> bool:
        """Returns True if we should not retry (aircraft doesn't exist)"""
        return self.status == QueryStatus.NOT_FOUND
