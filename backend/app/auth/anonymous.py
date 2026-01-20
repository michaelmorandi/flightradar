"""
Anonymous user support for backward compatibility.

The anonymous user allows the existing shared-secret flow to work
with fastapi-users by creating a well-known user account.
"""

import logging

from fastapi_users.password import PasswordHelper

from .models import User
from .manager import ANONYMOUS_EMAIL

logger = logging.getLogger(__name__)

password_helper = PasswordHelper()


async def ensure_anonymous_user(client_secret: str) -> User:
    """
    Ensure the anonymous user exists in the database.

    This user is used for backward compatibility with the
    current shared-secret authentication flow.

    Args:
        client_secret: The shared secret (becomes the password)

    Returns:
        The anonymous User document
    """
    existing = await User.find_one(User.email == ANONYMOUS_EMAIL)

    if existing:
        logger.info("Anonymous user already exists")
        verified, updated_hash = password_helper.verify_and_update(
            client_secret, existing.hashed_password
        )
        if not verified:
            existing.hashed_password = password_helper.hash(client_secret)
            await existing.save()
            logger.info("Anonymous user password updated")
        elif updated_hash:
            existing.hashed_password = updated_hash
            await existing.save()
            logger.info("Anonymous user password hash upgraded")
        return existing

    anonymous_user = User(
        email=ANONYMOUS_EMAIL,
        hashed_password=password_helper.hash(client_secret),
        is_active=True,
        is_superuser=False,
        is_verified=True,
        display_name="Anonymous",
        role="anonymous",
    )

    await anonymous_user.insert()
    logger.info("Anonymous user created successfully")

    return anonymous_user
