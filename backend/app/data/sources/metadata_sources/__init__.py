from abc import ABC, abstractmethod
from typing import Optional
from ....core.models.aircraft import Aircraft
from .query_result import QueryResult, QueryStatus


class AircraftMetadataSource(ABC):
    """
    Abstract base class for aircraft metadata sources.

    This interface defines the contract that all aircraft metadata sources must implement.
    Similar to Java interfaces, this ensures consistent behavior across different data sources.
    """

    @staticmethod
    @abstractmethod
    def name() -> str:
        """
        Returns the name of the data source.

        Returns:
            str: A human-readable name for this data source
        """
        pass

    @abstractmethod
    def accept(self, modes_address: str) -> bool:
        """
        Determines if this data source can handle the given Mode S address.

        Args:
            modes_address: The Mode S hex address to check

        Returns:
            bool: True if this source can handle the address, False otherwise
        """
        pass

    @abstractmethod
    def query_aircraft(self, mode_s_hex: str) -> Optional[Aircraft]:
        """
        Queries aircraft metadata for the given Mode S hex address.

        Args:
            mode_s_hex: The Mode S hex address to look up

        Returns:
            Aircraft: Aircraft object with metadata if found, None otherwise
        """
        pass

    def query_aircraft_with_status(self, mode_s_hex: str) -> QueryResult:
        """
        Queries aircraft metadata with detailed status information.

        This method distinguishes between:
        - Aircraft not found (404) - permanent, don't retry
        - Service error (5xx, timeout, rate limit) - temporary, should retry
        - Success - data found

        Default implementation wraps query_aircraft() for backward compatibility.
        Subclasses should override this method for proper error classification.

        Args:
            mode_s_hex: The Mode S hex address to look up

        Returns:
            QueryResult: Result with status and optional aircraft data
        """
        # Default implementation for backward compatibility
        aircraft = self.query_aircraft(mode_s_hex)
        if aircraft:
            if aircraft.is_complete_with_operator():
                return QueryResult.success(aircraft)
            else:
                return QueryResult.partial(aircraft)
        return QueryResult.not_found()