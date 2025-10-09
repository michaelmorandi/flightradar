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
def get_flight(flight_id: str, mongodb: Database = Depends(get_mongodb)):
    try:
        flight = mongodb.flights.find_one({"_id": ObjectId(flight_id)})

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


@router.get('/flights/{flight_id}/positions/stream')
async def sse_flight_positions(request: Request, flight_id: str):
    """SSE endpoint for real-time position updates for a specific flight"""
    client_id = str(uuid.uuid4())
    app = request.app
    mongodb = app.state.mongodb
    
    # Check if flight exists
    try:
        flight = mongodb.flights.find_one({"_id": ObjectId(flight_id)})
        if not flight:
            raise HTTPException(status_code=404, detail=f'Flight {flight_id} not found')
    except Exception as e:
        logger.error(f"Error checking flight {flight_id}: {str(e)}")
        raise HTTPException(status_code=500, detail="Server error")
    
    async def event_stream() -> AsyncGenerator[str, None]:
        # Create SSE client
        queue = asyncio.Queue()
        client = SSEClient(
            id=client_id,
            request=request,
            queue=queue,
            type="flight",
            flight_id=flight_id
        )
        
        # Add client to manager
        sse_manager.add_client(client)
        
        registered_callback = None
        last_position = None
        
        try:
            # Get the flight ID in ObjectId format
            flight_oid = ObjectId(flight_id)
            
            # Fetch initial positions from mongodb for the given flight
            positions = list(mongodb.positions.find(
                {"flight_id": flight_oid},
                {"lat": 1, "lon": 1, "alt": 1, "gs": 1, "_id": 0}  # Only fetch needed fields
            ).sort("timestmp", 1).limit(10000))  # Limit to prevent memory issues
            
            # Format all positions for the initial message
            all_positions = []
            
            if positions:
                for pos in positions:
                    position_data = {
                        "lat": pos["lat"],
                        "lon": pos["lon"],
                        "alt": pos["alt"] if pos["alt"] is not None else -1
                    }
                    if pos.get("gs") is not None:
                        position_data["gs"] = pos["gs"]
                    all_positions.append(position_data)
                
                # Save the most recent position for comparison with future updates
                last_position = all_positions[-1] if all_positions else None
            
            # Send initial positions
            initial_message = {
                "type": "initial",
                "count": len(all_positions),
                "positions": {flight_id: all_positions} if all_positions else {}
            }
            
            yield f"event: flight_position\ndata: {json.dumps(initial_message)}\n\n"
            
            # Function to track position changes for a specific flight
            async def send_flight_position_updates(positions_dict):
                """Callback function to send position updates for a specific flight"""
                nonlocal last_position
                
                # Check if this update contains this flight
                if flight_id not in positions_dict:
                    return
                    
                # Get flight's new position
                new_position = positions_dict[flight_id]
                
                # Skip if there's no previous position yet
                if last_position is None:
                    last_position = new_position
                    await sse_manager.send_flight_position(flight_id, new_position)
                    return
                
                # Only send update if position has changed
                if (last_position["lat"] != new_position["lat"] or 
                    last_position["lon"] != new_position["lon"] or 
                    last_position["alt"] != new_position["alt"] or
                    last_position.get("gs") != new_position.get("gs")):
                    
                    # Update the last known position
                    last_position = new_position
                    
                    # Send position update via SSE manager
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
def get_positions(flight_id: str, mongodb: Database = Depends(get_mongodb)):
    try:
        flight = mongodb.flights.find_one({"_id": ObjectId(flight_id)})
        if not flight:
            raise HTTPException(status_code=404, detail="Flight not found")

        positions = list(mongodb.positions.find(
            {"flight_id": ObjectId(flight_id)},
            {"lat": 1, "lon": 1, "alt": 1, "_id": 0}  # Only fetch needed fields
        ).sort("timestmp", 1).limit(10000))  # Limit to prevent memory issues

        # Convert to array of arrays format
        result = []
        for p in positions:
            alt = p["alt"] if p["alt"] is not None else -1
            result.append([p["lat"], p["lon"], alt])

        return result

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
            
        # Convert flight data to position array format
        if hasattr(flight_data, 'lat') and hasattr(flight_data, 'lon'):
            alt = getattr(flight_data, 'alt', -1)
            if alt is None:
                alt = -1
                
            positions[icao24] = [[flight_data.lat, flight_data.lon, alt]]
    
    return positions
