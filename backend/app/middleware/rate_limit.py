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

    Priority:
    1. CF-Connecting-IP — set by Cloudflare with the real client IP;
       clients cannot inject or override this header.
    2. request.client.host — the direct TCP peer address set by the ASGI
       server; also not spoofable by clients.

    X-Forwarded-For is intentionally not used: its first entry is
    client-controlled and can be set to any arbitrary value, which would
    allow an attacker to bypass per-IP rate limits entirely.

    Args:
        request: FastAPI request object

    Returns:
        Client IP address as string
    """
    # 1. Cloudflare sets this to the verified client IP before the request
    #    reaches the origin.  It passes through Nginx unchanged and cannot
    #    be injected by a remote client.
    cf_ip = request.headers.get("CF-Connecting-IP")
    if cf_ip:
        return cf_ip.strip()

    # 2. Nginx sets X-Real-IP to $remote_addr (the IP that connected to
    #    Nginx itself).  Unlike X-Forwarded-For, Nginx overwrites this
    #    value rather than appending to a client-supplied one, so it is
    #    safe to trust when a reverse proxy is present.
    #    Requires:  proxy_set_header X-Real-IP $remote_addr;  in nginx.conf
    real_ip = request.headers.get("X-Real-IP")
    if real_ip:
        return real_ip.strip()

    # 3. No proxy: use the raw TCP peer address from the ASGI layer.
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
