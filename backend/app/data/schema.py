"""
MongoDB Schema Definition

Centralized schema definitions for all MongoDB collections, indexes, and TTLs.
This module provides a single source of truth for database structure.

Best Practices Implemented:
1. Single source of truth - All collections defined in one place
2. Declarative configuration - Schema defined as data structures
3. Idempotent setup - Safe to call multiple times
4. TTL management - Handles TTL index conflicts gracefully
"""

import logging
from dataclasses import dataclass, field
from typing import List, Optional, Dict, Any
from pymongo.database import Database
from pymongo.errors import OperationFailure, CollectionInvalid

logger = logging.getLogger(__name__)


# TTL Constants (in seconds)
TTL_7_DAYS = 7 * 24 * 60 * 60
TTL_30_DAYS = 30 * 24 * 60 * 60


@dataclass
class IndexDefinition:
    """Definition for a MongoDB index."""
    keys: Any  # str for single field, list of tuples for compound
    unique: bool = False
    ttl_seconds: Optional[int] = None  # For TTL indexes
    name: Optional[str] = None  # Optional custom name

    def get_key_spec(self):
        """Convert keys to pymongo format."""
        if isinstance(self.keys, str):
            return self.keys
        return self.keys  # Already in [(field, direction), ...] format


@dataclass
class CollectionDefinition:
    """Definition for a MongoDB collection."""
    name: str
    indexes: List[IndexDefinition] = field(default_factory=list)
    timeseries: Optional[Dict[str, Any]] = None  # For time-series collections
    ttl_seconds: Optional[int] = None  # Collection-level TTL (for time-series)


# =============================================================================
# COLLECTION DEFINITIONS
# =============================================================================

COLLECTIONS = {
    # -------------------------------------------------------------------------
    # Flights Collection
    # Stores flight records - each flight (even from same aircraft) gets new record
    # -------------------------------------------------------------------------
    "flights": CollectionDefinition(
        name="flights",
        indexes=[
            IndexDefinition(keys="modeS", unique=False),
            IndexDefinition(keys="last_contact"),
            IndexDefinition(keys="is_military"),
            IndexDefinition(keys=[("modeS", 1), ("callsign", 1)]),
            # TTL index using expire_at field - expireAfterSeconds=0 means
            # document expires at the time specified in the expire_at field
            IndexDefinition(keys="expire_at", ttl_seconds=0),
        ]
    ),

    # -------------------------------------------------------------------------
    # Positions Collection (Time-Series)
    # Stores position data for flights
    # -------------------------------------------------------------------------
    "positions": CollectionDefinition(
        name="positions",
        timeseries={
            "timeField": "timestmp",
            "metaField": "flight_id",
            "granularity": "seconds"
        },
        indexes=[
            IndexDefinition(keys="flight_id"),
            IndexDefinition(keys=[("flight_id", 1), ("timestmp", 1)]),
        ]
    ),

    # -------------------------------------------------------------------------
    # Aircraft Collection
    # Master aircraft data (registration, type, operator, etc.)
    # -------------------------------------------------------------------------
    "aircraft": CollectionDefinition(
        name="aircraft",
        indexes=[
            IndexDefinition(keys="modeS", unique=True),
        ]
    ),

    # -------------------------------------------------------------------------
    # Aircraft Processing Queue
    # Tracks aircraft pending metadata crawling
    # -------------------------------------------------------------------------
    "aircraft_to_process": CollectionDefinition(
        name="aircraft_to_process",
        indexes=[
            IndexDefinition(keys="modeS", unique=True),
            IndexDefinition(keys="query_attempts"),
            IndexDefinition(keys="last_attempt_time"),
            IndexDefinition(keys="failure_type"),
        ]
    ),

    # -------------------------------------------------------------------------
    # Crawler Logs
    # Stores crawler query history for multi-source lookups
    # TTL: 7 days
    # -------------------------------------------------------------------------
    "crawler_logs": CollectionDefinition(
        name="crawler_logs",
        indexes=[
            IndexDefinition(keys="icao24"),
            IndexDefinition(keys="timestamp", ttl_seconds=TTL_7_DAYS),
        ]
    ),
}


