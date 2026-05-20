//! `FlightUpdater` use case.
//!
//! Consumes a `Stream<PositionReport>` from a `RadarSource`. Two seams:
//!
//! - [`ingest`](FlightUpdater::ingest) — fast, in-memory: drop into a
//!   pending map keyed by ICAO24. The latest report for an aircraft wins.
//! - [`flush`](FlightUpdater::flush) — slow, async: upsert flights, batch
//!   persist positions, prune stale entries, compute delta vs. last
//!   snapshot, publish to the event bus, atomically swap the live state.
//!
//! [`run`](FlightUpdater::run) is a convenience that drives those two from
//! a `Stream` + a tick interval via `tokio::select!`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use time::OffsetDateTime;
use tracing::{debug, warn};

use flightradar_domain::policy::modes::ModeSClassifier;
use flightradar_domain::ports::clock::Clock;
use flightradar_domain::ports::event_bus::{PositionEvent, PositionEventBus};
use flightradar_domain::ports::radar_source::{PositionStream, RadarSource};
use flightradar_domain::ports::repositories::{
    CrawlerQueueRepository, FlightRepository, PositionRepository,
};
use flightradar_domain::{Flight, FlightId, Icao24, LivePosition, LiveSnapshot, PositionReport};

use crate::error::ApplicationError;
use crate::live_state::LiveState;

#[derive(Debug, Clone, Copy)]
pub struct FlightUpdaterConfig {
    /// If a position arrives for an aircraft we have not seen for at least
    /// this many seconds, close the existing open flight and start a new
    /// one. Matches the "flight ends after silence" heuristic in the
    /// Python implementation.
    pub flight_gap_seconds: i64,

    /// Drop live-state entries that have not been refreshed in this many
    /// seconds. Required for streaming sources, where "disappearance" is
    /// implicit (no more events arrive) rather than explicit (poll returns
    /// fewer aircraft).
    pub position_ttl_seconds: i64,

    /// Filter out civilian aircraft entirely if `true`. Mirrors the
    /// `MIL_ONLY` env var.
    pub military_only: bool,

    /// Enqueue ICAO24s seen on the stream for crawler enrichment.
    pub enqueue_unknown_aircraft: bool,
}

