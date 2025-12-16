from fastapi import Request, Query, HTTPException, Depends
from fastapi.responses import StreamingResponse
from typing import List, Dict, Optional, Any, AsyncGenerator
from pydantic import BaseModel
from pymongo.database import Database
from bson import ObjectId
import logging
import asyncio
import json
import uuid

from .. import router
from ..mappers import toFlightDto
from ..models import FlightDto, to_datestring
from ...sse.manager import sse_manager, SSEClient
from ...sse.notifier import SSENotifier
from ..dependencies import MetaInfoDep, get_mongodb
from ...scheduling import UPDATER_JOB_NAME

# Initialize logging
logger = logging.getLogger(__name__)

# Create SSE notifier
sse_notifier = SSENotifier()

# Constants
MAX_FLIGHTS_LIMIT = 300

# Define response models


class MetaInfo(BaseModel):
    class Config:
        arbitrary_types_allowed = True


class PositionReport(BaseModel):
    lat: float
    lon: float
    alt: int

    class Config:
        arbitrary_types_allowed = True


@router.get('/info', response_model=Dict[str, Any])
def get_meta_info(meta_info: MetaInfoDep):
    return {
        "commit_id": meta_info.commit_id,
        "build_timestamp": meta_info.build_timestamp
    }

@router.get('/alive')
def alive():
    return "Yes"

@router.get('/ready')
def ready(request: Request):
    updater_job = request.app.state.apscheduler.get_job(UPDATER_JOB_NAME)
    if updater_job and not updater_job.pending:
        return "Yes"
    else:
        raise HTTPException(status_code=500, detail="Service not ready")


@router.get('/flights', response_model=List[FlightDto], 
    summary="Get all flights",
    description="Returns a list of currently tracked flights. icao24 is the ICAO 24-bit hex address, cls is the callsign, lstCntct is the time of last contact, firstCntct is the time of first contact",
    responses={
        200: {
            "description": "List of flights",
            "content": {
                "application/json": {
                    "example": [
                        {
                            "id": "683f570bd570101935e7ff63",
                            "icao24": "394a03",
                            "cls": "AFR990",
                            "lstCntct": "2025-06-03T20:12:03.615000Z",
                            "firstCntct": "2025-06-03T20:11:55.542000Z"
                        }
                    ]
                }
            }
        }
    }
)
def get_flights(
    request: Request,
    filter: Optional[str] = Query(None, description="Filter flights (e.g. 'mil' for military only)"),
    limit: Optional[int] = Query(None, description="Maximum number of flights to return")
):
    try:
        # Get currently tracked flights from memory
        cached_flights = request.app.state.updater.get_cached_flights()
        
        flight_manager = request.app.state.updater._flight_manager
        modes_util = request.app.state.modes_util
        
        flight_dtos = []
        
        for flight_id, position_report in cached_flights.items():
            if filter == 'mil' and not modes_util.is_military(position_report.icao24):
                continue
                
            callsign = position_report.callsign
            last_contact = flight_manager.flight_last_contact.get(flight_id)
            
            if last_contact:
                flight_dto = FlightDto(
                    id=flight_id,
                    icao24=position_report.icao24,
                    cls=callsign,
                    lstCntct=to_datestring(last_contact),
                    firstCntct=to_datestring(last_contact)  # For live flights, use last contact as first contact approximation
                )
                flight_dtos.append(flight_dto)
        
        flight_dtos.sort(key=lambda x: x.lstCntct, reverse=True)
        
        # Apply limit (default and max limit is MAX_FLIGHTS_LIMIT)
        if limit is not None:
            applied_limit = min(limit, MAX_FLIGHTS_LIMIT)
        else:
            applied_limit = MAX_FLIGHTS_LIMIT
            
        return flight_dtos[:applied_limit]

    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Invalid arguments: {str(e)}")


@router.get('/flights/{flight_id}', response_model=FlightDto,
    summary="Get flight by ID",
    description="Returns a specific flight by its ID. icao24 is the ICAO 24-bit hex address, cls is the callsign, lstCntct is the time of last contact, firstCntct is the time of first contact",
    responses={
        200: {
            "description": "Flight details",
            "content": {
                "application/json": {
                    "example": {
                        "id": "683f570bd570101935e7ff63",
                        "icao24": "394a03",
                        "cls": "AFR990",
                        "lstCntct": "2025-06-03T20:14:19.571000Z",
                        "firstCntct": "2025-06-03T20:11:55.542000Z"
                    }
                }
            }
        },
        404: {"description": "Flight not found"}
    }
)
async def get_flight(flight_id: str, mongodb: Database = Depends(get_mongodb)):
    try:
        flight = await asyncio.to_thread(
            lambda: mongodb.flights.find_one({"_id": ObjectId(flight_id)})
        )

        if flight:
            return toFlightDto(flight)
        else:
            raise HTTPException(status_code=404, detail="Flight not found")

    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Invalid flight ID format: {str(e)}")


