"""
Authentication module using fastapi-users with Beanie (MongoDB ODM).

This module provides:
- User model for MongoDB
- JWT cookie-based authentication
- Anonymous user support for backward compatibility
"""

from .models import User
from .database import init_auth_database, close_auth_database, get_user_db
from .config import create_fastapi_users, create_auth_backend
from .manager import get_user_manager, UserManager
from .anonymous import ensure_anonymous_user
from .manager import ANONYMOUS_EMAIL

__all__ = [
    "User",
    "init_auth_database",
    "close_auth_database",
    "get_user_db",
    "create_fastapi_users",
    "create_auth_backend",
    "get_user_manager",
    "UserManager",
    "ensure_anonymous_user",
    "ANONYMOUS_EMAIL",
]
