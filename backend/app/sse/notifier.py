import logging
from typing import Any, Dict, Set, Callable, Optional
import asyncio

logger = logging.getLogger('SSENotifier')


class SSENotifier:
    _main_loop: Optional[asyncio.AbstractEventLoop] = None

    def __init__(self):
        self._callbacks: Set[Callable] = set()
        # Try to capture the main event loop on initialization
        try:
            if SSENotifier._main_loop is None:
                SSENotifier._main_loop = asyncio.get_event_loop()
                logger.info("Captured main event loop for SSE notifications")
        except RuntimeError:
            logger.warning("No event loop available during SSENotifier initialization")
        
    def register_callback(self, callback: Callable[[Dict[str, Any]], None]):
        """Register a callback function to notify when positions are updated"""
        self._callbacks.add(callback)
        return callback
        
    def unregister_callback(self, callback: Callable[[Dict[str, Any]], None]):
        """Unregister a previously registered callback"""
        if callback in self._callbacks:
            self._callbacks.remove(callback)
            return True
        return False
        
    def has_callbacks(self):
        """Check if there are any registered callbacks"""
        return len(self._callbacks) > 0
        
    def notify_clients(self, positions_dict):
        """Notify all registered clients with position updates"""
        if not positions_dict or not self._callbacks:
            return

        logger.debug(f"Notifying {len(self._callbacks)} callbacks with {len(positions_dict)} positions")

        callbacks_to_remove = set()
        for callback in self._callbacks:
            try:
                # Check if callback is async
                if asyncio.iscoroutinefunction(callback):
                    # Try to get the running loop first
                    loop = None
                    try:
                        loop = asyncio.get_running_loop()
                        logger.debug("Using currently running event loop")
                    except RuntimeError:
                        # No running loop in current thread - use the stored main loop
                        if SSENotifier._main_loop is not None:
                            loop = SSENotifier._main_loop
                            logger.debug("Using stored main event loop")
                        else:
                            logger.error("No event loop available for async callback")
                            continue

                    # Schedule the coroutine in the event loop
                    if loop is not None:
                        try:
                            future = asyncio.run_coroutine_threadsafe(callback(positions_dict), loop)
                            logger.debug("Scheduled async callback successfully")
                        except Exception as e:
                            logger.error(f"Failed to schedule async callback: {str(e)}")
                else:
                    callback(positions_dict)
            except Exception as e:
                logger.error(f"Error in SSE callback: {str(e)}", exc_info=True)
                callbacks_to_remove.add(callback)

        if callbacks_to_remove:
            self._callbacks.difference_update(callbacks_to_remove)
            
    def notify_position_changes(self, all_cached_flights: Dict[str, Any], changed_flight_ids: Set[str]):
        """
        Notify clients of position changes with proper data transformation

        Args:
            all_cached_flights: Dictionary of flight_id -> PositionReport
            changed_flight_ids: Set of flight IDs that have changed
        """
        from ..core.models.position_report import PositionReport

        if not self.has_callbacks() or not changed_flight_ids:
            return

        positions_dict = {}
        for flight_id, pos in all_cached_flights.items():
            if str(flight_id) in changed_flight_ids:
                position_data = {
                    "icao": pos.icao24,
                    "lat": pos.lat,
                    "lon": pos.lon,
                    "alt": pos.alt,
                    "track": pos.track
                }
                if pos.gs is not None:
                    position_data["gs"] = pos.gs
                positions_dict[str(flight_id)] = position_data

        # Fallback if no positions matched (should be rare)
        if not positions_dict and all_cached_flights:
            logger.warning("No changed positions match cached flights")

            count = 0
            for flight_id, pos in all_cached_flights.items():
                if count < 50:  # Limit to 50 positions
                    position_data = {
                        "icao": pos.icao24,
                        "lat": pos.lat,
                        "lon": pos.lon,
                        "alt": pos.alt,
                        "track": pos.track
                    }
                    if pos.gs is not None:
                        position_data["gs"] = pos.gs
                    positions_dict[str(flight_id)] = position_data
                    count += 1
                else:
                    break

        if positions_dict:
            self.notify_clients(positions_dict)

    def notify_category_changes(self, category_changes: Dict[str, int]):
        """
        Notify clients of aircraft category changes

        Args:
            category_changes: Dictionary of flight_id -> category_number
        """
        from ..sse.manager import sse_manager

        if not self.has_callbacks() or not category_changes:
            return

        logger.debug(f"Notifying category changes for {len(category_changes)} flights")

        # Use the SSE manager to broadcast via the categories event
        for callback in self._callbacks:
            try:
                if asyncio.iscoroutinefunction(callback):
                    # For async callbacks, we need to schedule the broadcast
                    loop = None
                    try:
                        loop = asyncio.get_running_loop()
                    except RuntimeError:
                        if SSENotifier._main_loop is not None:
                            loop = SSENotifier._main_loop

                    if loop is not None:
                        try:
                            # Call the sse_manager.broadcast_categories directly
                            asyncio.run_coroutine_threadsafe(
                                sse_manager.broadcast_categories(category_changes),
                                loop
                            )
                        except Exception as e:
                            logger.error(f"Failed to schedule category broadcast: {str(e)}")
            except Exception as e:
                logger.error(f"Error in category change notification: {str(e)}", exc_info=True)

    def notify_callsign_changes(self, callsign_changes: Dict[str, str]):
        """
        Notify clients of aircraft callsign changes

        Args:
            callsign_changes: Dictionary of flight_id -> callsign
        """
        from ..sse.manager import sse_manager

        if not self.has_callbacks() or not callsign_changes:
            return

        logger.debug(f"Notifying callsign changes for {len(callsign_changes)} flights")

        # Use the SSE manager to broadcast via the callsigns event
        for callback in self._callbacks:
            try:
                if asyncio.iscoroutinefunction(callback):
                    # For async callbacks, we need to schedule the broadcast
                    loop = None
                    try:
                        loop = asyncio.get_running_loop()
                    except RuntimeError:
                        if SSENotifier._main_loop is not None:
                            loop = SSENotifier._main_loop

                    if loop is not None:
                        try:
                            # Call the sse_manager.broadcast_callsigns directly
                            asyncio.run_coroutine_threadsafe(
                                sse_manager.broadcast_callsigns(callsign_changes),
                                loop
                            )
                        except Exception as e:
                            logger.error(f"Failed to schedule callsign broadcast: {str(e)}")
            except Exception as e:
                logger.error(f"Error in callsign change notification: {str(e)}", exc_info=True)