def _create_index_safe(collection, index_def: IndexDefinition) -> bool:
    """
    Create an index, handling TTL conflicts gracefully.

    Returns True if index was created/updated, False if skipped.
    """
    keys = index_def.get_key_spec()
    kwargs = {}

    if index_def.unique:
        kwargs["unique"] = True

    if index_def.ttl_seconds is not None:
        kwargs["expireAfterSeconds"] = index_def.ttl_seconds

    if index_def.name:
        kwargs["name"] = index_def.name

    try:
        collection.create_index(keys, **kwargs)
        return True
    except OperationFailure as e:
        if e.code == 85:  # IndexOptionsConflict
            # TTL index exists with different options - drop and recreate
            index_name = index_def.name
            if not index_name:
                # Generate default index name
                if isinstance(keys, str):
                    index_name = f"{keys}_1"
                else:
                    index_name = "_".join(f"{k}_{v}" for k, v in keys)

            logger.info(f"Recreating index {index_name} with new options")
            try:
                collection.drop_index(index_name)
                collection.create_index(keys, **kwargs)
                return True
            except Exception as drop_err:
                logger.warning(f"Could not recreate index {index_name}: {drop_err}")
                return False
        else:
            raise


def _ensure_collection(db: Database, coll_def: CollectionDefinition,
                       retention_minutes: Optional[int] = None) -> None:
    """
    Ensure a collection exists with proper indexes.

    Args:
        db: MongoDB database
        coll_def: Collection definition
        retention_minutes: Optional retention time for TTL (overrides collection default)
    """
    collection_name = coll_def.name

    # Handle time-series collections
    if coll_def.timeseries:
        if collection_name not in db.list_collection_names():
            collection_options = {"timeseries": coll_def.timeseries}

            # Set TTL for time-series at collection level
            if retention_minutes and retention_minutes > 0:
                collection_options["expireAfterSeconds"] = retention_minutes * 60
                logger.info(f"Creating time-series collection {collection_name} "
                           f"with TTL={retention_minutes} minutes")

            try:
                db.create_collection(collection_name, **collection_options)
            except CollectionInvalid:
                # Collection already exists
                pass
    else:
        # Standard collection
        if collection_name not in db.list_collection_names():
            db.create_collection(collection_name)

    # Create indexes
    collection = db[collection_name]
    for index_def in coll_def.indexes:
        # Skip TTL index for flights if no retention specified
        if (collection_name == "flights" and
            index_def.keys == "expire_at" and
            (not retention_minutes or retention_minutes <= 0)):
            continue

        _create_index_safe(collection, index_def)


def ensure_schema(db: Database, retention_minutes: Optional[int] = None) -> None:
    """
    Ensure all collections and indexes exist.

    This function is idempotent - safe to call multiple times.
    It will create missing collections and indexes, and update
    TTL indexes if their configuration has changed.

    Args:
        db: MongoDB database instance
        retention_minutes: Optional retention time for flight/position data
    """
    logger.info("Ensuring database schema...")

    for coll_name, coll_def in COLLECTIONS.items():
        try:
            _ensure_collection(db, coll_def, retention_minutes)
            logger.debug(f"Ensured collection: {coll_name}")
        except Exception as e:
            logger.error(f"Error ensuring collection {coll_name}: {e}")
            raise

    logger.info("Database schema verified")


def get_collection_names() -> List[str]:
    """Get list of all defined collection names."""
    return list(COLLECTIONS.keys())


def get_collection_definition(name: str) -> Optional[CollectionDefinition]:
    """Get the definition for a specific collection."""
    return COLLECTIONS.get(name)
