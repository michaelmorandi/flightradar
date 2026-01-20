"""
Custom user manager for fastapi-users.
"""

import logging
from datetime import datetime
from typing import Optional, AsyncGenerator

from beanie import PydanticObjectId
from fastapi import Depends, Request
from fastapi_users import BaseUserManager
from fastapi_users.db import BeanieUserDatabase

from .models import User
from .database import get_user_db

logger = logging.getLogger(__name__)

ANONYMOUS_EMAIL = "anonymous@system.local"


class UserManager(BaseUserManager[User, PydanticObjectId]):
    """
    Custom user manager with hooks for logging.
    """

    reset_password_token_secret: str
    verification_token_secret: str

    def parse_id(self, value: str) -> PydanticObjectId:
        """Parse a string ID to PydanticObjectId."""
        return PydanticObjectId(value)

    async def on_after_register(
        self, user: User, request: Optional[Request] = None
    ) -> None:
        logger.info(f"User {user.id} has registered.")

    async def on_after_login(
        self,
        user: User,
        request: Optional[Request] = None,
        response=None,
    ) -> None:
        logger.info(f"User {user.id} logged in.")
        user.last_login = datetime.utcnow()
        await user.save()

    async def on_after_forgot_password(
        self, user: User, token: str, request: Optional[Request] = None
    ) -> None:
        logger.info(f"Password reset requested for user {user.id}")

    async def on_after_request_verify(
        self, user: User, token: str, request: Optional[Request] = None
    ) -> None:
        logger.info(f"Verification requested for user {user.id}")


async def get_user_manager(
    user_db: BeanieUserDatabase = Depends(get_user_db),
) -> AsyncGenerator[UserManager, None]:
    """Dependency to get the user manager."""
    yield UserManager(user_db)
