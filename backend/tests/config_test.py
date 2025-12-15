import unittest
from unittest.mock import patch
import os

from app.config import Config, ConfigSource, LoggingConfig


class ConfigTest(unittest.TestCase):

    def test_env_loading(self):
        "Test loading from env"

        env_vars = {
            'DATA_FOLDER': 'someresource',
            'SERVICE_URL': 'http://path/to/service',
            'SERVICE_TYPE': 'srvtype',
            'MIL_ONLY': 'true',
            'DB_RETENTION_MIN': '999',
            'UNKNOWN_AIRCRAFT_CRAWLING': 'true',
            'LOGGING_CONFIG': '{\"syslogHost\":\"log.server.com\",\"syslogFormat\":\"logformat:[%(name)s]%(message)s\",\"logLevel\":\"INFO\",\"logToConsole\":true}'
        }

        with patch.dict(os.environ, env_vars):      

            config = Config()
            self.assertEqual(ConfigSource.ENV, config.config_src)
            self.assertEqual('someresource', config.DATA_FOLDER)
            self.assertEqual('http://path/to/service', config.RADAR_SERVICE_URL)
            self.assertEqual('srvtype', config.RADAR_SERVICE_TYPE)
            self.assertEqual(True, config.MILTARY_ONLY)
            self.assertEqual(999, config.DB_RETENTION_MIN)
            self.assertEqual(True, config.UNKNOWN_AIRCRAFT_CRAWLING)
            self.assertTrue(isinstance(config.LOGGING_CONFIG, LoggingConfig) )

    