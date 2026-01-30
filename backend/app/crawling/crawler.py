from ..data.sources.metadata_sources.openskynet import OpenskyNet
from ..data.sources.metadata_sources.hexdb_io import HexdbIo
from ..data.sources.metadata_sources import AircraftMetadataSource
from ..data.sources.metadata_sources.query_result import QueryResult, QueryStatus
from ..data.sources.radar_services.nighthawk_proxy import NighthawkProxy
from ..core.models.aircraft import Aircraft
from ..data.repositories.aircraft_repository import AircraftRepository
from ..data.repositories.aircraft_processing_repository import AircraftProcessingRepository
from ..core.utils.logging import init_logging
from .utils.source_backoff import CircuitBreakerRegistry

import logging
from dataclasses import dataclass
from typing import List, Optional

logger = logging.getLogger("Crawler")


@dataclass
class CrawlResult:
    """Result of crawling metadata sources for a single aircraft"""
    aircraft: Optional[Aircraft] = None
    had_service_error: bool = False  # True if any source had a service error
    all_not_found: bool = True  # True if all sources returned not found
    error_message: Optional[str] = None


class AirplaneCrawler:
    """Aircraft metadata crawler for retrieving and updating aircraft information"""

    def __init__(self, config, mongodb=None) -> None:
        init_logging(config.LOGGING_CONFIG)

        # Get config values with defaults
        max_attempts = getattr(config, 'CRAWLER_MAX_ATTEMPTS', 5)
        service_error_reset_hours = getattr(config, 'CRAWLER_SERVICE_ERROR_RESET_HOURS', 6)
        self.batch_size = getattr(config, 'CRAWLER_BATCH_SIZE', 50)
        circuit_breaker_threshold = getattr(config, 'CRAWLER_CIRCUIT_BREAKER_THRESHOLD', 5)
        circuit_breaker_reset_sec = getattr(config, 'CRAWLER_CIRCUIT_BREAKER_RESET_SEC', 300)

        self.aircraft_repo = AircraftRepository(mongodb)
        self.processing_repo = AircraftProcessingRepository(
            mongodb,
            max_attempts=max_attempts,
            service_error_reset_hours=service_error_reset_hours
        )

        # Circuit breaker registry for all sources
        self.circuit_breakers = CircuitBreakerRegistry(
            failure_threshold=circuit_breaker_threshold,
            reset_seconds=circuit_breaker_reset_sec
        )

        self.sources: List[AircraftMetadataSource] = [
            HexdbIo(),
            OpenskyNet(),
        ]

        if config.NIGHTHAWK_PROXY_URL:
            self.sources.append(NighthawkProxy(base_url=config.NIGHTHAWK_PROXY_URL))

    def _query_aircraft_metadata(self, icao24: str) -> CrawlResult:
        """
        Query metadata sources for aircraft information.

        Queries sources in order until complete data is found.
        If no source provides complete data, returns the best merged
        partial result from all sources.

        Returns a CrawlResult that distinguishes between:
        - Aircraft found (complete or partial)
        - Aircraft not found in any source (permanent failure)
        - Service errors occurred (temporary failure, should retry)
        """
        best_result: Optional[Aircraft] = None
        sources_used: List[str] = []
        had_service_error = False
        all_not_found = True

        for source in self.sources:
            if not source.accept(icao24):
                continue

            source_name = source.name()

            # Check circuit breaker before querying
            if not self.circuit_breakers.is_source_available(source_name):
                logger.debug(f'Skipping {source_name} - circuit breaker open')
                had_service_error = True  # Treat as potential service error
                all_not_found = False
                continue

            try:
                result = source.query_aircraft_with_status(icao24)

                if result.status == QueryStatus.SERVICE_ERROR:
                    # Service error - record failure and continue to next source
                    self.circuit_breakers.record_failure(source_name)
                    had_service_error = True
                    all_not_found = False
                    logger.warning(f'Service error from {source_name} for {icao24}: {result.error_message}')
                    continue

                if result.status == QueryStatus.NOT_FOUND:
                    # Not found in this source - continue to next
                    self.circuit_breakers.record_success(source_name)  # Service is working
                    logger.debug(f'Aircraft {icao24} not found in {source_name}')
                    continue

                # Success or partial data
                self.circuit_breakers.record_success(source_name)
                all_not_found = False
                aircraft = result.aircraft

                if aircraft:
                    if aircraft.is_complete_with_operator():
                        # Found complete data, return immediately
                        logger.info(f'Found complete data for {icao24} from {source_name}')
                        return CrawlResult(aircraft=aircraft, had_service_error=had_service_error, all_not_found=False)

                    # Partial data - merge with best result so far
                    if best_result is None:
                        best_result = aircraft
                        sources_used.append(source_name)
                        logger.debug(f'Partial data for {icao24} from {source_name}')
                    else:
                        if best_result.merge(aircraft):
                            sources_used.append(source_name)
                            logger.debug(f'Merged additional data for {icao24} from {source_name}')

                        # Check if merged result is now complete
                        if best_result.is_complete_with_operator():
                            best_result.source = '+'.join(sources_used)
                            logger.info(f'Merged complete data for {icao24} from {best_result.source}')
                            return CrawlResult(aircraft=best_result, had_service_error=had_service_error, all_not_found=False)

            except Exception as e:
                logger.warning(f'Unexpected error from {source_name} for {icao24}: {e}')
                self.circuit_breakers.record_failure(source_name)
                had_service_error = True
                all_not_found = False

        # Return best partial result (may still be incomplete)
        if best_result and len(sources_used) > 1:
            best_result.source = '+'.join(sources_used)

        if best_result:
            logger.info(f'Returning {"partial" if not best_result.is_complete_with_operator() else "complete"} data for {icao24} from {best_result.source}')

        return CrawlResult(
            aircraft=best_result,
            had_service_error=had_service_error,
            all_not_found=all_not_found and best_result is None
        )

    def crawl_sources(self) -> None:
        """Process aircraft from the collection that need metadata"""

        try:
            # Reset service error attempts that have cooled down
            reset_count = self.processing_repo.reset_service_error_attempts()
            if reset_count > 0:
                logger.info(f"Reset {reset_count} aircraft with expired service errors")

            # Clean up aircraft that have exhausted "not found" retries
            cleanup_count = self.processing_repo.cleanup_failed_aircraft()
            if cleanup_count > 0:
                logger.info(f"Cleaned up {cleanup_count} aircraft with max 'not found' attempts")

            aircraft_to_process = self.processing_repo.get_aircraft_for_processing(limit=self.batch_size)

            if not aircraft_to_process:
                logger.debug("No aircraft to process")
                return

            logger.info(f"Processing {len(aircraft_to_process)} aircraft")

            for icao24 in aircraft_to_process:
                try:
                    crawl_result = self._query_aircraft_metadata(icao24)

                    if crawl_result.aircraft:
                        # Found data (complete or partial) - save it
                        if self.aircraft_repo.insert_aircraft(crawl_result.aircraft):
                            self.processing_repo.remove_aircraft(icao24)
                            logger.info(f"Successfully processed aircraft: {icao24}")
                        else:
                            # Database error - treat as service error (retry later)
                            logger.warning(f"Failed to insert aircraft {icao24} to database")
                            self.processing_repo.record_service_error(icao24, "Database insert failed")
                    elif crawl_result.had_service_error:
                        # Service error occurred - don't increment attempts, just record for retry
                        self.processing_repo.record_service_error(icao24, crawl_result.error_message)
                        logger.debug(f"Service error for {icao24}, will retry after cooldown")
                    elif crawl_result.all_not_found:
                        # Aircraft not found in any source - increment "not found" attempts
                        self.processing_repo.record_not_found(icao24)
                        logger.debug(f"Aircraft {icao24} not found in any source, incremented attempts")
                    else:
                        # Unexpected state - treat as service error to be safe
                        self.processing_repo.record_service_error(icao24, "Unknown crawl state")
                        logger.warning(f"Unexpected crawl state for {icao24}")

                except Exception as e:
                    logger.warning(f"Error processing aircraft {icao24}: {e}")
                    self.processing_repo.record_service_error(icao24, str(e))

        except Exception as e:
            logger.exception(f"Error in crawl_sources: {e}")

    def get_circuit_breaker_stats(self) -> dict:
        """Get statistics for all circuit breakers"""
        return self.circuit_breakers.get_all_stats()