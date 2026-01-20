"""
fastapi-users configuration.
"""

from typing import TYPE_CHECKING

from beanie import PydanticObjectId
from fastapi_users import FastAPIUsers
from fastapi_users.authentication import (
    AuthenticationBackend,
    CookieTransport,
    JWTStrategy,
)

from .models import User
from .manager import get_user_manager

if TYPE_CHECKING:
    from fastapi import FastAPI

# Cookie transport configuration (matches previous implementation)
cookie_transport = CookieTransport(
    cookie_name="access_token",
    cookie_max_age=900,  # 15 minutes
    cookie_httponly=True,  # XSS protection
    cookie_secure=False,  # Set True in production (HTTPS only)
    cookie_samesite="lax",  # Set "strict" in production
)


def get_jwt_strategy(secret: str, lifetime_seconds: int = 900) -> JWTStrategy:
    """
    JWT strategy for token generation/validation.

    Args:
        secret: JWT signing secret
        lifetime_seconds: Token lifetime (default 15 min)
    """
    return JWTStrategy(
        secret=secret,
        lifetime_seconds=lifetime_seconds,
        algorithm="HS256",
        token_audience=["flightradar:api"],
    )


def create_auth_backend(jwt_secret: str) -> AuthenticationBackend:
    """Create the authentication backend."""
    return AuthenticationBackend(
        name="jwt-cookie",
        transport=cookie_transport,
        get_strategy=lambda: get_jwt_strategy(jwt_secret),
    )


def create_fastapi_users(jwt_secret: str) -> FastAPIUsers[User, PydanticObjectId]:
    """
    Create FastAPIUsers instance with configured backend.

    Args:
        jwt_secret: Secret for JWT signing

    Returns:
        Configured FastAPIUsers instance
    """
    auth_backend = create_auth_backend(jwt_secret)

    return FastAPIUsers[User, PydanticObjectId](
        get_user_manager,
        [auth_backend],
    )


def setup_auth_routes(app: "FastAPI", jwt_secret: str) -> FastAPIUsers[User, PydanticObjectId]:
    """
    Set up authentication routes on the FastAPI app.

    Args:
        app: FastAPI application
        jwt_secret: Secret for JWT signing

    Returns:
        FastAPIUsers instance for dependency injection
    """
    fastapi_users = create_fastapi_users(jwt_secret)
    auth_backend = create_auth_backend(jwt_secret)

    # Login/logout routes
    app.include_router(
        fastapi_users.get_auth_router(auth_backend),
        prefix="/api/v1/auth/jwt",
        tags=["auth"],
    )

    return fastapi_users
