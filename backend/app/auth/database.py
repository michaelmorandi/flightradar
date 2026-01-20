"""
Async MongoDB connection for authentication using Beanie ODM.

This runs alongside the existing sync PyMongo connection used for flight data.
"""

import logging
from typing import Optional, AsyncGenerator

import certifi
from beanie import init_beanie
from fastapi_users.db import BeanieUserDatabase
from motor.motor_asyncio import AsyncIOMotorClient

from .models import User

logger = logging.getLogger(__name__)

_motor_client: Optional[AsyncIOMotorClient] = None


async def init_auth_database(mongodb_uri: str, db_name: str) -> None:
    """
    Initialize async MongoDB connection for authentication.

    This runs alongside the existing sync PyMongo connection.
    Uses motor for async operations required by fastapi-users/Beanie.
    """
    global _motor_client

    logger.info("Initializing async MongoDB connection for auth...")

    # Only pass TLS CA file when TLS is not explicitly disabled
    uri_lower = mongodb_uri.lower()
    if "ssl=false" in uri_lower or "tls=false" in uri_lower:
        _motor_client = AsyncIOMotorClient(mongodb_uri)
    else:
        _motor_client = AsyncIOMotorClient(mongodb_uri, tlsCAFile=certifi.where())

    await init_beanie(
        database=_motor_client[db_name],
        document_models=[User],
    )

    logger.info("Auth database initialized successfully")


async def close_auth_database() -> None:
    """Close async MongoDB connection."""
    global _motor_client
    if _motor_client:
        _motor_client.close()
        _motor_client = None
        logger.info("Auth database connection closed")


async def get_user_db() -> AsyncGenerator[BeanieUserDatabase, None]:
    """
    Dependency to get the Beanie User database adapter.
    Used by fastapi-users.
    """
    yield BeanieUserDatabase(User)
