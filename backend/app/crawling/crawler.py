from ..data.sources.metadata_sources.openskynet import OpenskyNet
from ..data.sources.metadata_sources.hexdb_io import HexdbIo
from ..data.sources.metadata_sources import AircraftMetadataSource
from ..data.sources.metadata_sources.query_result import QueryResult, QueryStatus
from ..data.sources.radar_services.nighthawk_sources import get_nighthawk_sources
from ..core.models.aircraft import Aircraft
from ..data.repositories.aircraft_repository import AircraftRepository
from ..data.repositories.aircraft_processing_repository import AircraftProcessingRepository
from ..data.repositories.crawler_log_repository import CrawlerLogRepository
from ..core.utils.logging import init_logging
from .utils.source_backoff import CircuitBreakerRegistry

import logging
import time
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional, Any

logger = logging.getLogger("Crawler")

# Maximum number of activity entries to keep
MAX_ACTIVITY_ENTRIES = 50


@dataclass
class CrawlActivity:
    """Record of a single aircraft fetch attempt"""
    icao24: str
    timestamp: datetime
    status: str  # 'success', 'merged', 'partial', 'not_found', 'service_error'
    source: Optional[str] = None
    registration: Optional[str] = None
    aircraft_type: Optional[str] = None
    crawl_reason: Optional[str] = None  # Why the aircraft was queued for crawling
    query_count: int = 1  # Number of sources queried (logs only saved when >= 2)


@dataclass
class SourceInfo:
    """Information about a metadata source"""
    name: str
    enabled: bool = True


@dataclass
class SourceQueryLog:
    """Log entry for a single source query"""
    source: str
    status: str
    duration_ms: int
    payload: Optional[dict[str, Any]] = None
    error: Optional[str] = None


@dataclass
class CrawlResult:
    """Result of crawling metadata sources for a single aircraft"""
    aircraft: Optional[Aircraft] = None
    had_service_error: bool = False  # True if any source had a service error
    all_not_found: bool = True  # True if all sources returned not found
    error_message: Optional[str] = None
    query_logs: List[SourceQueryLog] = field(default_factory=list)  # Log of all source queries


def _is_sufficient(aircraft: Aircraft) -> bool:
    """
    Check if aircraft has enough data to stop querying additional sources.
    Sufficient means: registration + type_code + (type_description OR operator)
    This allows early stopping when we have 3 of 4 key fields.
    """
    has_reg = aircraft.reg is not None
    has_type_code = aircraft.icao_type_code is not None
    has_type_desc = aircraft.aircraft_type_description is not None
    has_operator = aircraft.operator is not None

    # Must have registration and type code at minimum
    if not has_reg or not has_type_code:
        return False

    # Plus at least one of type_description or operator
    return has_type_desc or has_operator


