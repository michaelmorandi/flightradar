import logging
import grpc
from typing import List, Optional, Iterator
from ..base import RadarService
from ....core.models.position_report import PositionReport
from . import adsb_pb2
from . import adsb_pb2_grpc

logger = logging.getLogger('GrpcAdsb')


class GrpcAdsb(RadarService):
    """gRPC ADS-B client for real-time plane tracking"""

    def __init__(self, url: str):
        """
        Initialize gRPC ADS-B client.

        Args:
            url: gRPC server address in format "host:port" (e.g., "localhost:50051")
        """
        super().__init__(url)
        self.grpc_address = f"{self._url_parms.hostname}:{self._url_parms.port or 50051}"
        self.channel: Optional[grpc.Channel] = None
        self.stub: Optional[adsb_pb2_grpc.PlaneTrackingServiceStub] = None
        self._connect()

    def _connect(self):
        """Establish gRPC connection"""
        try:
            self.channel = grpc.insecure_channel(self.grpc_address)
            self.stub = adsb_pb2_grpc.PlaneTrackingServiceStub(self.channel)
            logger.info(f"Connected to gRPC ADS-B service at {self.grpc_address}")
            self.connection_alive = True
        except Exception as e:
            logger.error(f"Failed to connect to gRPC service: {e}")
            self.connection_alive = False

    def _ensure_connected(self):
        """Ensure connection is alive, reconnect if needed"""
        if not self.channel or not self.connection_alive:
            self._connect()

    def close(self):
        """Close gRPC channel"""
        if self.channel:
            self.channel.close()
            logger.info("gRPC channel closed")

    def get_status(self) -> Optional[dict]:
        """
        Get service status information.

        Returns:
            Dictionary with status information or None on error
        """
        self._ensure_connected()
        try:
            request = adsb_pb2.GetStatusRequest()
            response = self.stub.GetStatus(request, timeout=5.0)
            return {
                'plane_count': response.plane_count,
                'source_count': response.source_count,
                'connected_sources': list(response.connected_sources),
                'total_messages': response.total_messages,
                'uptime_seconds': response.uptime_seconds
            }
        except grpc.RpcError as e:
            logger.error(f"Failed to get status: {e}")
            self.connection_alive = False
            return None

    def _plane_state_to_position_report(self, plane: adsb_pb2.PlaneState) -> PositionReport:
        """
        Convert gRPC PlaneState to PositionReport.

        Args:
            plane: PlaneState protobuf message

        Returns:
            PositionReport object
        """
        lat = plane.position.latitude if plane.HasField('position') else None
        lon = plane.position.longitude if plane.HasField('position') else None
        alt = plane.altitude_feet if plane.HasField('altitude_feet') else None

        gs = None
        track = None
        if plane.HasField('velocity'):
            gs = plane.velocity.ground_speed_knots
            track = plane.velocity.heading_degrees

        callsign = plane.callsign if plane.callsign else None

        # Convert AircraftCategory enum to string name
        category = adsb_pb2.AircraftCategory.Name(plane.category) if plane.category else None

        return PositionReport(
            icao24=plane.icao_address,
            lat=lat,
            lon=lon,
            alt=alt,
            gs=gs,
            track=track,
            callsign=callsign,
            category=category
        )

    def query_live_flights(self, filter_incomplete=True) -> Optional[List[PositionReport]]:
        """
        Get all currently tracked planes.

        Args:
            filter_incomplete: If True, filter out planes without position data

        Returns:
            List of PositionReport objects or None on error
        """
        self._ensure_connected()
        try:
            request = adsb_pb2.GetAllPlanesRequest()
            response = self.stub.GetAllPlanes(request, timeout=10.0)

            flights = []
            for plane in response.planes:
                has_position = plane.HasField('position')
                has_altitude = plane.HasField('altitude_feet')
                has_callsign = bool(plane.callsign)

                if filter_incomplete:
                    if (has_position and has_altitude) or has_callsign:
                        flights.append(self._plane_state_to_position_report(plane))
                else:
                    flights.append(self._plane_state_to_position_report(plane))

            self.connection_alive = True
            logger.debug(f"Retrieved {len(flights)} flights from gRPC service")
            return flights

        except grpc.RpcError as e:
            logger.error(f"Failed to get planes: {e}")
            self.connection_alive = False
            return None

    def query_live_icao24(self) -> Optional[List[str]]:
        """
        Get list of all ICAO24 addresses currently tracked.

        Returns:
            List of ICAO24 hex strings or None on error
        """
        flights = self.query_live_flights(filter_incomplete=False)
        if flights:
            return [flight.icao24 for flight in flights]
        return None

    def stream_updates(self,
                      include_initial_snapshot: bool = True,
                      update_interval_ms: Optional[int] = 500) -> Optional[Iterator[dict]]:
        """
        Stream real-time plane updates.

        Args:
            include_initial_snapshot: If True, receive all current planes before streaming updates
            update_interval_ms: Minimum interval between updates per aircraft (0 for no throttling)

        Yields:
            Dictionary with update information:
            - update_type: 'ADD', 'UPDATE', or 'REMOVE'
            - plane: PositionReport object (for ADD/UPDATE)
            - icao_address: ICAO24 address (for REMOVE)
        """
        self._ensure_connected()
        try:
            request = adsb_pb2.StreamUpdatesRequest(
                include_initial_snapshot=include_initial_snapshot
            )
            if update_interval_ms is not None:
                request.update_interval_ms = update_interval_ms

            logger.info(f"Starting plane update stream (initial_snapshot={include_initial_snapshot})")

            for update in self.stub.StreamUpdates(request):
                update_dict = {'update_type': adsb_pb2.UpdateType.Name(update.update_type)}

                if update.update_type == adsb_pb2.UPDATE_TYPE_REMOVE:
                    if update.HasField('removed_icao'):
                        update_dict['icao_address'] = update.removed_icao
                        yield update_dict
                else:
                    if update.HasField('plane'):
                        update_dict['plane'] = self._plane_state_to_position_report(update.plane)
                        yield update_dict

                self.connection_alive = True

        except grpc.RpcError as e:
            logger.error(f"Stream error: {e}")
            self.connection_alive = False
            return None

    def get_silhouete_params(self):
        """Not applicable for gRPC service"""
        return None