impl Default for FlightUpdaterConfig {
    fn default() -> Self {
        Self {
            flight_gap_seconds: 60 * 60, // 1 h
            position_ttl_seconds: 60,
            military_only: false,
            enqueue_unknown_aircraft: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlightUpdaterTickReport {
    pub pending_processed: usize,
    pub flights_created: usize,
    pub flights_updated: usize,
    pub positions_persisted: usize,
    pub delta_size: usize,
    pub removed_aircraft: usize,
    pub unknown_aircraft_enqueued: usize,
}

#[derive(Debug)]
pub struct FlightUpdater {
    radar: Arc<dyn RadarSource>,
    flight_repo: Arc<dyn FlightRepository>,
    position_repo: Arc<dyn PositionRepository>,
    crawler_queue: Arc<dyn CrawlerQueueRepository>,
    event_bus: Arc<dyn PositionEventBus>,
    live_state: LiveState,
    classifier: Arc<ModeSClassifier>,
    clock: Arc<dyn Clock>,
    config: FlightUpdaterConfig,
    /// Reports accumulated since the last flush, keyed by ICAO24. The
    /// latest sighting for an aircraft replaces earlier ones.
    pending: Mutex<HashMap<Icao24, PositionReport>>,
}

impl FlightUpdater {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radar: Arc<dyn RadarSource>,
        flight_repo: Arc<dyn FlightRepository>,
        position_repo: Arc<dyn PositionRepository>,
        crawler_queue: Arc<dyn CrawlerQueueRepository>,
        event_bus: Arc<dyn PositionEventBus>,
        live_state: LiveState,
        classifier: Arc<ModeSClassifier>,
        clock: Arc<dyn Clock>,
        config: FlightUpdaterConfig,
    ) -> Self {
        Self {
            radar,
            flight_repo,
            position_repo,
            crawler_queue,
            event_bus,
            live_state,
            classifier,
            clock,
            config,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn live_state(&self) -> LiveState {
        self.live_state.clone()
    }

    /// Apply a single position observation. Fast, lock-bound, no I/O. The
    /// `military_only` filter is applied here so dropped reports never
    /// reach the flush path.
    pub fn ingest(&self, report: PositionReport) {
        if self.config.military_only && !self.classifier.is_military(&report.icao24) {
            return;
        }
        self.pending
            .lock()
            .expect("pending mutex poisoned")
            .insert(report.icao24.clone(), report);
    }

    /// Flush all pending reports. Upserts flights, persists positions,
    /// prunes stale entries, publishes the delta, atomically swaps the
    /// live snapshot.
    pub async fn flush(
        &self,
        now: OffsetDateTime,
    ) -> Result<FlightUpdaterTickReport, ApplicationError> {
        let pending = std::mem::take(&mut *self.pending.lock().expect("pending mutex poisoned"));
        let mut report = FlightUpdaterTickReport {
            pending_processed: pending.len(),
            ..Default::default()
        };

        let prev_snapshot = self.live_state.read();
        let mut next_positions: HashMap<Icao24, LivePosition> = prev_snapshot.positions().clone();
        let mut to_persist: Vec<(FlightId, PositionReport)> = Vec::with_capacity(pending.len());

        for (icao, pr) in pending {
            let flight_id = match self.upsert_flight_for(&pr, now).await {
                Ok((id, created)) => {
                    if created {
                        report.flights_created += 1;
                    } else {
                        report.flights_updated += 1;
                    }
                    id
                }
                Err(err) => {
                    warn!(error = %err, %icao, "flight upsert failed");
                    continue;
                }
            };

            next_positions.insert(icao, to_live_position(&pr));
            to_persist.push((flight_id, pr));
        }

        // Prune stale entries that we have not seen recently.
        let ttl = time::Duration::seconds(self.config.position_ttl_seconds);
        next_positions.retain(|_, lp| now - lp.updated_at <= ttl);

        if !to_persist.is_empty() {
            match self.position_repo.append_batch(&to_persist).await {
                Ok(()) => report.positions_persisted = to_persist.len(),
                Err(err) => warn!(error = %err, "position batch append failed"),
            }
        }

        let next_snapshot = LiveSnapshot::new(next_positions, now);
        let changed = next_snapshot.delta_against(&prev_snapshot);
        let removed = next_snapshot.removed_since(&prev_snapshot);
        report.delta_size = changed.len();
        report.removed_aircraft = removed.len();

        if !changed.is_empty() || !removed.is_empty() {
            self.event_bus.publish(PositionEvent::Delta {
                changed,
                removed,
                emitted_at: now,
            });
        }
        self.live_state.publish(next_snapshot);

        if self.config.enqueue_unknown_aircraft {
            for (_, pr) in &to_persist {
                if let Err(err) = self.crawler_queue.enqueue(&pr.icao24).await {
                    debug!(error = %err, icao24 = %pr.icao24, "crawler enqueue failed");
                } else {
                    report.unknown_aircraft_enqueued += 1;
                }
            }
        }

        Ok(report)
    }

    /// Drive ingestion and periodic flushes until the radar stream ends.
    /// The server crate wraps this in a supervisor that restarts on exit.
    pub async fn run(&self, flush_interval: Duration) -> Result<(), ApplicationError> {
        let stream = self.radar.stream().await?;
        self.run_with_stream(stream, flush_interval).await
    }

    /// Test seam: same as [`run`] but takes the stream as an argument so
    /// tests can inject `futures::stream::iter(...)`.
    pub async fn run_with_stream(
        &self,
        mut stream: PositionStream,
        flush_interval: Duration,
    ) -> Result<(), ApplicationError> {
        let mut ticker = tokio::time::interval(flush_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // First tick fires immediately; skip it so we don't flush an empty pending set.
        ticker.tick().await;

        loop {
            tokio::select! {
                next = stream.next() => if let Some(report) = next {
                    self.ingest(report);
                } else {
                    // Stream ended — drain pending and return.
                    let _ = self.flush(self.clock.now()).await;
                    return Ok(());
                },
                _ = ticker.tick() => match self.flush(self.clock.now()).await {
                    Ok(r) => debug!(?r, "tick flushed"),
                    Err(err) => warn!(error = %err, "flush failed"),
                },
            }
        }
    }

    async fn upsert_flight_for(
        &self,
        pr: &PositionReport,
        now: OffsetDateTime,
    ) -> Result<(FlightId, bool), ApplicationError> {
        let existing = self
            .flight_repo
            .find_open_for_icao24(&pr.icao24)
            .await
            .map_err(ApplicationError::from)?;

        if let Some(mut flight) = existing {
            let gap = (now - flight.last_contact).whole_seconds();
            if gap >= self.config.flight_gap_seconds {
                let new = build_new_flight(pr, now, &self.classifier);
                self.flight_repo.upsert(&new).await?;
                return Ok((new.id, true));
            }
            flight.last_contact = now;
            if let Some(callsign) = pr.callsign.clone() {
                if flight.callsign.as_ref() != Some(&callsign) {
                    flight.airline_icao =
                        flightradar_domain::policy::callsign::extract_airline_icao(&callsign);
                    flight.callsign = Some(callsign);
                }
            }
            self.flight_repo.upsert(&flight).await?;
            Ok((flight.id, false))
        } else {
            let new = build_new_flight(pr, now, &self.classifier);
            self.flight_repo.upsert(&new).await?;
            Ok((new.id, true))
        }
    }
}

fn build_new_flight(
    pr: &PositionReport,
    now: OffsetDateTime,
    classifier: &ModeSClassifier,
) -> Flight {
    let airline_icao = pr
        .callsign
        .as_ref()
        .and_then(flightradar_domain::policy::callsign::extract_airline_icao);
    Flight {
        id: FlightId::new(format!("{}-{}", pr.icao24, now.unix_timestamp())),
        icao24: pr.icao24.clone(),
        callsign: pr.callsign.clone(),
        airline_icao,
        is_military: classifier.is_military(&pr.icao24),
        first_contact: now,
        last_contact: now,
    }
}

fn to_live_position(pr: &PositionReport) -> LivePosition {
    LivePosition {
        lat: pr.latitude,
        lon: pr.longitude,
        alt_ft: pr.altitude_ft,
        ground_speed_kt: pr.ground_speed_kt,
        track_deg: pr.track_deg,
        callsign: pr.callsign.clone(),
        category: pr.category,
        updated_at: pr.observed_at,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)] // intentional: positions are bit-identical, see live_snapshot::LivePosition::differs_from
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use futures::stream;

    use flightradar_domain::ports::clock::Clock;
    use flightradar_domain::ports::event_bus::EventStream;
    use flightradar_domain::ports::radar_source::RadarError;
    use flightradar_domain::ports::repositories::{
        CrawlerQueueEntry, FlightFilter, Page, PageRequest, RepoResult, RepositoryError,
    };
    use flightradar_domain::{Callsign, PositionReport};

    use super::*;

    // -- Clock ----------------------------------------------------------

    #[derive(Debug)]
    struct FixedClock(OffsetDateTime);
    impl Clock for FixedClock {
        fn now(&self) -> OffsetDateTime {
            self.0
        }
    }

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    // -- Radar source --------------------------------------------------

    struct StreamRadar(StdMutex<Option<PositionStream>>);

    impl std::fmt::Debug for StreamRadar {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("StreamRadar").finish_non_exhaustive()
        }
    }

    impl StreamRadar {
        fn empty() -> Self {
            Self(StdMutex::new(Some(Box::pin(stream::iter(Vec::<
                PositionReport,
            >::new(
            ))))))
        }
    }

    #[async_trait]
    impl RadarSource for StreamRadar {
        fn name(&self) -> &'static str {
            "stream-fake"
        }
        async fn stream(&self) -> Result<PositionStream, RadarError> {
            self.0
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| RadarError::Unavailable("stream consumed".into()))
        }
    }

    // -- Repositories --------------------------------------------------

    #[derive(Debug, Default)]
    struct InMemFlightRepo {
        by_icao: StdMutex<HashMap<String, Flight>>,
        upserts: StdMutex<usize>,
    }
    #[async_trait]
    impl FlightRepository for InMemFlightRepo {
        async fn upsert(&self, flight: &Flight) -> RepoResult<()> {
            self.by_icao
                .lock()
                .unwrap()
                .insert(flight.icao24.to_string(), flight.clone());
            *self.upserts.lock().unwrap() += 1;
            Ok(())
        }
        async fn find_by_id(&self, _id: &FlightId) -> RepoResult<Flight> {
            Err(RepositoryError::NotFound)
        }
        async fn find_open_for_icao24(&self, icao24: &Icao24) -> RepoResult<Option<Flight>> {
            Ok(self
                .by_icao
                .lock()
                .unwrap()
                .get(&icao24.to_string())
                .cloned())
        }
        async fn list(
            &self,
            _filter: &FlightFilter,
            _page: PageRequest,
        ) -> RepoResult<Page<Flight>> {
            unimplemented!()
        }
    }

    #[derive(Debug, Default)]
    struct InMemPositionRepo {
        appended: StdMutex<Vec<(FlightId, PositionReport)>>,
    }
    #[async_trait]
    impl PositionRepository for InMemPositionRepo {
        async fn append(&self, _id: &FlightId, _p: &PositionReport) -> RepoResult<()> {
            Ok(())
        }
        async fn append_batch(&self, entries: &[(FlightId, PositionReport)]) -> RepoResult<()> {
            self.appended
                .lock()
                .unwrap()
                .extend(entries.iter().cloned());
            Ok(())
        }
        async fn history(&self, _id: &FlightId) -> RepoResult<Vec<PositionReport>> {
            unimplemented!()
        }
    }

    #[derive(Debug, Default)]
    struct InMemCrawlerQueue {
        enqueued: StdMutex<Vec<Icao24>>,
    }
    #[async_trait]
    impl CrawlerQueueRepository for InMemCrawlerQueue {
        async fn enqueue(&self, icao24: &Icao24) -> RepoResult<()> {
            self.enqueued.lock().unwrap().push(icao24.clone());
            Ok(())
        }
        async fn next_batch(&self, _n: u32) -> RepoResult<Vec<CrawlerQueueEntry>> {
            unimplemented!()
        }
        async fn record_attempt(&self, _icao24: &Icao24, _success: bool) -> RepoResult<()> {
            Ok(())
        }
    }

    // -- Event bus -----------------------------------------------------

    #[derive(Debug, Default)]
    struct CapturingBus {
        events: StdMutex<Vec<PositionEvent>>,
    }
    impl PositionEventBus for CapturingBus {
        fn publish(&self, event: PositionEvent) {
            self.events.lock().unwrap().push(event);
        }
        fn subscribe(&self) -> EventStream {
            unimplemented!()
        }
    }

    // -- Helpers -------------------------------------------------------

    fn pr(icao: &str, lat: f64, lon: f64, observed: OffsetDateTime) -> PositionReport {
        PositionReport::new(Icao24::new(icao).unwrap(), lat, lon, observed).unwrap()
    }

    fn pr_with_callsign(
        icao: &str,
        lat: f64,
        lon: f64,
        callsign: &str,
        observed: OffsetDateTime,
    ) -> PositionReport {
        let mut p = pr(icao, lat, lon, observed);
        p.callsign = Some(Callsign::new(callsign).unwrap());
        p
    }

    struct Harness {
        updater: FlightUpdater,
        flights: Arc<InMemFlightRepo>,
        positions: Arc<InMemPositionRepo>,
        queue: Arc<InMemCrawlerQueue>,
        bus: Arc<CapturingBus>,
        live: LiveState,
    }

    fn build(clock_t: OffsetDateTime, config: FlightUpdaterConfig) -> Harness {
        let radar = Arc::new(StreamRadar::empty());
        let flights = Arc::new(InMemFlightRepo::default());
        let positions = Arc::new(InMemPositionRepo::default());
        let queue = Arc::new(InMemCrawlerQueue::default());
        let bus = Arc::new(CapturingBus::default());
        let live = LiveState::empty();
        let classifier = Arc::new(ModeSClassifier::from_hex_pairs(vec![("AE0000", "AFFFFF")]));
        let updater = FlightUpdater::new(
            radar,
            flights.clone(),
            positions.clone(),
            queue.clone(),
            bus.clone(),
            live.clone(),
            classifier,
            Arc::new(FixedClock(clock_t)),
            config,
        );
        Harness {
            updater,
            flights,
            positions,
            queue,
            bus,
            live,
        }
    }

    // -- Tests ---------------------------------------------------------

    #[tokio::test]
    async fn flush_with_no_pending_publishes_nothing() {
        let h = build(t(0), FlightUpdaterConfig::default());
        let report = h.updater.flush(t(0)).await.unwrap();
        assert_eq!(report.pending_processed, 0);
        assert_eq!(report.delta_size, 0);
        assert!(h.bus.events.lock().unwrap().is_empty());
        assert!(h.live.read().is_empty());
    }

    #[tokio::test]
    async fn first_position_creates_flight_and_publishes_delta() {
        let h = build(t(0), FlightUpdaterConfig::default());
        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));