class AirplaneCrawler:
    """Aircraft metadata crawler for retrieving and updating aircraft information"""

    def __init__(self, config, mongodb=None) -> None:
        init_logging(config.LOGGING_CONFIG)

        # Get config values with defaults
        max_attempts = getattr(config, 'CRAWLER_MAX_ATTEMPTS', 5)
        service_error_reset_hours = getattr(config, 'CRAWLER_SERVICE_ERROR_RESET_HOURS', 6)
        self.batch_size = getattr(config, 'CRAWLER_BATCH_SIZE', 50)
        circuit_breaker_threshold = getattr(config, 'CRAWLER_CIRCUIT_BREAKER_THRESHOLD', 5)
        circuit_breaker_base_reset_sec = getattr(config, 'CRAWLER_CIRCUIT_BREAKER_BASE_RESET_SEC', 60)
        circuit_breaker_max_reset_sec = getattr(config, 'CRAWLER_CIRCUIT_BREAKER_MAX_RESET_SEC', 1800)

        self.aircraft_repo = AircraftRepository(mongodb)
        self.processing_repo = AircraftProcessingRepository(
            mongodb,
            max_attempts=max_attempts,
            service_error_reset_hours=service_error_reset_hours
        )
        self.log_repo = CrawlerLogRepository(mongodb)

        # Circuit breaker registry for all sources (with exponential backoff)
        self.circuit_breakers = CircuitBreakerRegistry(
            failure_threshold=circuit_breaker_threshold,
            base_reset_seconds=circuit_breaker_base_reset_sec,
            max_reset_seconds=circuit_breaker_max_reset_sec
        )

        self.sources: List[AircraftMetadataSource] = [
            HexdbIo(),
            OpenskyNet(),
        ]

        # Add nighthawk sources if configured (discovered from proxy, sorted by priority)
        if config.NIGHTHAWK_PROXY_URL:
            nighthawk_sources = get_nighthawk_sources(base_url=config.NIGHTHAWK_PROXY_URL)
            self.sources.extend(nighthawk_sources)
            logger.info(f"Added {len(nighthawk_sources)} nighthawk sources: {[s.name() for s in nighthawk_sources]}")

        # Activity tracking for admin dashboard
        self._activity_log: deque[CrawlActivity] = deque(maxlen=MAX_ACTIVITY_ENTRIES)

        # Volatile source enabled state (resets on restart)
        self._source_enabled: dict[str, bool] = {source.name(): True for source in self.sources}

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
        any_not_found = False  # Track if any source confirmed "not found"
        any_source_queried = False  # Track if we actually queried any source
        query_logs: List[SourceQueryLog] = []

        for source in self.sources:
            if not source.accept(icao24):
                continue

            source_name = source.name()

            # Check if source is enabled (volatile admin toggle)
            if not self._source_enabled.get(source_name, True):
                logger.debug(f'Skipping {source_name} - disabled by admin')
                continue

            # Check circuit breaker before querying
            if not self.circuit_breakers.is_source_available(source_name):
                logger.debug(f'Skipping {source_name} - circuit breaker open')
                # Don't set had_service_error here - circuit breaker skips shouldn't
                # override a "not found" response from sources that were actually queried
                query_logs.append(SourceQueryLog(
                    source=source_name,
                    status='skipped_circuit_breaker',
                    duration_ms=0,
                ))
                continue

            try:
                start_time = time.time()
                result = source.query_aircraft_with_status(icao24)
                duration_ms = int((time.time() - start_time) * 1000)

                # Create query log entry
                query_log = SourceQueryLog(
                    source=source_name,
                    status=result.status.value,
                    duration_ms=duration_ms,
                    payload=result.raw_payload,
                    error=result.error_message,
                )
                query_logs.append(query_log)

                any_source_queried = True

                if result.status == QueryStatus.SERVICE_ERROR:
                    # Service error - record failure and continue to next source
                    self.circuit_breakers.record_failure(source_name)
                    had_service_error = True
                    logger.warning(f'Service error from {source_name} for {icao24}: {result.error_message}')
                    continue

                if result.status == QueryStatus.NOT_FOUND:
                    # Not found in this source - this is a definitive answer from a working source
                    self.circuit_breakers.record_success(source_name)  # Service is working
                    any_not_found = True  # At least one source confirmed "not found"
                    logger.debug(f'Aircraft {icao24} not found in {source_name}')
                    continue

                # Success or partial data
                self.circuit_breakers.record_success(source_name)
                aircraft = result.aircraft

                if aircraft:
                    if aircraft.is_complete_with_operator():
                        # Found complete data, return immediately
                        logger.info(f'Found complete data for {icao24} from {source_name}')
                        return CrawlResult(
                            aircraft=aircraft,
                            had_service_error=had_service_error,
                            all_not_found=False,
                            query_logs=query_logs
                        )

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
                        return CrawlResult(
                            aircraft=best_result,
                            had_service_error=had_service_error,
                            all_not_found=False,
                            query_logs=query_logs
                        )

                    # Early stop if we have sufficient data (3 of 4 key fields)
                    # This avoids unnecessary calls to remaining sources
                    if _is_sufficient(best_result):
                        best_result.source = '+'.join(sources_used)
                        logger.info(f'Sufficient data for {icao24} from {best_result.source}, stopping early')
                        return CrawlResult(
                            aircraft=best_result,
                            had_service_error=had_service_error,
                            all_not_found=False,
                            query_logs=query_logs
                        )

            except Exception as e:
                logger.warning(f'Unexpected error from {source_name} for {icao24}: {e}')
                self.circuit_breakers.record_failure(source_name)
                had_service_error = True
                any_source_queried = True
                query_logs.append(SourceQueryLog(
                    source=source_name,
                    status='exception',
                    duration_ms=0,
                    error=str(e),
                ))

        # Return best partial result (may still be incomplete)
        if best_result and len(sources_used) > 1:
            best_result.source = '+'.join(sources_used)

        if best_result:
            logger.info(f'Returning {"partial" if not best_result.is_complete_with_operator() else "complete"} data for {icao24} from {best_result.source}')

        # Determine final status:
        # - If we have data, return it (not_found=False, service_error=False)
        # - If any source confirmed "not found", treat as permanent failure
        # - Only if NO source confirmed "not found" AND we had service errors, retry later
        # - If no sources were queried at all (all circuit breakers open), treat as service error
        is_not_found = any_not_found and best_result is None
        is_service_error = (
            not is_not_found and  # "not found" takes priority over service errors
            (had_service_error or not any_source_queried) and  # actual errors or all skipped
            best_result is None
        )

        return CrawlResult(
            aircraft=best_result,
            had_service_error=is_service_error,
            all_not_found=is_not_found,
            query_logs=query_logs
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
                    # Get crawl reason before processing (it will be removed on success)
                    crawl_reason = self.processing_repo.get_crawl_reason(icao24)

                    crawl_result = self._query_aircraft_metadata(icao24)

                    query_count = len(crawl_result.query_logs)

                    if crawl_result.aircraft:
                        # Found data (complete or partial) - save it
                        if self.aircraft_repo.insert_aircraft(crawl_result.aircraft):
                            self.processing_repo.remove_aircraft(icao24)
                            if crawl_result.aircraft.is_complete_with_operator():
                                status = 'success'
                            elif '+' in (crawl_result.aircraft.source or ''):
                                status = 'merged'
                            else:
                                status = 'partial'
                            self._record_activity(icao24, status, crawl_result.aircraft, crawl_reason, query_count)
                            logger.info(f"Successfully processed aircraft: {icao24}")
                        else:
                            # Database error - treat as service error (retry later)
                            logger.warning(f"Failed to insert aircraft {icao24} to database")
                            self.processing_repo.record_service_error(icao24, "Database insert failed")
                            self._record_activity(icao24, 'service_error', crawl_reason=crawl_reason, query_count=query_count)
                            status = 'service_error'
                    elif crawl_result.had_service_error:
                        # Service error occurred - don't increment attempts, just record for retry
                        self.processing_repo.record_service_error(icao24, crawl_result.error_message)
                        self._record_activity(icao24, 'service_error', crawl_reason=crawl_reason, query_count=query_count)
                        logger.debug(f"Service error for {icao24}, will retry after cooldown")
                        status = 'service_error'
                    elif crawl_result.all_not_found:
                        # Aircraft not found in any source - increment "not found" attempts
                        self.processing_repo.record_not_found(icao24)
                        self._record_activity(icao24, 'not_found', crawl_reason=crawl_reason, query_count=query_count)
                        logger.debug(f"Aircraft {icao24} not found in any source, incremented attempts")
                        status = 'not_found'
                    else:
                        # Unexpected state - treat as service error to be safe
                        self.processing_repo.record_service_error(icao24, "Unknown crawl state")
                        self._record_activity(icao24, 'service_error', crawl_reason=crawl_reason, query_count=query_count)
                        logger.warning(f"Unexpected crawl state for {icao24}")
                        status = 'service_error'

                    # Save query log if multiple sources were queried
                    if len(crawl_result.query_logs) >= 2:
                        self._save_query_log(icao24, crawl_result, status)

                except Exception as e:
                    logger.warning(f"Error processing aircraft {icao24}: {e}")
                    self.processing_repo.record_service_error(icao24, str(e))

        except Exception as e:
            logger.exception(f"Error in crawl_sources: {e}")

    def get_circuit_breaker_stats(self) -> dict:
        """Get statistics for all circuit breakers"""
        return self.circuit_breakers.get_all_stats()

    def _record_activity(self, icao24: str, status: str, aircraft: Optional[Aircraft] = None,
                         crawl_reason: Optional[str] = None, query_count: int = 1) -> None:
        """Record a crawl activity for the admin dashboard"""
        activity = CrawlActivity(
            icao24=icao24,
            timestamp=datetime.utcnow(),
            status=status,
            source=aircraft.source if aircraft else None,
            registration=aircraft.reg if aircraft else None,
            aircraft_type=aircraft.icao_type_code if aircraft else None,
            crawl_reason=crawl_reason,
            query_count=query_count,
        )
        self._activity_log.appendleft(activity)

    def _save_query_log(self, icao24: str, crawl_result: CrawlResult, final_status: str) -> None:
        """Save query log to MongoDB for multi-source queries"""
        try:
            queries = [
                {
                    'source': log.source,
                    'status': log.status,
                    'duration_ms': log.duration_ms,
                    'payload': log.payload,
                    'error': log.error,
                }
                for log in crawl_result.query_logs
            ]

            final_source = crawl_result.aircraft.source if crawl_result.aircraft else None

            self.log_repo.save_query_log(
                icao24=icao24,
                queries=queries,
                final_status=final_status,
                final_source=final_source,
            )
            logger.debug(f"Saved query log for {icao24} with {len(queries)} source queries")
        except Exception as e:
            logger.warning(f"Failed to save query log for {icao24}: {e}")

    def get_recent_activity(self, limit: int = 20) -> List[dict]:
        """Get recent crawl activity for the admin dashboard"""
        activities = list(self._activity_log)[:limit]
        return [
            {
                'icao24': a.icao24,
                'timestamp': a.timestamp.isoformat(),
                'status': a.status,
                'source': a.source,
                'registration': a.registration,
                'aircraft_type': a.aircraft_type,
                'crawl_reason': a.crawl_reason,
                'query_count': a.query_count,
            }
            for a in activities
        ]

    def get_sources_status(self) -> List[dict]:
        """Get list of all sources with their enabled state (volatile)"""
        return [
            {
                'name': source.name(),
                'enabled': self._source_enabled.get(source.name(), True),
            }
            for source in self.sources
        ]

    def set_source_enabled(self, source_name: str, enabled: bool) -> bool:
        """
        Set the enabled state of a source (volatile, resets on restart).
        Returns True if the source was found and updated, False otherwise.
        """
        # Check if source exists
        source_names = [s.name() for s in self.sources]
        if source_name not in source_names:
            return False
        self._source_enabled[source_name] = enabled
        logger.info(f'Source {source_name} {"enabled" if enabled else "disabled"} by admin')
        return True