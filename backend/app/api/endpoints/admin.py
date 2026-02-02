"""
Admin API endpoints.

These endpoints require admin role authentication.
"""

from datetime import datetime
from typing import Optional

from fastapi import Depends, HTTPException, status, Response
from fastapi.security import OAuth2PasswordRequestForm
from pydantic import BaseModel
from pymongo.database import Database
from pymongo import ReturnDocument

from fastapi import Request

from .. import router
from ..dependencies import AdminUserDep, CurrentUserDep, get_mongodb, get_config
from ...auth.models import User
from ...auth.anonymous import ADMIN_EMAIL, password_helper
from ...auth.config import get_jwt_strategy, cookie_transport
from ...config import Config
from ...data.repositories.aircraft_processing_repository import AircraftProcessingRepository


class DashboardStats(BaseModel):
    """Dashboard statistics response model."""
    flight_count: int


class CircuitBreakerStats(BaseModel):
    """Circuit breaker status for a single source."""
    state: str
    consecutive_failures: int
    total_failures: int
    total_successes: int
    trip_count: int
    current_backoff_seconds: int
    seconds_until_retry: float


class SourceStatusInline(BaseModel):
    """Status of a single crawler source (inline in stats)."""
    name: str
    enabled: bool


class CrawlerStats(BaseModel):
    """Crawler processing queue statistics."""
    enabled: bool
    queue_total: int
    queue_pending: int
    queue_in_progress: int
    not_found_failures: int
    service_error_failures: int
    max_attempts_reached: int
    circuit_breakers: dict[str, CircuitBreakerStats]
    sources: list[SourceStatusInline] = []


class CrawlerActivityItem(BaseModel):
    """A single crawler activity entry."""
    icao24: str
    timestamp: str
    status: str  # 'success', 'partial', 'not_found', 'service_error'
    source: Optional[str] = None
    registration: Optional[str] = None
    aircraft_type: Optional[str] = None


class CrawlerActivityResponse(BaseModel):
    """Response containing recent crawler activity."""
    activity: list[CrawlerActivityItem]


class SourceStatus(BaseModel):
    """Status of a single crawler source."""
    name: str
    enabled: bool


class SourcesResponse(BaseModel):
    """Response containing all crawler sources."""
    sources: list[SourceStatus]


class SourceToggleRequest(BaseModel):
    """Request to toggle a source's enabled state."""
    enabled: bool


class UserInfo(BaseModel):
    """Current user info response model."""
    email: str
    role: str
    is_admin: bool


class AircraftEditRequest(BaseModel):
    """Request model for editing aircraft."""
    registration: Optional[str] = None
    icao_type_code: Optional[str] = None
    type_description: Optional[str] = None
    operator: Optional[str] = None


class AircraftEditResponse(BaseModel):
    """Response model for aircraft data (for editing)."""
    icao24: str
    registration: Optional[str] = None
    icao_type_code: Optional[str] = None
    type_description: Optional[str] = None
    operator: Optional[str] = None
    icao_type_designator: Optional[str] = None
    source: Optional[str] = None
    first_created: Optional[str] = None
    last_modified: Optional[str] = None


@router.post('/admin/login', tags=["admin"])
async def admin_login(
    response: Response,
    form_data: OAuth2PasswordRequestForm = Depends(),
    config: Config = Depends(get_config)
):
    """
    Admin login endpoint.

    Returns 401 Unauthorized for invalid credentials.
    On success, sets the JWT cookie.
    """
    # Verify it's an admin login attempt
    if form_data.username != ADMIN_EMAIL:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid credentials"
        )

    # Find admin user
    admin_user = await User.find_one(User.email == ADMIN_EMAIL)
    if not admin_user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid credentials"
        )

    # Verify password
    verified, _ = password_helper.verify_and_update(
        form_data.password, admin_user.hashed_password
    )
    if not verified:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid credentials"
        )

    # Generate JWT token
    jwt_strategy = get_jwt_strategy(config.JWT_SECRET)
    token = await jwt_strategy.write_token(admin_user)

    # Set cookie using the transport's login method
    login_response = await cookie_transport.get_login_response(token)

    # Copy cookies from the transport response to our response
    for header_name, header_value in login_response.headers.items():
        if header_name.lower() == "set-cookie":
            response.headers.append("set-cookie", header_value)

    return {"status": "ok"}


