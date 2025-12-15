import logging
from .radar_services.virtualradarserver import VirtualRadarServer
from .radar_services.dump1090 import Dump1090
from .radar_services.grpc_adsb import GrpcAdsb

logger = logging.getLogger('RadarServiceFactory')

class RadarServiceFactory:
    @staticmethod
    def create(config):
        """Create appropriate radar service based on configuration"""
        if config.RADAR_SERVICE_TYPE == 'vrs':
            logger.info(f"Connecting to VirtualRadarServer at {config.RADAR_SERVICE_URL}")
            return VirtualRadarServer(config.RADAR_SERVICE_URL)
        elif config.RADAR_SERVICE_TYPE == 'dmp1090':
            logger.info(f"Connecting to dump1090 at {config.RADAR_SERVICE_URL}")
            return Dump1090(config.RADAR_SERVICE_URL)
        elif config.RADAR_SERVICE_TYPE == 'grpc':
            logger.info(f"Connecting to gRPC ADS-B service at {config.GRPC_SERVER_ADDRESS}")
            url = f"grpc://{config.GRPC_SERVER_ADDRESS}"
            return GrpcAdsb(url)
        else:
            raise ValueError('Service type not specified in config')