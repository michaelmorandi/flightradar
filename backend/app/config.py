import json
import os
import logging
from enum import Enum
from pathlib import Path
from dotenv import load_dotenv

load_dotenv()

logger = logging.getLogger("Config")

class AppState:
    mongodb = None
    
app_state = AppState()

class LoggingConfig:

    syslogHost = None
    syslogFormat = None
    logToConsole = True

    @staticmethod
    def from_json(json):
        logToConsole = True
        logLevel = logging.INFO
        syslogHost = json['syslogHost'] if 'syslogHost' in json else None
        syslogFormat = json['syslogFormat'] if 'syslogFormat' in json else None
        
        if 'logLevel' in json:
            logLevel = logging.getLevelName(json['logLevel'].strip().upper())

        if bool(syslogHost) != bool(syslogFormat):
            raise ValueError('Incomplete logging config')     

        if 'logToConsole' in json:
            logToConsole = json['logToConsole']

        return LoggingConfig(syslogHost, syslogFormat, logToConsole, logLevel)

    def __init__(self, syslogHost, syslogFormat, logToConsole=True, logLevel=logging.INFO):
        self.syslogHost = syslogHost
        self.syslogFormat = syslogFormat
        self.logLevel = logLevel
        self.logToConsole = logToConsole

class ConfigSource(Enum):
    NONE = 0
    ENV = 2

