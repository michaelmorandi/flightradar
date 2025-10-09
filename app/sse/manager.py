import logging
import threading
from typing import Dict, Any, Optional
from fastapi import Request
from asyncio import Queue
from dataclasses import dataclass

logger = logging.getLogger("SSEManager")


@dataclass
class SSEClient:
    """Represents an SSE client connection"""
    id: str
    request: Request
    queue: Queue
    type: str  # 'positions' or 'flight'
    flight_id: Optional[str] = None
    last_activity: float = 0.0


class SSEConnectionManager:
    """
    Manages SSE connections for real-time position updates
    """

    def __init__(self):
        # Store active connections
        self.active_connections: Dict[str, SSEClient] = {}
        # Lock for thread safety when modifying connections
        self._lock = threading.Lock()

    def add_client(self, client: SSEClient):
        """
        Add a new SSE client connection
        """
        with self._lock:
            self.active_connections[client.id] = client

        client_info = f"New SSE connection established for {client.type}"
        if client.flight_id:
            client_info += f" (flight: {client.flight_id})"
        if hasattr(client.request, 'headers') and "x-forwarded-for" in client.request.headers:
            client_info += f" from {client.request.headers['x-forwarded-for']}"
        logger.debug(f"{client_info}. Total active: {len(self.active_connections)}")

    def remove_client(self, client_id: str):
        """
        Remove an SSE client connection
        """
        with self._lock:
            if client_id in self.active_connections:
                del self.active_connections[client_id]
        logger.debug(f"SSE connection closed. Total active: {len(self.active_connections)}")

    def get_client(self, client_id: str) -> Optional[SSEClient]:
        """Get a client by ID"""
        with self._lock:
            return self.active_connections.get(client_id)

    async def send_to_client(self, client_id: str, data: Dict[str, Any], event_type: str = "message"):
        """
        Send data to a specific client
        """
        client = self.get_client(client_id)
        if client:
            try:
                message = {
                    "event": event_type,
                    "data": data
                }
                await client.queue.put(message)
            except Exception as e:
                logger.error(f"Error sending to SSE client {client_id}: {str(e)}")
                self.remove_client(client_id)

    async def broadcast_positions(self, positions: Dict[str, Any]):
        """
        Broadcast position data to all connected position clients
        """
        # Make a thread-safe copy of the connections
        with self._lock:
            if not self.active_connections:
                logger.debug("No active connections, skipping broadcast")
                return
            # Get only position clients
            position_clients = {
                client_id: client 
                for client_id, client in self.active_connections.items() 
                if client.type == "positions"
            }

        # Validate positions is not empty
        if not positions:
            logger.warning("Attempted to broadcast empty positions, skipping")
            return

        # Create a message with metadata about the update
        message = {
            "type": "update",
            "count": len(positions),
            "positions": positions
        }

        logger.debug(f"Broadcasting {len(positions)} position updates to {len(position_clients)} connected clients")

        disconnected_clients = []

        # Broadcast to all connected position clients
        for client_id, client in position_clients.items():
            try:
                await client.queue.put({
                    "event": "positions",
                    "data": message
                })
            except Exception as e:
                logger.error(f"Error sending to SSE client {client_id}: {str(e)}")
                # If connection is closed or had an error, mark for removal
                disconnected_clients.append(client_id)

        # Remove disconnected connections
        if disconnected_clients:
            with self._lock:
                for client_id in disconnected_clients:
                    if client_id in self.active_connections:
                        del self.active_connections[client_id]
                logger.debug(
                    f"Removed {len(disconnected_clients)} disconnected clients. {len(self.active_connections)} remaining."
                )

    async def send_flight_position(self, flight_id: str, position_data: Dict[str, Any]):
        """
        Send position data to clients subscribed to a specific flight
        """
        with self._lock:
            # Get clients subscribed to this flight
            flight_clients = {
                client_id: client 
                for client_id, client in self.active_connections.items() 
                if client.type == "flight" and client.flight_id == flight_id
            }

        if not flight_clients:
            return

        message = {
            "type": "update",
            "flight_id": flight_id,
            **position_data
        }

        disconnected_clients = []

        for client_id, client in flight_clients.items():
            try:
                await client.queue.put({
                    "event": "flight_position",
                    "data": message
                })
            except Exception as e:
                logger.error(f"Error sending flight position to SSE client {client_id}: {str(e)}")
                disconnected_clients.append(client_id)

        # Remove disconnected connections
        if disconnected_clients:
            with self._lock:
                for client_id in disconnected_clients:
                    if client_id in self.active_connections:
                        del self.active_connections[client_id]
                logger.debug(f"Removed {len(disconnected_clients)} disconnected flight clients")


# Global SSE connection manager
sse_manager = SSEConnectionManager()