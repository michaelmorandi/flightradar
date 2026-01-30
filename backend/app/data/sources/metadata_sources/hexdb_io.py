import requests
from requests.exceptions import HTTPError, Timeout, ConnectionError
import logging
from typing import Optional

from ....core.models.aircraft import Aircraft
from . import AircraftMetadataSource
from .query_result import QueryResult

logger = logging.getLogger('HexdbIo')


class HexdbIo(AircraftMetadataSource):

    """ HexDB.io Aircraft Database """

    def __init__(self) -> None:

        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36',
            'Accept': 'application/json',
            'Accept-Language': 'en-US,en;q=0.9'
        }
        self.timeout = 5.0
        self.maxretries = 3

    @staticmethod
    def name() -> str:
        return 'HexDB.io'

    def accept(self, modes_address: str) -> bool:
        return True

    def _parse_aircraft_data(self, aircraft_data: dict, mode_s_hex: str) -> Optional[Aircraft]:
        """Parse aircraft data from API response"""
        if not aircraft_data:
            return None

        mode_s = aircraft_data.get('ModeS', mode_s_hex).upper()
        reg = aircraft_data.get('Registration')
        icao_type_code = aircraft_data.get('ICAOTypeCode')

        # Build aircraft_type_description from manufacturer and type
        manufacturer = aircraft_data.get('Manufacturer', '')
        aircraft_type = aircraft_data.get('Type', '')

        if manufacturer and aircraft_type:
            aircraft_type_description = f"{manufacturer} {aircraft_type}"
        elif manufacturer:
            aircraft_type_description = manufacturer
        elif aircraft_type:
            aircraft_type_description = aircraft_type
        else:
            aircraft_type_description = None

        operator = aircraft_data.get('RegisteredOwners')

        # Only return aircraft if we have at least registration or type info
        if reg or icao_type_code or aircraft_type_description:
            return Aircraft(mode_s, reg=reg, icao_type_code=icao_type_code,
                          aircraft_type_description=aircraft_type_description,
                          operator=operator, source=self.name())
        return None

    def query_aircraft_with_status(self, mode_s_hex: str) -> QueryResult:
        """
        Queries aircraft data from hexdb.io with detailed status.

        Properly distinguishes between:
        - 404: Aircraft not found (permanent, don't retry)
        - 429/5xx: Service error (temporary, should retry)
        - Success: Data found
        """
        try:
            url = f'https://hexdb.io/api/v1/aircraft/{mode_s_hex}'
            response = requests.get(url, headers=self.headers, timeout=self.timeout)

            # 404 = Aircraft definitively not in database
            if response.status_code == requests.codes.not_found:
                logger.debug(f'Aircraft {mode_s_hex} not found in HexDB.io (404)')
                return QueryResult.not_found()

            # Rate limiting - temporary error, should retry later
            if response.status_code == requests.codes.too_many:
                logger.warning(f'HTTP 429 - Rate limited for {mode_s_hex}')
                return QueryResult.service_error('Rate limited (429)')

            # Server errors - temporary, should retry
            if response.status_code >= 500:
                logger.warning(f'HTTP {response.status_code} - Server error for {mode_s_hex}')
                return QueryResult.service_error(f'Server error ({response.status_code})')

            response.raise_for_status()

            aircraft_data = response.json()
            aircraft = self._parse_aircraft_data(aircraft_data, mode_s_hex)

            if aircraft:
                if aircraft.is_complete_with_operator():
                    return QueryResult.success(aircraft)
                else:
                    return QueryResult.partial(aircraft)

            # Empty response - treat as not found
            return QueryResult.not_found()

        except Timeout:
            logger.warning(f'Timeout querying HexDB.io for {mode_s_hex}')
            return QueryResult.service_error('Request timeout')
        except ConnectionError as e:
            logger.warning(f'Connection error querying HexDB.io for {mode_s_hex}: {e}')
            return QueryResult.service_error('Connection error')
        except HTTPError as http_err:
            status_code = http_err.response.status_code if http_err.response else 0
            if status_code >= 500 or status_code == 429:
                return QueryResult.service_error(f'HTTP error ({status_code})')
            logger.exception(http_err)
            return QueryResult.service_error(f'HTTP error: {http_err}')
        except (KeyboardInterrupt, SystemExit):
            raise
        except Exception as e:
            logger.exception('An unexpected error occurred')
            return QueryResult.service_error(f'Unexpected error: {e}')

    def query_aircraft(self, mode_s_hex: str) -> Optional[Aircraft]:
        """queries aircraft data from hexdb.io (legacy method)"""
        result = self.query_aircraft_with_status(mode_s_hex)
        return result.aircraft