class Config:

    """ Application configuration """

    DATA_FOLDER = 'resources'
    RADAR_SERVICE_URL = 'http://flightlive.gotdns.ch:8084/VirtualRadar'
    RADAR_SERVICE_TYPE = 'vrs'
    MILTARY_ONLY = False
    DB_RETENTION_MIN = 1440
    LOGGING_CONFIG = None
    UNKNOWN_AIRCRAFT_CRAWLING = False

    # Database configuration
    MONGODB_URI = 'mongodb://localhost:27017/'
    MONGODB_DB_NAME = 'flightradar'

    # gRPC configuration (used when RADAR_SERVICE_TYPE = 'grpc')
    GRPC_SERVER_ADDRESS = 'localhost:50051'

    # Nighthawk proxy URL for aircraft metadata lookups (disabled if not set)
    NIGHTHAWK_PROXY_URL = None

    # Crawler configuration
    CRAWLER_MAX_ATTEMPTS = 5  # Max retry attempts for "not found" errors
    CRAWLER_SERVICE_ERROR_RESET_HOURS = 6  # Reset service error attempts after N hours
    CRAWLER_STALENESS_DAYS = 120  # Re-process aircraft older than this (days)
    CRAWLER_INCOMPLETE_STALENESS_DAYS = 7  # Re-process incomplete aircraft after N days
    CRAWLER_BATCH_SIZE = 50  # Number of aircraft to process per cycle
    CRAWLER_RUN_INTERVAL_SEC = 20  # Seconds between crawler runs
    CRAWLER_CIRCUIT_BREAKER_THRESHOLD = 5  # Consecutive failures before circuit opens
    CRAWLER_CIRCUIT_BREAKER_RESET_SEC = 300  # Seconds before circuit breaker resets (5 min)

    # JWT Authentication configuration
    JWT_SECRET = None  # Required - must be set via environment variable
    JWT_ACCESS_TOKEN_EXPIRE_MINUTES = 15  # Token lifetime in minutes
    JWT_ALGORITHM = 'HS256'

    # Client secret for anonymous user authentication
    CLIENT_SECRET = None  # Required - must match frontend VITE_CLIENT_SECRET

    # Admin user password (set via ADMIN_PASSWORD environment variable)
    ADMIN_PASSWORD = None

    # Allowed frontend origins (comma-separated)
    # Used for CORS and cookie security
    ALLOWED_ORIGINS = None

    def __init__(self):

        self.config_src = ConfigSource.NONE

        self.from_env()

        if self.config_src == ConfigSource.NONE:
            raise ValueError('Configuration is neither read from env nor file')
        else:
            logger.info('Config loaded from source: {}'.format(self.config_src.name))

    def str2bool(self, v):
        return v.lower() in ("yes", "true", "t", "1")

    def sanitize_url(self, url):
        return url[:-1] if url[-1] == "/" else url

    def from_env(self):

        ENV_DATA_FOLDER = 'DATA_FOLDER'
        ENV_RADAR_SERVICE_URL = 'SERVICE_URL'
        ENV_RADAR_SERVICE_TYPE = 'SERVICE_TYPE'
        ENV_MIL_ONLY = 'MIL_ONLY'
        ENV_DB_RETENTION_MIN = 'DB_RETENTION_MIN'
        ENV_UNKNOWN_AIRCRAFT_CRAWLING = 'UNKNOWN_AIRCRAFT_CRAWLING'
        ENV_LOGGING_CONFIG = 'LOGGING_CONFIG'
        ENV_MONGODB_URI = 'MONGODB_URI'
        ENV_MONGODB_DB_NAME = 'MONGODB_DB_NAME'
        ENV_GRPC_SERVER_ADDRESS = 'GRPC_SERVER_ADDRESS'
        ENV_JWT_SECRET = 'JWT_SECRET'
        ENV_JWT_ACCESS_TOKEN_EXPIRE_MINUTES = 'JWT_ACCESS_TOKEN_EXPIRE_MINUTES'
        ENV_CLIENT_SECRET = 'CLIENT_SECRET'
        ENV_ADMIN_PASSWORD = 'ADMIN_PASSWORD'
        ENV_ALLOWED_ORIGINS = 'ALLOWED_ORIGINS'
        ENV_NIGHTHAWK_PROXY_URL = 'NIGHTHAWK_PROXY_URL'
        ENV_CRAWLER_MAX_ATTEMPTS = 'CRAWLER_MAX_ATTEMPTS'
        ENV_CRAWLER_SERVICE_ERROR_RESET_HOURS = 'CRAWLER_SERVICE_ERROR_RESET_HOURS'
        ENV_CRAWLER_STALENESS_DAYS = 'CRAWLER_STALENESS_DAYS'
        ENV_CRAWLER_INCOMPLETE_STALENESS_DAYS = 'CRAWLER_INCOMPLETE_STALENESS_DAYS'
        ENV_CRAWLER_BATCH_SIZE = 'CRAWLER_BATCH_SIZE'
        ENV_CRAWLER_RUN_INTERVAL_SEC = 'CRAWLER_RUN_INTERVAL_SEC'
        ENV_CRAWLER_CIRCUIT_BREAKER_THRESHOLD = 'CRAWLER_CIRCUIT_BREAKER_THRESHOLD'
        ENV_CRAWLER_CIRCUIT_BREAKER_RESET_SEC = 'CRAWLER_CIRCUIT_BREAKER_RESET_SEC'

        if os.environ.get(ENV_DATA_FOLDER):
            self.DATA_FOLDER = os.environ.get(ENV_DATA_FOLDER)
        if os.environ.get(ENV_RADAR_SERVICE_URL):
            self.RADAR_SERVICE_URL = self.sanitize_url(os.environ.get(ENV_RADAR_SERVICE_URL))
        if os.environ.get(ENV_RADAR_SERVICE_TYPE):
            self.RADAR_SERVICE_TYPE = os.environ.get(ENV_RADAR_SERVICE_TYPE)        
        if os.environ.get(ENV_MIL_ONLY):
            self.MILTARY_ONLY = self.str2bool(os.environ.get(ENV_MIL_ONLY))
        if os.environ.get(ENV_UNKNOWN_AIRCRAFT_CRAWLING):
            self.UNKNOWN_AIRCRAFT_CRAWLING = self.str2bool(os.environ.get(ENV_UNKNOWN_AIRCRAFT_CRAWLING))
        if os.environ.get(ENV_DB_RETENTION_MIN):
            try:
                self.DB_RETENTION_MIN = int(os.environ.get(ENV_DB_RETENTION_MIN))
            except ValueError:
                pass
        if os.environ.get(ENV_LOGGING_CONFIG):
            try:
                logging_json = json.loads(os.environ.get(ENV_LOGGING_CONFIG))
                self.LOGGING_CONFIG = LoggingConfig.from_json(logging_json)
            except ValueError as e:
                logging.getLogger().error(e)
        if os.environ.get(ENV_MONGODB_URI):
            self.MONGODB_URI = os.environ.get(ENV_MONGODB_URI)
        if os.environ.get(ENV_MONGODB_DB_NAME):
            self.MONGODB_DB_NAME = os.environ.get(ENV_MONGODB_DB_NAME)
        if os.environ.get(ENV_GRPC_SERVER_ADDRESS):
            self.GRPC_SERVER_ADDRESS = os.environ.get(ENV_GRPC_SERVER_ADDRESS)
        if os.environ.get(ENV_JWT_SECRET):
            self.JWT_SECRET = os.environ.get(ENV_JWT_SECRET)
        if os.environ.get(ENV_JWT_ACCESS_TOKEN_EXPIRE_MINUTES):
            try:
                self.JWT_ACCESS_TOKEN_EXPIRE_MINUTES = int(os.environ.get(ENV_JWT_ACCESS_TOKEN_EXPIRE_MINUTES))
            except ValueError:
                pass
        # Accept either CLIENT_SECRET or VITE_CLIENT_SECRET (for single-variable Docker setup)
        if os.environ.get(ENV_CLIENT_SECRET):
            self.CLIENT_SECRET = os.environ.get(ENV_CLIENT_SECRET)
        elif os.environ.get('VITE_CLIENT_SECRET'):
            self.CLIENT_SECRET = os.environ.get('VITE_CLIENT_SECRET')
        if os.environ.get(ENV_ADMIN_PASSWORD):
            self.ADMIN_PASSWORD = os.environ.get(ENV_ADMIN_PASSWORD)
        if os.environ.get(ENV_ALLOWED_ORIGINS):
            self.ALLOWED_ORIGINS = os.environ.get(ENV_ALLOWED_ORIGINS)
        if os.environ.get(ENV_NIGHTHAWK_PROXY_URL):
            self.NIGHTHAWK_PROXY_URL = self.sanitize_url(os.environ.get(ENV_NIGHTHAWK_PROXY_URL))

        # Crawler configuration
        if os.environ.get(ENV_CRAWLER_MAX_ATTEMPTS):
            try:
                self.CRAWLER_MAX_ATTEMPTS = int(os.environ.get(ENV_CRAWLER_MAX_ATTEMPTS))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_SERVICE_ERROR_RESET_HOURS):
            try:
                self.CRAWLER_SERVICE_ERROR_RESET_HOURS = int(os.environ.get(ENV_CRAWLER_SERVICE_ERROR_RESET_HOURS))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_STALENESS_DAYS):
            try:
                self.CRAWLER_STALENESS_DAYS = int(os.environ.get(ENV_CRAWLER_STALENESS_DAYS))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_INCOMPLETE_STALENESS_DAYS):
            try:
                self.CRAWLER_INCOMPLETE_STALENESS_DAYS = int(os.environ.get(ENV_CRAWLER_INCOMPLETE_STALENESS_DAYS))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_BATCH_SIZE):
            try:
                self.CRAWLER_BATCH_SIZE = int(os.environ.get(ENV_CRAWLER_BATCH_SIZE))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_RUN_INTERVAL_SEC):
            try:
                self.CRAWLER_RUN_INTERVAL_SEC = int(os.environ.get(ENV_CRAWLER_RUN_INTERVAL_SEC))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_CIRCUIT_BREAKER_THRESHOLD):
            try:
                self.CRAWLER_CIRCUIT_BREAKER_THRESHOLD = int(os.environ.get(ENV_CRAWLER_CIRCUIT_BREAKER_THRESHOLD))
            except ValueError:
                pass
        if os.environ.get(ENV_CRAWLER_CIRCUIT_BREAKER_RESET_SEC):
            try:
                self.CRAWLER_CIRCUIT_BREAKER_RESET_SEC = int(os.environ.get(ENV_CRAWLER_CIRCUIT_BREAKER_RESET_SEC))
            except ValueError:
                pass

        self.config_src = ConfigSource.ENV

    def __str__(self):
        return 'data folder: {0}, service url: {1}, type: {2}, mil only: {3}, delete after: {4}, crawling: {5}'.format(self.DATA_FOLDER, self.RADAR_SERVICE_URL, self.RADAR_SERVICE_TYPE, self.MILTARY_ONLY, self.DB_RETENTION_MIN, self.UNKNOWN_AIRCRAFT_CRAWLING) 

class DevConfig(Config):
    DEBUG = True

class ProductionConfig(Config):
    pass