"""
MongoDB Database Initialization

Provides database connection and schema initialization.
"""

import certifi
import logging
from pymongo import MongoClient

from .schema import ensure_schema


def init_mongodb(connection_string: str, db_name: str, retention_minutes: int):
    """
    Initialize MongoDB connection and ensure schema exists.

    Args:
        connection_string: MongoDB connection string
        db_name: Database name
        retention_minutes: Retention time for flight/position data (0 to disable TTL)

    Returns:
        Database instance with schema initialized
    """
    logger = logging.getLogger("MongoDBInit")

    # Connect with the connection string
    try:
        # Log connection attempt without exposing credentials
        if '@' in connection_string:
            masked_uri = connection_string.split('@')[0].split('://')[0] + '://*****@' + connection_string.split('@')[1]
            logger.info(f"Connecting to MongoDB at {masked_uri}")
        else:
            logger.info(f"Connecting to MongoDB")

        # Only add tlsCAFile if ssl or tls are not explicitly set to false
        lower_conn = connection_string.lower()
        use_tls = not (("ssl=false" in lower_conn) or ("tls=false" in lower_conn))
        if use_tls:
            client = MongoClient(connection_string, tlsCAFile=certifi.where())
        else:
            client = MongoClient(connection_string)

        # Verify connection by pinging
        client.admin.command('ping')
        logger.info("Successfully connected to MongoDB")
    except Exception as e:
        logger.error(f"MongoDB connection failed: {str(e)}")
        raise

    db = client[db_name]

    # Store collection names for backward compatibility
    db.flights_collection = "flights"
    db.positions_collection = "positions"

    # Ensure all collections and indexes exist
    ensure_schema(db, retention_minutes)

    return db
