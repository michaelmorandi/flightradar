import logging
from datetime import datetime, timedelta
from typing import Set, List, Tuple
from ...data.repositories.aircraft_repository import AircraftRepository
from ...data.repositories.aircraft_processing_repository import AircraftProcessingRepository, CrawlReason

logger = logging.getLogger("IncompleteAircraftManager")


class IncompleteAircraftManager:
    """
    Manages all aspects of incomplete/unknown aircraft processing:
    1. Aircraft not present in aircraft collection
    2. Aircraft with lastModified date older than staleness threshold
    3. Aircraft with missing critical fields - re-queued sooner than complete aircraft
    4. Repository initialization and configuration
    """

    def __init__(self, config, mongodb=None):
        """Initialize manager with database connections"""
        from ...data.repositories.aircraft_repository import AircraftRepository

        # Get staleness config from config object or use defaults
        self.staleness_days = getattr(config, 'CRAWLER_STALENESS_DAYS', 120)
        self.incomplete_staleness_days = getattr(config, 'CRAWLER_INCOMPLETE_STALENESS_DAYS', 7)
        max_attempts = getattr(config, 'CRAWLER_MAX_ATTEMPTS', 5)
        service_error_reset_hours = getattr(config, 'CRAWLER_SERVICE_ERROR_RESET_HOURS', 6)

        self.aircraft_repo = AircraftRepository(mongodb)
        self.processing_aircraft_repo = AircraftProcessingRepository(
            mongodb,
            max_attempts=max_attempts,
            service_error_reset_hours=service_error_reset_hours
        )

    @classmethod
    def create_with_repositories(cls, aircraft_repo: AircraftRepository,
                                  processing_aircraft_repo: AircraftProcessingRepository,
                                  staleness_days: int = 120,
                                  incomplete_staleness_days: int = 7):
        """Create manager with existing repositories (for testing)"""
        instance = cls.__new__(cls)
        instance.aircraft_repo = aircraft_repo
        instance.processing_aircraft_repo = processing_aircraft_repo
        instance.staleness_days = staleness_days
        instance.incomplete_staleness_days = incomplete_staleness_days
        return instance

    def schedule_aircraft_for_processing(self, icao24s: Set[str]) -> None:
        """
        Process a batch of aircraft and schedule them for metadata processing if they meet criteria.

        Args:
            icao24s: Set of ICAO24 addresses to process
        """
        if not icao24s:
            return

        aircraft_to_process = self._classify_unknown_aircraft(icao24s)

        if aircraft_to_process:
            # Only add aircraft that don't already exist in unknown collection
            new_unknown_aircraft = [
                (icao24, reason) for icao24, reason in aircraft_to_process
                if not self.processing_aircraft_repo.aircraft_exists(icao24)
            ]

            if new_unknown_aircraft:
                logger.info(f"Adding {len(new_unknown_aircraft)} new aircraft (found {len(aircraft_to_process)} total requiring metadata)")
                for icao24, reason in new_unknown_aircraft:
                    self.processing_aircraft_repo.add_aircraft(icao24, reason)
            else:
                logger.debug(f"All {len(aircraft_to_process)} aircraft requiring metadata already exist in unknown collection")

    def _classify_unknown_aircraft(self, icao24s: Set[str]) -> List[Tuple[str, CrawlReason]]:
        """
        Classify aircraft as unknown based on staleness and completeness criteria.

        Aircraft are queued for processing if:
        1. Aircraft not present in aircraft collection
        2. Aircraft has lastModified date older than staleness threshold (configurable)
        3. Aircraft has incomplete data AND lastModified older than incomplete_staleness threshold

        This allows incomplete aircraft to be re-queued sooner than complete aircraft,
        giving external services another chance to provide full data.

        Args:
            icao24s: Set of ICAO24 addresses to classify

        Returns:
            List of tuples (icao24, crawl_reason) for aircraft that need processing
        """
        aircraft_to_process: List[Tuple[str, CrawlReason]] = []
        staleness_threshold = datetime.now() - timedelta(days=self.staleness_days)
        incomplete_staleness_threshold = datetime.now() - timedelta(days=self.incomplete_staleness_days)

        for icao24 in icao24s:
            try:
                # Skip if already in processing queue (being crawled)
                if self.processing_aircraft_repo.aircraft_exists(icao24):
                    continue

                existing_aircraft = self.aircraft_repo.query_aircraft(icao24)

                if existing_aircraft is None:
                    # Criteria 1: Aircraft not present in aircraft collection
                    logger.debug(f"Aircraft {icao24} not found in database")
                    aircraft_to_process.append((icao24, CrawlReason.NOT_IN_DB))
                    continue

                aircraft_doc = self.aircraft_repo.db[self.aircraft_repo.collection_name].find_one(
                    {"modeS": icao24.strip().upper()}
                )

                if aircraft_doc:
                    last_modified = aircraft_doc.get("lastModified")
                    is_incomplete = self._has_missing_critical_fields(aircraft_doc)

                    # Check staleness based on completeness
                    if last_modified is None:
                        # No timestamp - always re-queue
                        logger.debug(f"Aircraft {icao24} has no lastModified, queuing")
                        aircraft_to_process.append((icao24, CrawlReason.NO_TIMESTAMP))
                    elif is_incomplete and last_modified < incomplete_staleness_threshold:
                        # Incomplete data - use shorter staleness threshold
                        logger.debug(f"Aircraft {icao24} is incomplete and stale ({self.incomplete_staleness_days}d), queuing")
                        aircraft_to_process.append((icao24, CrawlReason.INCOMPLETE_STALE))
                    elif last_modified < staleness_threshold:
                        # Complete data but old - use longer staleness threshold
                        logger.debug(f"Aircraft {icao24} is stale ({self.staleness_days}d), queuing")
                        aircraft_to_process.append((icao24, CrawlReason.STALE))
                    else:
                        logger.debug(f"Aircraft {icao24} was recently updated, skipping")

            except Exception as e:
                logger.warning(f"Error classifying aircraft {icao24}: {e}")
                aircraft_to_process.append((icao24, CrawlReason.UNKNOWN))

        return aircraft_to_process

    def _has_missing_critical_fields(self, aircraft_doc: dict) -> bool:
        """
        Check if aircraft has missing critical fields
        
        Args:
            aircraft_doc: Aircraft document from database
            
        Returns:
            True if aircraft has missing critical fields, False otherwise
        """
        critical_fields = ["registeredOwners", "type", "icaoTypeCode", "registration"]
        
        for field in critical_fields:
            value = aircraft_doc.get(field)
            if value is None or (isinstance(value, str) and value.strip() == ""):
                return True
                
        return False

    def get_stats(self) -> dict:
        """Get statistics about aircraft processing"""
        return self.processing_aircraft_repo.get_stats()