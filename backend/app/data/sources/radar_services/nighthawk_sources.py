"""
Nighthawk Proxy Source Implementation

Generic metadata source that queries nighthawk-proxy's per-source endpoints.
Sources are discovered dynamically from the proxy's /sources endpoint and
sorted by priority.
"""

import requests
from requests.exceptions import Timeout, ConnectionError
import logging
from typing import Optional, List

from app.core.models.aircraft import Aircraft
from ..metadata_sources import AircraftMetadataSource
from ..metadata_sources.query_result import QueryResult


logger = logging.getLogger('NighthawkSources')


class NighthawkSource(AircraftMetadataSource):
    """
    Generic Nighthawk proxy source implementation.

    Queries a specific source endpoint on the nighthawk proxy.
    Source endpoint is configured at construction time.
    """

    def __init__(self, base_url: str, source_endpoint: str, priority: int = 0) -> None:
        self.base_url = base_url.rstrip('/')
        self.source_endpoint = source_endpoint
        self.priority = priority
        self._name = f"Nighthawk:{source_endpoint}"
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:62.0) Gecko/20100101 Firefox/62.0',
            "Content-type": "application/json",
            "Accept": "application/json"
        }
        self.timeout = 30.0

    def name(self) -> str:
        return self._name

    def accept(self, modes_address: str) -> bool:
        return True

    def _build_url(self, icao: str) -> str:
        """Build the URL for the specific source endpoint."""
        return f'{self.base_url}/aircraft/source/{self.source_endpoint}/{icao}'

    def query_aircraft_with_status(self, mode_s_hex: str) -> QueryResult:
        """
        Query aircraft with proper status classification.

        Returns:
            - QueryResult.not_found() for 404 (aircraft doesn't exist)
            - QueryResult.service_error() for 5xx, timeout, connection errors
            - QueryResult.success/partial() for successful lookups
        """
        try:
            url = self._build_url(mode_s_hex)
            response = requests.get(url, headers=self.headers, timeout=self.timeout)

            if response.status_code == requests.codes.ok:
                data = response.json()
                aircraft = Aircraft(
                    modeShex=data['icao'],
                    reg=data.get('registration'),
                    icao_type_code=data.get('type_code'),
                    aircraft_type_description=data.get('type_description'),
                    operator=data.get('owner'),
                    source=f"nighthawk-{self.source_endpoint}"
                )
                if aircraft.is_complete_with_operator():
                    return QueryResult.success(aircraft, raw_payload=data)
                return QueryResult.partial(aircraft, raw_payload=data)

            elif response.status_code == 404:
                logger.debug(f'Aircraft not found in {self._name}: {mode_s_hex}')
                return QueryResult.not_found()

            elif response.status_code == 429:
                logger.warning(f'HTTP 429 - Too many requests from {self._name}')
                return QueryResult.service_error('Rate limited (429)')

            elif response.status_code == 503:
                logger.warning(f'HTTP 503 - {self._name} service unavailable')
                return QueryResult.service_error(f'{self._name} unavailable (503)')

            elif 500 <= response.status_code < 600:
                logger.warning(f'HTTP {response.status_code} - Server error from {self._name}')
                return QueryResult.service_error(f'Server error ({response.status_code})')

            else:
                logger.warning(f'Unexpected http code from {self._name}: {response.status_code}')
                return QueryResult.not_found()

        except (Timeout, ConnectionError) as e:
            logger.warning(f'Connection error for {mode_s_hex} from {self._name}: {e}')
            return QueryResult.service_error(f'Connection error: {type(e).__name__}')

        except (KeyboardInterrupt, SystemExit):
            raise

        except Exception as e:
            logger.exception(f'Unexpected error for {mode_s_hex} from {self._name}')
            return QueryResult.service_error(f'Unexpected error: {type(e).__name__}')

    def query_aircraft(self, mode_s_hex: str) -> Optional[Aircraft]:
        """
        Legacy method for backward compatibility.
        Prefer using query_aircraft_with_status() for proper error handling.
        """
        result = self.query_aircraft_with_status(mode_s_hex)
        return result.aircraft


def get_nighthawk_sources(base_url: str) -> List[AircraftMetadataSource]:
    """
    Discover and return all available sources from nighthawk proxy.

    Queries the /sources endpoint to find which sources are available,
    creates source instances, and returns them sorted by priority
    (lower priority number = higher precedence).

    Args:
        base_url: The nighthawk proxy base URL

    Returns:
        List of AircraftMetadataSource instances sorted by priority
    """
    sources = []

    try:
        response = requests.get(
            f"{base_url.rstrip('/')}/sources",
            timeout=10,
            headers={"Accept": "application/json"}
        )

        if response.status_code == 200:
            data = response.json()
            for source_info in data.get("sources", []):
                source_name = source_info.get("name")
                priority = source_info.get("priority", 100)
                if source_name:
                    sources.append(NighthawkSource(
                        base_url=base_url,
                        source_endpoint=source_name,
                        priority=priority
                    ))
                    logger.info(f"Discovered nighthawk source: {source_name} (priority={priority})")

            # Sort by priority (lower number = higher precedence)
            sources.sort(key=lambda s: s.priority)
        else:
            logger.warning(f"Failed to discover nighthawk sources: HTTP {response.status_code}")

    except Exception as e:
        logger.warning(f"Failed to discover nighthawk sources: {e}")

    return sources
