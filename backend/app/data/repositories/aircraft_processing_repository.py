import logging
from datetime import datetime, timedelta
from enum import Enum
from typing import List, Optional
from pymongo.database import Database
from pymongo.errors import PyMongoError

logger = logging.getLogger(__name__)


class FailureType(Enum):
    """Type of failure when querying metadata sources"""
    NONE = "none"  # No failure yet
    NOT_FOUND = "not_found"  # Aircraft not in any source (permanent)
    SERVICE_ERROR = "service_error"  # Temporary error (should retry after cooldown)


class CrawlReason(Enum):
    """Reason why aircraft was queued for crawling"""
    NOT_IN_DB = "not_in_db"  # Aircraft not found in database
    NO_TIMESTAMP = "no_timestamp"  # Aircraft has no lastModified timestamp
    INCOMPLETE_STALE = "incomplete_stale"  # Aircraft is incomplete and stale
    STALE = "stale"  # Aircraft data is stale (complete but old)
    UNKNOWN = "unknown"  # Unknown reason (error during classification)


class AircraftProcessingRepository:
    """
    Repository for managing aircraft that need metadata processing.

    Tracks:
    - query_attempts: Number of attempts made
    - last_attempt_time: When the last attempt was made
    - failure_type: Whether failure was "not found" or "service error"

    Service errors are reset after a configurable time period, allowing retry
    when external services recover from outages.
    """

    def __init__(self, mongodb: Database, max_attempts: int = 5,
                 service_error_reset_hours: int = 6):
        self.db = mongodb
        self.collection_name = "aircraft_to_process"
        self.max_attempts = max_attempts
        self.service_error_reset_hours = service_error_reset_hours

        # Create collection and indexes if needed
        if self.collection_name not in self.db.list_collection_names():
            self.db.create_collection(self.collection_name)

        collection = self.db[self.collection_name]
        collection.create_index("modeS", unique=True)
        collection.create_index("query_attempts")
        collection.create_index("last_attempt_time")
        collection.create_index("failure_type")

    def add_aircraft(self, icao24: str, crawl_reason: CrawlReason = CrawlReason.UNKNOWN) -> bool:
        """Add aircraft to processing queue with crawl reason"""
        try:
            self.db[self.collection_name].insert_one({
                "modeS": icao24.upper(),
                "query_attempts": 0,
                "sources_queried": [],
                "last_attempt_time": None,
                "failure_type": FailureType.NONE.value,
                "crawl_reason": crawl_reason.value,
                "created_at": datetime.now()
            })
            return True
        except PyMongoError as e:
            if hasattr(e, 'code') and e.code == 11000:  # Duplicate key error
                return True
            logger.error(f"Failed to add aircraft {icao24}: {e}")
            return False

    def get_crawl_reason(self, icao24: str) -> Optional[str]:
        """Get the crawl reason for an aircraft in the processing queue"""
        try:
            doc = self.db[self.collection_name].find_one(
                {"modeS": icao24.upper()},
                {"crawl_reason": 1}
            )
            return doc.get("crawl_reason") if doc else None
        except PyMongoError as e:
            logger.error(f"Failed to get crawl reason for {icao24}: {e}")
            return None

    def get_aircraft_for_processing(self, limit: int = 50) -> List[str]:
        """
        Get aircraft eligible for processing.

        Returns aircraft that:
        1. Have fewer than max_attempts AND failure_type is "not_found"
        2. OR have failure_type "service_error" AND last attempt was > reset_hours ago
        3. OR are new (no attempts yet)
        """
        try:
            reset_threshold = datetime.now() - timedelta(hours=self.service_error_reset_hours)

            # Query for eligible aircraft
            cursor = self.db[self.collection_name].find({
                "$or": [
                    # New aircraft (no attempts yet)
                    {"query_attempts": 0},
                    # Not found failures under max attempts
                    {
                        "failure_type": FailureType.NOT_FOUND.value,
                        "query_attempts": {"$lt": self.max_attempts}
                    },
                    # Service errors that have cooled down (reset attempts to retry)
                    {
                        "failure_type": FailureType.SERVICE_ERROR.value,
                        "last_attempt_time": {"$lt": reset_threshold}
                    },
                    # Legacy records without failure_type (treat as retriable)
                    {
                        "failure_type": {"$exists": False},
                        "query_attempts": {"$lt": self.max_attempts}
                    },
                    {
                        "failure_type": FailureType.NONE.value,
                        "query_attempts": {"$lt": self.max_attempts}
                    }
                ]
            }).sort([
                ("query_attempts", 1),  # Prioritize fewer attempts
                ("last_attempt_time", 1),  # Then oldest attempts
                ("_id", 1)
            ]).limit(limit)

            return [doc["modeS"] for doc in cursor]
        except PyMongoError as e:
            logger.error(f"Failed to get aircraft for processing: {e}")
            return []

    def record_not_found(self, icao24: str) -> bool:
        """
        Record a "not found" failure (aircraft doesn't exist in source).
        Increments attempts - after max_attempts, aircraft won't be retried.
        """
        try:
            result = self.db[self.collection_name].update_one(
                {"modeS": icao24.upper()},
                {
                    "$inc": {"query_attempts": 1},
                    "$set": {
                        "failure_type": FailureType.NOT_FOUND.value,
                        "last_attempt_time": datetime.now()
                    }
                }
            )
            return result.modified_count > 0
        except PyMongoError as e:
            logger.error(f"Failed to record not_found for {icao24}: {e}")
            return False

    def record_service_error(self, icao24: str, error_message: str = None) -> bool:
        """
        Record a service error (temporary failure, will be retried after cooldown).
        Does NOT increment attempts - service errors are retried after reset period.
        """
        try:
            update = {
                "$set": {
                    "failure_type": FailureType.SERVICE_ERROR.value,
                    "last_attempt_time": datetime.now()
                }
            }
            if error_message:
                update["$set"]["last_error"] = error_message

            result = self.db[self.collection_name].update_one(
                {"modeS": icao24.upper()},
                update
            )
            return result.modified_count > 0
        except PyMongoError as e:
            logger.error(f"Failed to record service_error for {icao24}: {e}")
            return False

    def increment_attempts(self, icao24: str) -> bool:
        """Increment query attempts for an aircraft (legacy method)"""
        return self.record_not_found(icao24)

    def reset_service_error_attempts(self) -> int:
        """
        Reset attempts for aircraft with service errors after cooldown period.
        This allows them to be retried when external services recover.
        """
        try:
            reset_threshold = datetime.now() - timedelta(hours=self.service_error_reset_hours)

            result = self.db[self.collection_name].update_many(
                {
                    "failure_type": FailureType.SERVICE_ERROR.value,
                    "last_attempt_time": {"$lt": reset_threshold}
                },
                {
                    "$set": {
                        "query_attempts": 0,
                        "failure_type": FailureType.NONE.value
                    }
                }
            )

            if result.modified_count > 0:
                logger.info(f"Reset {result.modified_count} aircraft with expired service errors")

            return result.modified_count
        except PyMongoError as e:
            logger.error(f"Failed to reset service error attempts: {e}")
            return 0

    def remove_aircraft(self, icao24: str) -> bool:
        """Remove aircraft from processing queue (successfully processed)"""
        try:
            result = self.db[self.collection_name].delete_one({"modeS": icao24.upper()})
            return result.deleted_count > 0
        except PyMongoError as e:
            logger.error(f"Failed to remove aircraft {icao24}: {e}")
            return False

    def aircraft_exists(self, icao24: str) -> bool:
        """Check if aircraft exists in processing queue"""
        try:
            result = self.db[self.collection_name].find_one(
                {"modeS": icao24.upper()},
                {"_id": 1}
            )
            return result is not None
        except PyMongoError as e:
            logger.error(f"Failed to check if aircraft {icao24} exists: {e}")
            return False

    def cleanup_failed_aircraft(self, max_attempts: int = None) -> int:
        """
        Remove aircraft that have reached max attempts with NOT_FOUND failures.
        Service error failures are NOT cleaned up - they will be retried after cooldown.
        """
        if max_attempts is None:
            max_attempts = self.max_attempts

        try:
            result = self.db[self.collection_name].delete_many({
                "failure_type": FailureType.NOT_FOUND.value,
                "query_attempts": {"$gte": max_attempts}
            })

            if result.deleted_count > 0:
                logger.info(f"Cleaned up {result.deleted_count} aircraft with max 'not found' attempts")

            return result.deleted_count
        except PyMongoError as e:
            logger.error(f"Failed to cleanup failed aircraft: {e}")
            return 0

    def get_stats(self) -> dict:
        """Get detailed statistics about the processing queue"""
        try:
            total = self.db[self.collection_name].count_documents({})
            zero_attempts = self.db[self.collection_name].count_documents({"query_attempts": 0})
            not_found_count = self.db[self.collection_name].count_documents({
                "failure_type": FailureType.NOT_FOUND.value
            })
            service_error_count = self.db[self.collection_name].count_documents({
                "failure_type": FailureType.SERVICE_ERROR.value
            })
            max_attempts_reached = self.db[self.collection_name].count_documents({
                "failure_type": FailureType.NOT_FOUND.value,
                "query_attempts": {"$gte": self.max_attempts}
            })

            return {
                "total_count": total,
                "zero_attempts": zero_attempts,
                "in_progress": total - zero_attempts,
                "not_found_failures": not_found_count,
                "service_error_failures": service_error_count,
                "max_attempts_reached": max_attempts_reached
            }
        except PyMongoError as e:
            logger.error(f"Failed to get stats: {e}")
            return {
                "total_count": 0,
                "zero_attempts": 0,
                "in_progress": 0,
                "not_found_failures": 0,
                "service_error_failures": 0,
                "max_attempts_reached": 0
            }