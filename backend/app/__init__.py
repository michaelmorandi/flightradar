import logging
from fastapi import FastAPI
from fastapi.middleware.trustedhost import TrustedHostMiddleware
from fastapi.middleware.cors import CORSMiddleware
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded

from .config import Config, app_state
from .meta import MetaInformation
from .data import init_mongodb
from .core.utils.logging import init_logging
from .middleware import limiter, rate_limit_exceeded_handler
from .auth import init_auth_database, close_auth_database, ensure_anonymous_user, ensure_admin_user
from .auth.config import setup_auth_routes

from .scheduling import configure_scheduling



def create_app():
    app = FastAPI(
        title="Flight Radar",
        description="ADS-B flight data API",
        version="1.0.0"
    )

    # Rate limiting
    app.state.limiter = limiter
    app.add_exception_handler(RateLimitExceeded, rate_limit_exceeded_handler)

    # Config
    conf = Config()
    init_logging(conf.LOGGING_CONFIG)
    
    logger = logging.getLogger(__name__)

    # Initialize MongoDB
    mongodb = init_mongodb(
        conf.MONGODB_URI,
        conf.MONGODB_DB_NAME,
        conf.DB_RETENTION_MIN
    )
    app.state.mongodb = mongodb
    app_state.mongodb = mongodb

    # Store app state
    app.state.config = conf
    app.state.metaInfo = MetaInformation()

    # Log version information
    logger.info(f"Flight Radar API starting - Commit: {app.state.metaInfo.commit_id}, Built: {app.state.metaInfo.build_timestamp}")

    from .core.utils.modes_util import ModesUtil
    app.state.modes_util = ModesUtil(conf.DATA_FOLDER)

    # Add middleware
    app.add_middleware(TrustedHostMiddleware, allowed_hosts=["*"])

    # Allowed origins for CORS: use configured value or default to localhost for development
    if conf.ALLOWED_ORIGINS:
        allowed_origins = [origin.strip() for origin in conf.ALLOWED_ORIGINS.split(",")]
    else:
        allowed_origins = ["http://localhost:5173", "http://localhost:8000", "http://127.0.0.1:5173"]

    app.add_middleware(
        CORSMiddleware,
        allow_origins=allowed_origins,
        allow_credentials=True,
        allow_methods=["GET", "POST", "PUT", "OPTIONS"],
        allow_headers=["Content-Type", "Authorization", "Pragma", "Cache-Control", "Expires"],
    )

    # Import and include routers
    from .api import router as api_router
    app.include_router(api_router, prefix="/api/v1")

    # Set up auth routes
    fastapi_users = setup_auth_routes(app, conf.JWT_SECRET)
    app.state.fastapi_users = fastapi_users

    # Configure async tasks
    @app.on_event("startup")
    async def startup():
        # Capture the event loop and pass it to scheduling
        import asyncio
        from .sse.notifier import SSENotifier

        loop = asyncio.get_running_loop()
        SSENotifier._main_loop = loop
        logger.info(f"Captured FastAPI event loop for SSE: {loop}")

        # Initialize auth database (async MongoDB via Beanie)
        await init_auth_database(conf.MONGODB_URI, conf.MONGODB_DB_NAME)

        # Ensure anonymous user exists for backward compatibility
        if conf.CLIENT_SECRET:
            await ensure_anonymous_user(conf.CLIENT_SECRET)

        # Ensure admin user exists if password is configured
        if conf.ADMIN_PASSWORD:
            await ensure_admin_user(conf.ADMIN_PASSWORD)

        configure_scheduling(app, conf)

    @app.on_event("shutdown")
    async def shutdown():
        logger.info("Application shutdown initiated")
        await close_auth_database()

    return app


# Create app instance for ASGI servers
app = create_app()