@router.get('/auth/me', response_model=UserInfo, tags=["auth"])
async def get_current_user_info(current_user: CurrentUserDep) -> UserInfo:
    """
    Get current user information.

    Returns the current user's email, role, and admin status.
    Used by frontend to restore session state on page reload.
    """
    return UserInfo(
        email=current_user.email,
        role=current_user.role,
        is_admin=current_user.role == "admin"
    )


@router.get('/admin/stats', response_model=DashboardStats, tags=["admin"])
async def get_dashboard_stats(
    current_user: AdminUserDep,
    mongodb: Database = Depends(get_mongodb)
) -> DashboardStats:
    """
    Get dashboard statistics.

    Requires admin role. Returns the total number of flights in the database.
    """
    flight_count = mongodb.flights.count_documents({})
    return DashboardStats(flight_count=flight_count)


@router.get('/admin/aircraft/{icao24}', response_model=AircraftEditResponse, tags=["admin"])
async def get_aircraft_for_edit(
    icao24: str,
    current_user: AdminUserDep,
    mongodb: Database = Depends(get_mongodb)
) -> AircraftEditResponse:
    """
    Get aircraft data for editing.

    Requires admin role. Returns full aircraft data including metadata.
    """
    result = mongodb.aircraft.find_one({"modeS": icao24.strip().upper()})

    if not result:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Aircraft not found"
        )

    return AircraftEditResponse(
        icao24=result["modeS"],
        registration=result.get("registration"),
        icao_type_code=result.get("icaoTypeCode"),
        type_description=result.get("type"),
        operator=result.get("registeredOwners"),
        icao_type_designator=result.get("icaoTypeDesignator"),
        source=result.get("source"),
        first_created=result.get("firstCreated").isoformat() if result.get("firstCreated") else None,
        last_modified=result.get("lastModified").isoformat() if result.get("lastModified") else None,
    )


@router.put('/admin/aircraft/{icao24}', response_model=AircraftEditResponse, tags=["admin"])
async def update_aircraft(
    icao24: str,
    data: AircraftEditRequest,
    current_user: AdminUserDep,
    mongodb: Database = Depends(get_mongodb)
) -> AircraftEditResponse:
    """
    Update aircraft data.

    Requires admin role. Updates the specified fields and returns updated aircraft.
    If aircraft doesn't exist, creates a new entry.
    """
    icao24_upper = icao24.strip().upper()
    now = datetime.utcnow()

    # Build update document
    update_fields = {"lastModified": now, "source": "admin"}

    if data.registration is not None:
        update_fields["registration"] = data.registration.strip() if data.registration else None
    if data.icao_type_code is not None:
        update_fields["icaoTypeCode"] = data.icao_type_code.strip() if data.icao_type_code else None
    if data.type_description is not None:
        update_fields["type"] = data.type_description.strip() if data.type_description else None
    if data.operator is not None:
        update_fields["registeredOwners"] = data.operator.strip() if data.operator else None

    # Try to update existing document
    result = mongodb.aircraft.find_one_and_update(
        {"modeS": icao24_upper},
        {"$set": update_fields},
        return_document=ReturnDocument.AFTER
    )

    # If not found, create new document
    if not result:
        new_doc = {
            "modeS": icao24_upper,
            "firstCreated": now,
            **update_fields
        }
        mongodb.aircraft.insert_one(new_doc)
        result = mongodb.aircraft.find_one({"modeS": icao24_upper})

    return AircraftEditResponse(
        icao24=result["modeS"],
        registration=result.get("registration"),
        icao_type_code=result.get("icaoTypeCode"),
        type_description=result.get("type"),
        operator=result.get("registeredOwners"),
        icao_type_designator=result.get("icaoTypeDesignator"),
        source=result.get("source"),
        first_created=result.get("firstCreated").isoformat() if result.get("firstCreated") else None,
        last_modified=result.get("lastModified").isoformat() if result.get("lastModified") else None,
    )


