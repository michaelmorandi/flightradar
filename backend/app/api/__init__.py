from fastapi import APIRouter

router = APIRouter()

from .endpoints import flights
from .endpoints import aircraft
# Auth routes are now provided by fastapi-users in app/__init__.py

