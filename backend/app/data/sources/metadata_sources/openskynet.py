import requests
from requests.exceptions import HTTPError, Timeout, ConnectionError
import logging
from typing import Optional
from ....core.models.aircraft import Aircraft
from . import AircraftMetadataSource
from .query_result import QueryResult

logger = logging.getLogger('OpenSky')


class OpenskyNet(AircraftMetadataSource):

    """ Opensky-Network """

    def __init__(self) -> None:

        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Macintosh; U; Intel Mac OS X 10_6_6; en-en) AppleWebKit/533.19.4 (KHTML, like Gecko) Version/5.0.3 Safari/533.19.4',
            "Content-type": "application/json",
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "de,en-US;q=0.7,en;q=0.3"
        }
        self.timeout = 5.0
        self.maxretries = 5

    @staticmethod
    def name() -> str:
        return 'Opensky'

    def accept(self, modes_address: str) -> bool:
        return True

    def _parse_aircraft_data(self, aircraft_data: dict) -> Optional[Aircraft]:
        """Parse aircraft data from API response"""
        if not aircraft_data:
            return None

        modeS = aircraft_data.get('icao24', '').upper()
        reg = aircraft_data.get('registration')
        icao_type_code = aircraft_data.get('typecode')
        op = aircraft_data.get('operator')
        model = aircraft_data.get('model')
        manufacturer_name = aircraft_data.get('manufacturerName')

        aircraft_type_description = None
        if model and manufacturer_name:
            aircraft_type_description = (model
                        if model.startswith(manufacturer_name)
                        else '{:s} {:s}'.format(manufacturer_name, model))
        elif model:
            aircraft_type_description = model
        elif manufacturer_name:
            aircraft_type_description = manufacturer_name

        # Return aircraft if we have at least some useful data
        # (relaxed from original which required all fields)
        if modeS and (reg or icao_type_code or aircraft_type_description):
            return Aircraft(modeS, reg=reg, icao_type_code=icao_type_code,
                          aircraft_type_description=aircraft_type_description,
                          operator=op, source=self.name())
        return None

    def query_aircraft_with_status(self, mode_s_hex: str) -> QueryResult:
        """
        Queries aircraft data from OpenSky Network with detailed status.

        Properly distinguishes between:
        - 404: Aircraft not found (permanent, don't retry)
        - 429/5xx: Service error (temporary, should retry)
        - Success: Data found
        """
        try:
            url = 'https://opensky-network.org/api/metadata/aircraft/icao/{:s}'.format(mode_s_hex)
            response = requests.get(url, headers=self.headers, timeout=self.timeout)

            # 404 = Aircraft definitively not in database
            if response.status_code == requests.codes.not_found:
                logger.debug(f'Aircraft {mode_s_hex} not found in OpenSky (404)')
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
            aircraft = self._parse_aircraft_data(aircraft_data)

            if aircraft:
                if aircraft.is_complete_with_operator():
                    return QueryResult.success(aircraft, raw_payload=aircraft_data)
                else:
                    return QueryResult.partial(aircraft, raw_payload=aircraft_data)

            # Empty response - treat as not found
            return QueryResult.not_found()

        except Timeout:
            logger.warning(f'Timeout querying OpenSky for {mode_s_hex}')
            return QueryResult.service_error('Request timeout')
        except ConnectionError as e:
            logger.warning(f'Connection error querying OpenSky for {mode_s_hex}: {e}')
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
        """queries aircraft data (legacy method)"""
        result = self.query_aircraft_with_status(mode_s_hex)
        return result.aircraft