        let report = h.updater.flush(t(0)).await.unwrap();
        assert_eq!(report.pending_processed, 1);
        assert_eq!(report.flights_created, 1);
        assert_eq!(report.positions_persisted, 1);
        assert_eq!(report.delta_size, 1);
        assert_eq!(report.unknown_aircraft_enqueued, 1);

        assert_eq!(*h.flights.upserts.lock().unwrap(), 1);
        assert_eq!(h.positions.appended.lock().unwrap().len(), 1);
        assert_eq!(h.queue.enqueued.lock().unwrap().len(), 1);
        assert_eq!(h.bus.events.lock().unwrap().len(), 1);
        assert_eq!(h.live.read().len(), 1);
    }

    #[tokio::test]
    async fn second_position_updates_existing_flight() {
        let h = build(t(0), FlightUpdaterConfig::default());
        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        h.updater.flush(t(0)).await.unwrap();

        h.updater.ingest(pr("ABCDEF", 1.0, 3.0, t(10)));
        let report = h.updater.flush(t(10)).await.unwrap();
        assert_eq!(report.flights_created, 0);
        assert_eq!(report.flights_updated, 1);
        assert_eq!(report.delta_size, 1);
        assert_eq!(
            h.live
                .read()
                .get(&Icao24::new("ABCDEF").unwrap())
                .unwrap()
                .lon,
            3.0
        );
    }

    #[tokio::test]
    async fn long_gap_starts_new_flight() {
        let cfg = FlightUpdaterConfig {
            flight_gap_seconds: 600,
            ..Default::default()
        };
        let h = build(t(0), cfg);

        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        let r1 = h.updater.flush(t(0)).await.unwrap();
        assert_eq!(r1.flights_created, 1);

        h.updater.ingest(pr("ABCDEF", 1.0, 3.0, t(601)));
        let r2 = h.updater.flush(t(601)).await.unwrap();
        assert_eq!(r2.flights_created, 1);
        assert_eq!(r2.flights_updated, 0);
    }

    #[tokio::test]
    async fn military_only_filters_at_ingest() {
        let cfg = FlightUpdaterConfig {
            military_only: true,
            ..Default::default()
        };
        let h = build(t(0), cfg);
        h.updater.ingest(pr("AE0001", 1.0, 2.0, t(0))); // military per fixture
        h.updater.ingest(pr("ABCDEF", 3.0, 4.0, t(0))); // civilian — dropped

        let report = h.updater.flush(t(0)).await.unwrap();
        assert_eq!(report.pending_processed, 1);
        assert_eq!(report.flights_created, 1);
    }

    #[tokio::test]
    async fn callsign_change_extracts_airline_icao() {
        let h = build(t(0), FlightUpdaterConfig::default());
        h.updater
            .ingest(pr_with_callsign("ABCDEF", 1.0, 2.0, "AFR990", t(0)));
        h.updater.flush(t(0)).await.unwrap();

        let stored = h
            .flights
            .by_icao
            .lock()
            .unwrap()
            .get("ABCDEF")
            .cloned()
            .unwrap();
        assert_eq!(stored.airline_icao.unwrap().as_str(), "AFR");
    }

    #[tokio::test]
    async fn unchanged_position_no_delta() {
        let h = build(t(0), FlightUpdaterConfig::default());
        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        h.updater.flush(t(0)).await.unwrap();

        let before = h.bus.events.lock().unwrap().len();
        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        let report = h.updater.flush(t(0)).await.unwrap();
        assert_eq!(report.delta_size, 0);
        assert_eq!(h.bus.events.lock().unwrap().len(), before);
    }

    #[tokio::test]
    async fn stale_entries_pruned_by_ttl() {
        let cfg = FlightUpdaterConfig {
            position_ttl_seconds: 30,
            ..Default::default()
        };
        let h = build(t(0), cfg);

        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        h.updater.flush(t(0)).await.unwrap();
        assert_eq!(h.live.read().len(), 1);

        // Advance past TTL with no new observation.
        let report = h.updater.flush(t(31)).await.unwrap();
        assert_eq!(report.removed_aircraft, 1);
        assert!(h.live.read().is_empty());

        let last = h.bus.events.lock().unwrap().last().cloned().unwrap();
        match last {
            PositionEvent::Delta { removed, .. } => {
                assert_eq!(removed, vec![Icao24::new("ABCDEF").unwrap()]);
            }
            other @ PositionEvent::Snapshot { .. } => panic!("expected Delta, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn latest_pending_wins_per_icao() {
        let h = build(t(0), FlightUpdaterConfig::default());
        // Three observations for the same aircraft within a single flush window.
        h.updater.ingest(pr("ABCDEF", 1.0, 1.0, t(0)));
        h.updater.ingest(pr("ABCDEF", 2.0, 2.0, t(1)));
        h.updater.ingest(pr("ABCDEF", 3.0, 3.0, t(2)));

        let report = h.updater.flush(t(2)).await.unwrap();
        assert_eq!(report.pending_processed, 1);
        assert_eq!(
            h.live
                .read()
                .get(&Icao24::new("ABCDEF").unwrap())
                .unwrap()
                .lat,
            3.0
        );
    }

    #[tokio::test]
    async fn config_disable_crawler_enqueue() {
        let cfg = FlightUpdaterConfig {
            enqueue_unknown_aircraft: false,
            ..Default::default()
        };
        let h = build(t(0), cfg);
        h.updater.ingest(pr("ABCDEF", 1.0, 2.0, t(0)));
        h.updater.flush(t(0)).await.unwrap();
        assert!(h.queue.enqueued.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_with_stream_drains_input_and_flushes() {
        let h = build(t(0), FlightUpdaterConfig::default());
        let reports = vec![
            pr("ABCDEF", 1.0, 2.0, t(0)),
            pr("123456", 3.0, 4.0, t(0)),
            pr("ABCDEF", 1.0, 9.0, t(0)),
        ];
        let s: PositionStream = Box::pin(stream::iter(reports));

        // Stream ends quickly, so the final drain flush handles all three.
        h.updater
            .run_with_stream(s, Duration::from_secs(60))
            .await
            .unwrap();

        // Two distinct aircraft. ABCDEF's last value (lon=9.0) wins.
        let snap = h.live.read();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.get(&Icao24::new("ABCDEF").unwrap()).unwrap().lon, 9.0);
    }
}
