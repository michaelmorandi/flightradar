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

from .. import router
from ..dependencies import AdminUserDep, CurrentUserDep, get_mongodb, get_config
from ...auth.models import User
from ...auth.anonymous import ADMIN_EMAIL, password_helper
from ...auth.config import get_jwt_strategy, cookie_transport
from ...config import Config


class DashboardStats(BaseModel):
    """Dashboard statistics response model."""
    flight_count: int


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
