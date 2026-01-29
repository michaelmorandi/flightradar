import requests
from requests.exceptions import HTTPError
import time
import logging
from typing import Optional

from app.core.models.aircraft import Aircraft
from ..metadata_sources import AircraftMetadataSource

logger = logging.getLogger('NighthawkProxy')


class NighthawkProxy(AircraftMetadataSource):

    """ Nighthawk Proxy Aircraft Lookup API Queries """

    def __init__(self, base_url: str = "http://localhost:8000") -> None:
        self.base_url = base_url.rstrip('/')
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:62.0) Gecko/20100101 Firefox/62.0',
            "Content-type": "application/json",
            "Accept": "application/json"
        }
        self.maxretries = 5
        self.timeout = 30.0
        self.failcounter = 0

    @staticmethod
    def name() -> str:
        return 'NighthawkProxy'

    def accept(self, modes_address: str) -> bool:
        return True

    @staticmethod
    def _get_timeout_sec(retry_attempt: int) -> int:
        """ Seconds in function of retry attempt, basically a Fibonacci seq with offset """
        n = 6 + retry_attempt
        a, b = 1, 1
        for i in range(n - 1):
            a, b = b, a + b
        return a

    def _fail_and_sleep(self) -> None:
        seconds = NighthawkProxy._get_timeout_sec(self.failcounter)
        logger.warning('Sleeping for {:d}sec'.format(seconds))
        time.sleep(seconds)
        self.failcounter += 1

    def query_aircraft(self, mode_s_hex: str) -> Optional[Aircraft]:
        """ Queries aircraft data by ICAO hex address """

        self.failcounter = 0
        while self.failcounter < self.maxretries:
            try:
                url = f'{self.base_url}/aircraft/{mode_s_hex}'
                response = requests.get(url, headers=self.headers, timeout=self.timeout)

                if response.status_code == requests.codes.ok:
                    data = response.json()
                    return Aircraft(
                        modeShex=data['icao'],
                        reg=data.get('registration'),
                        icao_type_code=data.get('type_code'),
                        aircraft_type_description=data.get('type_description'),
                        operator=data.get('owner'),
                        source=self.name()
                    )
                elif response.status_code == 404:
                    logger.debug(f'Aircraft not found: {mode_s_hex}')
                    return None
                elif response.status_code == 503:
                    logger.warning('HTTP 503 - Service unavailable')
                    self._fail_and_sleep()
                elif response.status_code == requests.codes.too_many:
                    logger.warning('HTTP 429 - Too many requests')
                    self._fail_and_sleep()
                elif response.status_code >= 500 and response.status_code < 600:
                    self._fail_and_sleep()
                else:
                    logger.warning(f'Unexpected http code: {response.status_code}')
                    return None

            except HTTPError as http_err:
                logger.exception(http_err)
                self._fail_and_sleep()
            except (KeyboardInterrupt, SystemExit):
                raise
            except Exception:
                logger.exception('An unexpected error occurred')
                self._fail_and_sleep()

        if self.failcounter == self.maxretries:
            logger.warning(f'Too many failures for {mode_s_hex}, giving up')

        return None
