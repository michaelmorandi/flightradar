"""
User model for fastapi-users with Beanie ODM.
"""

from datetime import datetime
from typing import Optional

from beanie import Document
from fastapi_users.db import BeanieBaseUser
from pydantic import Field
from pymongo import ASCENDING
from pymongo.collation import Collation


class User(BeanieBaseUser, Document):
    """
    User document for MongoDB.

    Inherits from BeanieBaseUser which provides:
    - id: UUID (mapped to MongoDB _id)
    - email: str
    - hashed_password: str
    - is_active: bool = True
    - is_superuser: bool = False
    - is_verified: bool = False
    """

    display_name: Optional[str] = None
    created_at: datetime = Field(default_factory=datetime.utcnow)
    last_login: Optional[datetime] = None
    role: str = Field(default="user")  # "user", "admin", "anonymous"

    class Settings:
        name = "users"
        email_collation = Collation("en", strength=2)