@router.get('/positions/live/stream')
async def sse_all_positions(request: Request):
    """SSE endpoint for real-time position updates"""
    client_id = str(uuid.uuid4())
    app = request.app
    
    async def event_stream() -> AsyncGenerator[str, None]:
        # Create SSE client
        queue = asyncio.Queue()
        client = SSEClient(
            id=client_id,
            request=request,
            queue=queue,
            type="positions"
        )
        
        # Add client to manager
        sse_manager.add_client(client)
        
        # Create a broadcast function for position updates
        async def broadcast_positions(positions_dict):
            """Function to broadcast positions to SSE clients"""
            logger.debug(f"SSE callback triggered with {len(positions_dict)} positions")
            await sse_manager.broadcast_positions(positions_dict)
        
        # Register the callback with the flight updater
        registered_callback = app.state.updater.register_sse_callback(broadcast_positions)
        
        try:
            # Send initial positions immediately after connection
            cached_flights = app.state.updater.get_cached_flights()
            initial_positions = {str(k): v.__dict__ for k, v in cached_flights.items()}

            # Send initial data
            initial_message = {
                "type": "initial",
                "count": len(initial_positions),
                "positions": initial_positions
            }
            
            yield f"event: positions\ndata: {json.dumps(initial_message)}\n\n"
            
            # Process messages from queue
            while True:
                try:
                    # Wait for messages with timeout to handle disconnections
                    message = await asyncio.wait_for(queue.get(), timeout=30.0)
                    event_type = message.get("event", "message")
                    data = message.get("data", {})
                    
                    yield f"event: {event_type}\ndata: {json.dumps(data)}\n\n"
                except asyncio.TimeoutError:
                    # Send heartbeat to keep connection alive
                    timestamp = asyncio.get_event_loop().time()
                    yield f"event: heartbeat\ndata: {{\"timestamp\": {timestamp}}}\n\n"
                except Exception as e:
                    logger.error(f"Error in SSE stream for client {client_id}: {str(e)}")
                    break
        
        except Exception as e:
            logger.error(f"Error in SSE positions stream: {str(e)}")
        finally:
            # Clean up
            sse_manager.remove_client(client_id)
            try:
                app.state.updater.unregister_sse_callback(registered_callback)
                logger.info("SSE broadcast callback unregistered")
            except Exception as e:
                logger.error(f"Error unregistering SSE callback: {str(e)}")
    
    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Headers": "Cache-Control"
        }
    )


def _format_position(position_data, include_gs: bool = True) -> dict:
    """Helper to format a position dict with consistent structure"""
    formatted = {
        "lat": position_data.get("lat") if isinstance(position_data, dict) else position_data.lat,
        "lon": position_data.get("lon") if isinstance(position_data, dict) else position_data.lon,
        "alt": position_data.get("alt", -1) if isinstance(position_data, dict) else (position_data.alt if position_data.alt is not None else -1)
    }
    if include_gs:
        gs = position_data.get("gs") if isinstance(position_data, dict) else getattr(position_data, "gs", None)
        if gs is not None:
            formatted["gs"] = gs

        track = position_data.get("track") if isinstance(position_data, dict) else getattr(position_data, "track", None)
        if track is not None:
            formatted["track"] = track

    category = position_data.get("category") if isinstance(position_data, dict) else getattr(position_data, "category", None)
    if category is not None:
        formatted["category"] = category

    return formatted


def _positions_equal(pos1: dict, pos2: dict) -> bool:
    """Check if two positions are equal"""
    return (pos1["lat"] == pos2["lat"] and
            pos1["lon"] == pos2["lon"] and
            pos1["alt"] == pos2["alt"] and
            pos1.get("gs") == pos2.get("gs"))


async def _fetch_flight_positions(mongodb, flight_id: str) -> list:
    """Fetch positions for a flight from MongoDB in a thread pool"""
    flight_oid = ObjectId(flight_id)

    def fetch():
        return list(mongodb.positions.find(
            {"flight_id": flight_oid},
            {"lat": 1, "lon": 1, "alt": 1, "gs": 1, "_id": 0}
        ).sort("timestmp", 1).limit(10000))

    return await asyncio.to_thread(fetch)


