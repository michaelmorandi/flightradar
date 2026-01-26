"""
System user management for authentication.

Provides functions to create and manage system users:
- Anonymous user: allows shared-secret flow for backward compatibility
- Admin user: provides administrative access to the dashboard
"""

import logging

from fastapi_users.password import PasswordHelper

from .models import User
from .manager import ANONYMOUS_EMAIL

logger = logging.getLogger(__name__)

password_helper = PasswordHelper()

ADMIN_EMAIL = "admin@system.local"


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


async def ensure_admin_user(admin_password: str) -> User:
    """
    Ensure the admin user exists in the database.

    Creates or updates the admin user with the provided password.
    The admin user has the 'admin' role for accessing the dashboard.

    Args:
        admin_password: The admin password (from ADMIN_PASSWORD env var)

    Returns:
        The admin User document
    """
    existing = await User.find_one(User.email == ADMIN_EMAIL)

    if existing:
        logger.info("Admin user already exists")
        verified, updated_hash = password_helper.verify_and_update(
            admin_password, existing.hashed_password
        )
        if not verified:
            existing.hashed_password = password_helper.hash(admin_password)
            await existing.save()
            logger.info("Admin user password updated")
        elif updated_hash:
            existing.hashed_password = updated_hash
            await existing.save()
            logger.info("Admin user password hash upgraded")
        return existing

    admin_user = User(
        email=ADMIN_EMAIL,
        hashed_password=password_helper.hash(admin_password),
        is_active=True,
        is_superuser=True,
        is_verified=True,
        display_name="Admin",
        role="admin",
    )

    await admin_user.insert()
    logger.info("Admin user created successfully")

    return admin_user
