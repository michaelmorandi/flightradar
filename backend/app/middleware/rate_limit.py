"""
Rate limiting middleware using slowapi.

Implements rate limiting for API endpoints to prevent abuse:
- Auth endpoints: Strict limits to prevent brute force
- General API endpoints: Moderate limits for normal usage
- SSE connections: Connection limit per IP
"""

import logging
from slowapi import Limiter
from slowapi.util import get_remote_address
from slowapi.errors import RateLimitExceeded
from fastapi import Request, Response
from fastapi.responses import JSONResponse

logger = logging.getLogger(__name__)


def rate_limit_key_func(request: Request) -> str:
    """
    Key function for rate limiting based on client IP address.

    Args:
        request: FastAPI request object

    Returns:
        Client IP address as string
    """
    # Try to get real IP from X-Forwarded-For header (if behind proxy)
    forwarded = request.headers.get("X-Forwarded-For")
    if forwarded:
        # Take the first IP if multiple are present
        return forwarded.split(",")[0].strip()

    # Fall back to direct client IP
    return get_remote_address(request)


# Initialize rate limiter
limiter = Limiter(
    key_func=rate_limit_key_func,
    default_limits=["1000/hour"],  # Default limit for all endpoints
    storage_uri="memory://",       # In-memory storage (use Redis for production scale)
    headers_enabled=True           # Add rate limit headers to responses
)


def rate_limit_exceeded_handler(request: Request, exc: RateLimitExceeded) -> Response:
    """
    Custom handler for rate limit exceeded errors.

    Returns a JSON response with rate limit information.
    """
    logger.warning(
        f"Rate limit exceeded for {rate_limit_key_func(request)} "
        f"on {request.url.path}"
    )

    return JSONResponse(
        status_code=429,
        content={
            "error": "Rate limit exceeded",
            "detail": "Too many requests. Please try again later.",
            "retry_after": exc.detail
        },
        headers={
            "Retry-After": str(exc.detail),
            "X-RateLimit-Limit": str(exc.headers.get("X-RateLimit-Limit", "unknown")),
            "X-RateLimit-Remaining": "0",
            "X-RateLimit-Reset": str(exc.headers.get("X-RateLimit-Reset", "unknown"))
        }
    )


# Rate limit decorators for specific endpoint types

# Auth endpoints - strict limits to prevent brute force
AUTH_CHALLENGE_LIMIT = "100/hour"  # Get challenge
AUTH_TOKEN_LIMIT = "20/hour"        # Exchange challenge for token
AUTH_REFRESH_LIMIT = "100/hour"     # Refresh token

# General API endpoints - moderate limits
API_GENERAL_LIMIT = "1000/hour"     # General API calls
SSE_CONNECTION_LIMIT = "10/hour"    # SSE connection attempts
