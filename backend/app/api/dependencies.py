from typing import Annotated, Optional
from fastapi import Depends, HTTPException, status
from pymongo.database import Database

from ..data.repositories.aircraft_repository import AircraftRepository
from ..data.repositories.mongodb_repository import MongoDBRepository
from ..core.utils.modes_util import ModesUtil
from ..config import Config, app_state
from ..meta import MetaInformation
from ..auth.models import User
from ..auth.config import create_fastapi_users

def get_mongodb() -> Database:
    """Get MongoDB database connection"""
    return app_state.mongodb

MongoDBDep = Annotated[Database, Depends(get_mongodb)]

# Singleton config instance - loaded once at startup
_config = Config()

def get_config() -> Config:
    """Get application configuration (singleton)"""
    return _config

def get_modes_util() -> ModesUtil:
    """Get ModesUtil instance"""
    return ModesUtil(_config.DATA_FOLDER)

def get_meta_info() -> MetaInformation:
    """Get MetaInformation instance"""
    return MetaInformation()

def get_aircraft_repository(mongodb: MongoDBDep) -> AircraftRepository:
    """Get AircraftRepository instance"""
    return AircraftRepository(mongodb)

def get_mongodb_repository(mongodb: MongoDBDep) -> MongoDBRepository:
    """Get MongoDBRepository instance"""
    return MongoDBRepository(mongodb)

ConfigDep = Annotated[Config, Depends(get_config)]
ModesUtilDep = Annotated[ModesUtil, Depends(get_modes_util)]
MetaInfoDep = Annotated[MetaInformation, Depends(get_meta_info)]
AircraftRepositoryDep = Annotated[AircraftRepository, Depends(get_aircraft_repository)]
MongoDBRepositoryDep = Annotated[MongoDBRepository, Depends(get_mongodb_repository)]

# Create fastapi-users instance for dependency injection
_fastapi_users = create_fastapi_users(_config.JWT_SECRET)

# Use fastapi-users' built-in dependencies
_current_active_user = _fastapi_users.current_user(active=True)
_current_optional_user = _fastapi_users.current_user(active=True, optional=True)


# Type annotation for authenticated user dependency
CurrentUserDep = Annotated[User, Depends(_current_active_user)]


# Type annotation for optional user dependency
OptionalUserDep = Annotated[Optional[User], Depends(_current_optional_user)]


def require_admin_user(current_user: CurrentUserDep) -> User:
    """
    Dependency that requires the current user to have admin role.

    Raises HTTPException 403 if user is not an admin.
    """
    if current_user.role != "admin":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Admin access required"
        )
    return current_user


# Type annotation for admin user dependency
AdminUserDep = Annotated[User, Depends(require_admin_user)]

