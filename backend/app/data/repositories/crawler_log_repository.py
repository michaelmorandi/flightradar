"""
Crawler Log Repository

Stores crawler query history for aircraft that trigger calls to multiple sources.
Each log entry contains the ICAO24 address, timestamp, and query results from each source.
"""

from datetime import datetime
from typing import List, Optional, Any
from pymongo.database import Database
from pymongo.errors import PyMongoError, OperationFailure
import logging

logger = logging.getLogger(__name__)


class CrawlerLogRepository:
    """MongoDB repository for storing crawler query logs."""

    COLLECTION_NAME = "crawler_logs"

    def __init__(self, mongodb: Database):
        self.db = mongodb
        self._ensure_collection()

    def _ensure_collection(self):
        """Ensure collection exists with proper indexes."""
        if self.COLLECTION_NAME not in self.db.list_collection_names():
            self.db.create_collection(self.COLLECTION_NAME)

        collection = self.db[self.COLLECTION_NAME]
        # Index for querying by icao24
        collection.create_index("icao24")
        # TTL index to auto-expire old logs after 30 days
        try:
            collection.create_index("timestamp", expireAfterSeconds=30 * 24 * 60 * 60)
        except OperationFailure as e:
            if e.code == 85:  # IndexOptionsConflict
                # Drop existing index and recreate with TTL
                collection.drop_index("timestamp_1")
                collection.create_index("timestamp", expireAfterSeconds=30 * 24 * 60 * 60)
            else:
                raise

    def save_query_log(
        self,
        icao24: str,
        queries: List[dict],
        final_status: str,
        final_source: Optional[str] = None,
    ) -> bool:
        """
        Save a crawler query log entry.

        Args:
            icao24: Aircraft ICAO24 address
            queries: List of query results from each source, each containing:
                - source: Source name
                - status: Query status (success, partial, not_found, service_error)
                - duration_ms: Query duration in milliseconds
                - payload: Raw response data (if any)
                - error: Error message (if any)
            final_status: Final crawl status (success, partial, not_found, service_error)
            final_source: Name of the source that provided the final result (if any)

        Returns:
            True if saved successfully, False otherwise
        """
        try:
            log_entry = {
                "icao24": icao24.upper(),
                "timestamp": datetime.utcnow(),
                "queries": queries,
                "final_status": final_status,
                "final_source": final_source,
                "query_count": len(queries),
            }

            self.db[self.COLLECTION_NAME].insert_one(log_entry)
            return True

        except PyMongoError as e:
            logger.error(f"Failed to save crawler log for {icao24}: {e}")
            return False

    def get_logs_for_aircraft(
        self,
        icao24: str,
        limit: int = 50,
    ) -> List[dict]:
        """
        Get recent crawler logs for a specific aircraft.

        Args:
            icao24: Aircraft ICAO24 address
            limit: Maximum number of logs to return

        Returns:
            List of log entries, most recent first
        """
        try:
            cursor = (
                self.db[self.COLLECTION_NAME]
                .find({"icao24": icao24.upper()})
                .sort("timestamp", -1)
                .limit(limit)
            )
            return list(cursor)
        except PyMongoError as e:
            logger.error(f"Failed to get crawler logs for {icao24}: {e}")
            return []

    def get_recent_logs(
        self,
        limit: int = 100,
        min_query_count: int = 2,
    ) -> List[dict]:
        """
        Get recent crawler logs, optionally filtering by minimum query count.

        Args:
            limit: Maximum number of logs to return
            min_query_count: Minimum number of source queries (default 2 for multi-source)

        Returns:
            List of log entries, most recent first
        """
        try:
            query = {}
            if min_query_count > 1:
                query["query_count"] = {"$gte": min_query_count}

            cursor = (
                self.db[self.COLLECTION_NAME]
                .find(query)
                .sort("timestamp", -1)
                .limit(limit)
            )
            return list(cursor)
        except PyMongoError as e:
            logger.error(f"Failed to get recent crawler logs: {e}")
            return []

    def get_stats(self) -> dict:
        """Get statistics about crawler logs."""
        try:
            collection = self.db[self.COLLECTION_NAME]
            total_logs = collection.count_documents({})
            multi_source_logs = collection.count_documents({"query_count": {"$gte": 2}})

            # Get status distribution
            pipeline = [
                {"$group": {"_id": "$final_status", "count": {"$sum": 1}}},
            ]
            status_counts = {doc["_id"]: doc["count"] for doc in collection.aggregate(pipeline)}

            return {
                "total_logs": total_logs,
                "multi_source_logs": multi_source_logs,
                "status_distribution": status_counts,
            }
        except PyMongoError as e:
            logger.error(f"Failed to get crawler log stats: {e}")
            return {
                "total_logs": 0,
                "multi_source_logs": 0,
                "status_distribution": {},
            }
