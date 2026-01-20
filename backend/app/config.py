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

    # JWT Authentication configuration
    JWT_SECRET = None  # Required - must be set via environment variable
    JWT_ACCESS_TOKEN_EXPIRE_MINUTES = 15  # Token lifetime in minutes
    JWT_ALGORITHM = 'HS256'

    # Client secret for anonymous user authentication
    CLIENT_SECRET = None  # Required - must match frontend VITE_CLIENT_SECRET

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
        ENV_ALLOWED_ORIGINS = 'ALLOWED_ORIGINS'

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
        if os.environ.get(ENV_ALLOWED_ORIGINS):
            self.ALLOWED_ORIGINS = os.environ.get(ENV_ALLOWED_ORIGINS)
        self.config_src = ConfigSource.ENV

    def __str__(self):
        return 'data folder: {0}, service url: {1}, type: {2}, mil only: {3}, delete after: {4}, crawling: {5}'.format(self.DATA_FOLDER, self.RADAR_SERVICE_URL, self.RADAR_SERVICE_TYPE, self.MILTARY_ONLY, self.DB_RETENTION_MIN, self.UNKNOWN_AIRCRAFT_CRAWLING) 

class DevConfig(Config):
    DEBUG = True

class ProductionConfig(Config):
    pass