@router.get('/admin/crawler/stats', response_model=CrawlerStats, tags=["admin"])
async def get_crawler_stats(
    request: Request,
    current_user: AdminUserDep,
    mongodb: Database = Depends(get_mongodb),
    config: Config = Depends(get_config)
) -> CrawlerStats:
    """
    Get crawler processing queue statistics.

    Returns queue size, failure rates, and circuit breaker status for each source.
    Used by the admin dashboard for monitoring crawler health.
    """
    crawler_enabled = config.UNKNOWN_AIRCRAFT_CRAWLING

    # Get processing queue stats
    processing_repo = AircraftProcessingRepository(
        mongodb,
        max_attempts=config.CRAWLER_MAX_ATTEMPTS,
        service_error_reset_hours=config.CRAWLER_SERVICE_ERROR_RESET_HOURS
    )
    queue_stats = processing_repo.get_stats()

    # Get circuit breaker stats and sources from the crawler if available
    circuit_breaker_stats = {}
    sources = []
    if hasattr(request.app.state, 'crawler') and request.app.state.crawler:
        raw_cb_stats = request.app.state.crawler.get_circuit_breaker_stats()
        for source_name, stats in raw_cb_stats.items():
            circuit_breaker_stats[source_name] = CircuitBreakerStats(
                state=stats["state"],
                consecutive_failures=stats["consecutive_failures"],
                total_failures=stats["total_failures"],
                total_successes=stats["total_successes"],
                trip_count=stats["trip_count"],
                current_backoff_seconds=stats["current_backoff_seconds"],
                seconds_until_retry=stats["seconds_until_retry"]
            )
        # Get source enabled states
        raw_sources = request.app.state.crawler.get_sources_status()
        sources = [SourceStatusInline(**s) for s in raw_sources]

    return CrawlerStats(
        enabled=crawler_enabled,
        queue_total=queue_stats["total_count"],
        queue_pending=queue_stats["zero_attempts"],
        queue_in_progress=queue_stats["in_progress"],
        not_found_failures=queue_stats["not_found_failures"],
        service_error_failures=queue_stats["service_error_failures"],
        max_attempts_reached=queue_stats["max_attempts_reached"],
        circuit_breakers=circuit_breaker_stats,
        sources=sources
    )


@router.get('/admin/crawler/activity', response_model=CrawlerActivityResponse, tags=["admin"])
async def get_crawler_activity(
    request: Request,
    current_user: AdminUserDep,
) -> CrawlerActivityResponse:
    """
    Get recent crawler activity.

    Returns a list of recent aircraft fetch attempts with their status and source.
    Used by the admin dashboard for monitoring crawler activity in real-time.
    """
    activity = []
    if hasattr(request.app.state, 'crawler') and request.app.state.crawler:
        raw_activity = request.app.state.crawler.get_recent_activity(limit=20)
        activity = [CrawlerActivityItem(**item) for item in raw_activity]

    return CrawlerActivityResponse(activity=activity)


@router.get('/admin/crawler/sources', response_model=SourcesResponse, tags=["admin"])
async def get_crawler_sources(
    request: Request,
    current_user: AdminUserDep,
) -> SourcesResponse:
    """
    Get all crawler sources with their enabled state.

    The enabled state is volatile and resets when the server restarts.
    Requires admin role.
    """
    sources = []
    if hasattr(request.app.state, 'crawler') and request.app.state.crawler:
        raw_sources = request.app.state.crawler.get_sources_status()
        sources = [SourceStatus(**s) for s in raw_sources]

    return SourcesResponse(sources=sources)


@router.post('/admin/crawler/sources/{source_name}/toggle', response_model=SourceStatus, tags=["admin"])
async def toggle_crawler_source(
    request: Request,
    source_name: str,
    data: SourceToggleRequest,
    current_user: AdminUserDep,
) -> SourceStatus:
    """
    Toggle a crawler source's enabled state.

    The enabled state is volatile and resets when the server restarts.
    Requires admin role.
    """
    if not hasattr(request.app.state, 'crawler') or not request.app.state.crawler:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Crawler not available"
        )

    success = request.app.state.crawler.set_source_enabled(source_name, data.enabled)
    if not success:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Source '{source_name}' not found"
        )

    return SourceStatus(name=source_name, enabled=data.enabled)