@router.get('/flights/{flight_id}/positions/stream')
async def sse_flight_positions(request: Request, flight_id: str):
    """SSE endpoint for real-time position updates for a specific flight"""
    app = request.app
    mongodb = app.state.mongodb

    # Verify flight exists (run in thread pool to avoid blocking)
    try:
        flight = await asyncio.to_thread(
            lambda: mongodb.flights.find_one({"_id": ObjectId(flight_id)})
        )
        if not flight:
            raise HTTPException(status_code=404, detail=f'Flight {flight_id} not found')
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error checking flight {flight_id}: {str(e)}")
        raise HTTPException(status_code=500, detail="Server error")
    
    async def event_stream() -> AsyncGenerator[str, None]:
        client_id = str(uuid.uuid4())
        queue = asyncio.Queue()
        client = SSEClient(
            id=client_id,
            request=request,
            queue=queue,
            type="flight",
            flight_id=flight_id
        )

        sse_manager.add_client(client)
        registered_callback = None
        last_position = None

        try:
            # Fetch historical positions from database
            positions = await _fetch_flight_positions(mongodb, flight_id)
            all_positions = [_format_position(pos) for pos in positions]

            # Add current in-memory position if available and different from last DB position
            cached_flights = app.state.updater.get_cached_flights()
            current_position = cached_flights.get(flight_id)

            if current_position:
                current_pos_data = _format_position(current_position)

                if not all_positions or not _positions_equal(all_positions[-1], current_pos_data):
                    all_positions.append(current_pos_data)

                last_position = current_pos_data
            else:
                last_position = all_positions[-1] if all_positions else None

            # Send initial message with all positions
            yield f"event: flight_position\ndata: {json.dumps({
                'type': 'initial',
                'count': len(all_positions),
                'positions': {flight_id: all_positions} if all_positions else {}
            })}\n\n"
            
            # Callback for live position updates
            async def send_flight_position_updates(positions_dict):
                """Send position updates when flight position changes"""
                nonlocal last_position

                if flight_id not in positions_dict:
                    return

                new_position = positions_dict[flight_id]

                if last_position is None:
                    last_position = new_position
                    await sse_manager.send_flight_position(flight_id, new_position)
                    return

                if not _positions_equal(last_position, new_position):
                    last_position = new_position
                    await sse_manager.send_flight_position(flight_id, new_position)
            
            # Register the callback with the flight updater
            registered_callback = app.state.updater.register_sse_callback(send_flight_position_updates)

            logger.info(f"SSE callback registered for flight {flight_id}")

            # Process messages from queue
            while True:
                try:
                    # Wait for messages with timeout to handle disconnections
                    message = await asyncio.wait_for(queue.get(), timeout=30.0)
                    event_type = message.get("event", "message")
                    data = message.get("data", {})

                    yield f"event: {event_type}\ndata: {json.dumps(data)}\n\n"
                except asyncio.TimeoutError:
                    # Send heartbeat to keep connection alive
                    timestamp = asyncio.get_event_loop().time()
                    yield f"event: heartbeat\ndata: {{\"timestamp\": {timestamp}}}\n\n"
                except Exception as e:
                    logger.error(f"Error in SSE flight stream for client {client_id}: {str(e)}")
                    break
        
        except Exception as e:
            logger.error(f"Error in SSE flight positions stream: {str(e)}")
        finally:
            # Clean up
            sse_manager.remove_client(client_id)
            if registered_callback:
                try:
                    app.state.updater.unregister_sse_callback(registered_callback)
                    logger.info(f"SSE callback for flight {flight_id} unregistered")
                except Exception as e:
                    logger.error(f"Error unregistering SSE flight callback: {str(e)}")
    
    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Headers": "Cache-Control"
        }
    )

@router.get('/flights/{flight_id}/positions',
    summary="Get flight positions",
    description="Returns an array of position coordinates [lat, lon, alt] for a specific flight",
    responses={
        200: {
            "description": "Array of position coordinates",
            "content": {
                "application/json": {
                    "example": [
                        [47.520152, 7.920509, 32025],
                        [47.655716, 11.048882, 28475]
                    ]
                }
            }
        },
        404: {"description": "Flight not found"}
    }
)
async def get_positions(flight_id: str, mongodb: Database = Depends(get_mongodb)):
    try:
        flight = await asyncio.to_thread(
            lambda: mongodb.flights.find_one({"_id": ObjectId(flight_id)})
        )
        if not flight:
            raise HTTPException(status_code=404, detail="Flight not found")

        positions = await _fetch_flight_positions(mongodb, flight_id)

        # Convert to array of arrays format
        return [[p["lat"], p["lon"], p.get("alt", -1) if p.get("alt") is not None else -1]
                for p in positions]

    except Exception as e:
        raise HTTPException(status_code=400, detail=f"Invalid flight id format: {str(e)}")


@router.get('/positions',
    summary="Get all positions",
    description="Returns a map with ICAO24 hex address as key and arrays of [lat, lon, alt] coordinates as values",
    responses={
        200: {
            "description": "Map of ICAO24 addresses to position arrays",
            "content": {
                "application/json": {
                    "example": {
                        "300781": [
                            [47.669632, 11.054512, 28200],
                            [47.655716, 11.048882, 28475]
                        ]
                    }
                }
            }
        }
    }
)
def get_all_positions(
    request: Request,
    filter: Optional[str] = Query(None, description="Filter positions (e.g. 'mil' for military only)")
):
    cached_flights = request.app.state.updater.get_cached_flights()
    positions = {}

    for icao24, flight_data in cached_flights.items():
        if filter == 'mil' and not request.app.state.modes_util.is_military(icao24):
            continue

        if hasattr(flight_data, 'lat') and hasattr(flight_data, 'lon'):
            pos = _format_position(flight_data, include_gs=False)
            positions[icao24] = [[pos["lat"], pos["lon"], pos["alt"]]]

    return positions
