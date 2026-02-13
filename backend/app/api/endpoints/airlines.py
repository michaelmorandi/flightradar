from fastapi import Query
from typing import Optional
import asyncio

from .. import router
from ..models import (
    AirlineDto, AirlineWithStatsDto, AirlineDetailDto,
    AirlinesResponse, to_datestring
)
from ..dependencies import MongoDBRepositoryDep, CurrentUserDep, AirlineServiceDep


@router.get('/airlines', response_model=AirlinesResponse,
    summary="Get airlines with flight statistics",
    description="Returns all airlines observed in flight data with flight and aircraft counts. "
                "Optionally filter by search query (matches airline name or ICAO code)."
)
async def get_airlines(
    repository: MongoDBRepositoryDep,
    airline_service: AirlineServiceDep,
    current_user: CurrentUserDep,
    q: Optional[str] = Query(None, description="Search query (airline name or ICAO code)")
):
    # Get airline stats from DB
    db_airlines = await asyncio.to_thread(repository.get_airlines_with_counts)

    results = []
    for entry in db_airlines:
        icao_code = entry["_id"]
        airline_info = airline_service.get(icao_code)

        dto = AirlineWithStatsDto(
            icaoCode=icao_code,
            name=airline_info.name if airline_info else icao_code,
            country=airline_info.country if airline_info else None,
            callsign=airline_info.callsign if airline_info else None,
            flightCount=entry["flight_count"],
            aircraftCount=entry.get("aircraft_count", 0),
            lastSeen=to_datestring(entry["last_seen"]) if entry.get("last_seen") else None
        )
        results.append(dto)

    # Apply search filter if provided
    if q:
        q_lower = q.strip().lower()
        results = [
            a for a in results
            if q_lower in a.name.lower() or q_lower in a.icaoCode.lower()
        ]

    return AirlinesResponse(
        airlines=results,
        total=len(results)
    )


@router.get('/airlines/search', response_model=list[AirlineDto],
    summary="Search airline database",
    description="Search the airline reference database by name or ICAO code. "
                "Does not require flights to exist in the system."
)
async def search_airlines(
    airline_service: AirlineServiceDep,
    current_user: CurrentUserDep,
    q: str = Query(..., description="Search query", min_length=1),
    limit: int = Query(20, description="Max results", ge=1, le=100)
):
    results = airline_service.search(q, limit=limit)
    return [
        AirlineDto(
            icaoCode=a.icao_code,
            name=a.name,
            country=a.country,
            callsign=a.callsign
        )
        for a in results
    ]


@router.get('/airlines/{icao_code}', response_model=AirlineDetailDto,
    summary="Get airline details",
    description="Returns detailed information about a specific airline including fleet and stats."
)
async def get_airline_detail(
    icao_code: str,
    repository: MongoDBRepositoryDep,
    airline_service: AirlineServiceDep,
    current_user: CurrentUserDep
):
    from fastapi import HTTPException

    airline_info = airline_service.get(icao_code)
    db_detail = await asyncio.to_thread(repository.get_airline_detail, icao_code)

    if not db_detail and not airline_info:
        raise HTTPException(status_code=404, detail="Airline not found")

    return AirlineDetailDto(
        icaoCode=icao_code.upper(),
        name=airline_info.name if airline_info else icao_code.upper(),
        country=airline_info.country if airline_info else None,
        callsign=airline_info.callsign if airline_info else None,
        flightCount=db_detail["flight_count"] if db_detail else 0,
        aircraftCount=db_detail["aircraft_count"] if db_detail else 0,
        firstSeen=to_datestring(db_detail["first_seen"]) if db_detail and db_detail.get("first_seen") else None,
        lastSeen=to_datestring(db_detail["last_seen"]) if db_detail and db_detail.get("last_seen") else None,
        aircraft=db_detail.get("aircraft", []) if db_detail else []
    )
