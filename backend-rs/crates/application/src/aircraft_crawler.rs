//! Aircraft metadata crawler use case.
//!
//! Pulls a batch from the queue, asks each known `MetadataSource` what it
//! knows, merges the answers into an `Aircraft`, persists what we have, and
//! records the attempt. A per-source `CircuitBreaker` keeps a flapping
//! upstream from being hammered.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::{debug, warn};

use flightradar_domain::policy::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use flightradar_domain::ports::clock::Clock;
use flightradar_domain::ports::metadata_source::{MetadataError, MetadataSource};
use flightradar_domain::ports::repositories::{
    AircraftRepository, CrawlerLogEntry, CrawlerLogRepository, CrawlerQueueRepository,
};
use flightradar_domain::{Aircraft, Icao24};

use crate::error::ApplicationError;

#[derive(Debug, Clone, Copy)]
pub struct AircraftCrawlerConfig {
    pub batch_size: u32,
    pub breaker: CircuitBreakerConfig,
}

impl Default for AircraftCrawlerConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            breaker: CircuitBreakerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrawlerRunReport {
    pub processed: usize,
    pub upserted: usize,
    pub source_successes: usize,
    pub source_failures: usize,
    pub skipped_by_breaker: usize,
}

#[derive(Debug)]
pub struct AircraftCrawler {
    sources: Vec<Arc<dyn MetadataSource>>,
    queue_repo: Arc<dyn CrawlerQueueRepository>,
    log_repo: Arc<dyn CrawlerLogRepository>,
    aircraft_repo: Arc<dyn AircraftRepository>,
    clock: Arc<dyn Clock>,
    config: AircraftCrawlerConfig,
    breakers: Mutex<HashMap<String, CircuitBreaker>>,
}

impl AircraftCrawler {
    pub fn new(
        sources: Vec<Arc<dyn MetadataSource>>,
        queue_repo: Arc<dyn CrawlerQueueRepository>,
        log_repo: Arc<dyn CrawlerLogRepository>,
        aircraft_repo: Arc<dyn AircraftRepository>,
        clock: Arc<dyn Clock>,
        config: AircraftCrawlerConfig,
    ) -> Self {
        Self {
            sources,
            queue_repo,
            log_repo,
            aircraft_repo,
            clock,
            config,
            breakers: Mutex::new(HashMap::new()),
        }
    }

    /// Run one crawl pass: drain a batch from the queue and try to enrich
    /// each entry.
    pub async fn run_once(&self) -> Result<CrawlerRunReport, ApplicationError> {
        let batch = self.queue_repo.next_batch(self.config.batch_size).await?;
        let mut report = CrawlerRunReport {
            processed: batch.len(),
            ..Default::default()
        };

        for entry in batch {
            let outcome = self.process(&entry.icao24).await?;
            report.upserted += usize::from(outcome.upserted);
            report.source_successes += outcome.source_successes;
            report.source_failures += outcome.source_failures;
            report.skipped_by_breaker += outcome.skipped_by_breaker;
            self.queue_repo
                .record_attempt(&entry.icao24, outcome.upserted)
                .await?;
        }

        Ok(report)
    }

    async fn process(&self, icao24: &Icao24) -> Result<ProcessOutcome, ApplicationError> {
        let mut merged = Aircraft::new(icao24.clone());
        let mut outcome = ProcessOutcome::default();

        for source in &self.sources {
            let name = source.name().to_owned();

            if !self.breaker_allow(&name) {
                outcome.skipped_by_breaker += 1;
                continue;
            }

            match source.fetch(icao24).await {
                Ok(Some(found)) => {
                    merged.merge_from(&found);
                    outcome.source_successes += 1;
                    self.breaker_success(&name);
                    self.record_log(icao24, &name, true).await;
                    if merged.is_complete_with_operator() {
                        break;
                    }
                }
                Ok(None) => {
                    self.breaker_success(&name); // source healthy, just no data
                    self.record_log(icao24, &name, false).await;
                }
                Err(MetadataError::RateLimited) => {
                    outcome.source_failures += 1;
                    self.breaker_failure(&name);
                    self.record_log(icao24, &name, false).await;
                }
                Err(err) => {
                    outcome.source_failures += 1;
                    self.breaker_failure(&name);
                    self.record_log(icao24, &name, false).await;
                    warn!(source = %name, error = %err, %icao24, "metadata source error");
                }
            }
        }

        if merged.is_empty() {
            debug!(%icao24, "no metadata gathered for aircraft");
        } else {
            self.aircraft_repo.upsert(&merged).await?;
            outcome.upserted = true;
        }

        Ok(outcome)
    }

    fn breaker_allow(&self, name: &str) -> bool {
        let now = self.clock.now();
        let mut breakers = self.breakers.lock().expect("breakers mutex poisoned");
        let cb = breakers
            .entry(name.to_owned())
            .or_insert_with(|| CircuitBreaker::new(self.config.breaker));
        cb.allow(now)
    }

    fn breaker_success(&self, name: &str) {
        if let Some(cb) = self
            .breakers
            .lock()
            .expect("breakers mutex poisoned")
            .get_mut(name)
        {
            cb.record_success();
        }
    }

    fn breaker_failure(&self, name: &str) {
        let now = self.clock.now();
        let mut breakers = self.breakers.lock().expect("breakers mutex poisoned");
        let cb = breakers
            .entry(name.to_owned())
            .or_insert_with(|| CircuitBreaker::new(self.config.breaker));
        cb.record_failure(now);
    }

    async fn record_log(&self, icao24: &Icao24, source: &str, success: bool) {
        let entry = CrawlerLogEntry {
            icao24: icao24.clone(),
            source: source.to_owned(),
            success,
            recorded_at: self.clock.now(),
        };
        if let Err(err) = self.log_repo.record(&entry).await {
            debug!(error = %err, %icao24, source, "crawler log record failed");
        }
    }
}

#[derive(Debug, Default)]
struct ProcessOutcome {
    upserted: bool,
    source_successes: usize,
    source_failures: usize,
    skipped_by_breaker: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use time::OffsetDateTime;

    use flightradar_domain::ports::repositories::{CrawlerQueueEntry, RepoResult};
    use flightradar_domain::AircraftSource;

    use super::*;

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

    // -- Metadata source -------------------------------------------------

    #[derive(Debug)]
    struct ScriptedSource {
        name: &'static str,
        responses: StdMutex<Vec<Result<Option<Aircraft>, MetadataError>>>,
        calls: StdMutex<usize>,
    }

    impl ScriptedSource {
        fn new(
            name: &'static str,
            responses: Vec<Result<Option<Aircraft>, MetadataError>>,
        ) -> Self {
            Self {
                name,
                responses: StdMutex::new(responses),
                calls: StdMutex::new(0),
            }
        }
        fn call_count(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl MetadataSource for ScriptedSource {
        fn name(&self) -> &str {
            self.name
        }
        async fn fetch(&self, _icao24: &Icao24) -> Result<Option<Aircraft>, MetadataError> {
            *self.calls.lock().unwrap() += 1;
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(None)
            } else {
                responses.remove(0)
            }
        }
    }

    // -- Repositories ----------------------------------------------------

    #[derive(Debug, Default)]
    struct InMemQueue {
        items: StdMutex<Vec<Icao24>>,
        attempts: StdMutex<Vec<(Icao24, bool)>>,
    }
    #[async_trait]
    impl CrawlerQueueRepository for InMemQueue {
        async fn enqueue(&self, icao24: &Icao24) -> RepoResult<()> {
            self.items.lock().unwrap().push(icao24.clone());
            Ok(())
        }
        async fn next_batch(&self, n: u32) -> RepoResult<Vec<CrawlerQueueEntry>> {
            let mut items = self.items.lock().unwrap();
            let take = items.len().min(n as usize);
            let out: Vec<_> = items.drain(..take).collect();
            Ok(out
                .into_iter()
                .map(|icao24| CrawlerQueueEntry {
                    icao24,
                    attempts: 0,
                    last_attempt_at: None,
                })
                .collect())
        }
        async fn record_attempt(&self, icao24: &Icao24, success: bool) -> RepoResult<()> {
            self.attempts
                .lock()
                .unwrap()
                .push((icao24.clone(), success));
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct InMemLog {
        records: StdMutex<Vec<CrawlerLogEntry>>,
    }
    #[async_trait]
    impl CrawlerLogRepository for InMemLog {
        async fn record(&self, entry: &CrawlerLogEntry) -> RepoResult<()> {
            self.records.lock().unwrap().push(entry.clone());
            Ok(())
        }
        async fn recent_for(
            &self,
            _icao24: &Icao24,
            _limit: u32,
        ) -> RepoResult<Vec<CrawlerLogEntry>> {
            unimplemented!()
        }
    }

    #[derive(Debug, Default)]
    struct InMemAircraft {
        upserts: StdMutex<Vec<Aircraft>>,
    }
    #[async_trait]
    impl AircraftRepository for InMemAircraft {
        async fn find(&self, _icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
            Ok(None)
        }
        async fn find_many(&self, _icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
            unimplemented!()
        }
        async fn upsert(&self, aircraft: &Aircraft) -> RepoResult<()> {
            self.upserts.lock().unwrap().push(aircraft.clone());
            Ok(())
        }
    }

    // -- Helpers ---------------------------------------------------------

    fn aircraft_with_type(icao: &str, type_code: &str, source: &str) -> Aircraft {
        let mut a = Aircraft::new(Icao24::new(icao).unwrap());
        a.type_code = Some(type_code.into());
        a.type_description = Some(format!("{type_code} desc"));
        a.registration = Some("HB-XXX".into());
        a.operator = Some("Test".into());
        a.source = Some(AircraftSource::new(source));
        a
    }

    fn icao() -> Icao24 {
        Icao24::new("ABCDEF").unwrap()
    }

    fn build(
        sources: Vec<Arc<dyn MetadataSource>>,
        queue: Arc<InMemQueue>,
        log: Arc<InMemLog>,
        aircraft: Arc<InMemAircraft>,
        clock_t: OffsetDateTime,
    ) -> AircraftCrawler {
        AircraftCrawler::new(
            sources,
            queue,
            log,
            aircraft,
            Arc::new(FixedClock(clock_t)),
            AircraftCrawlerConfig::default(),
        )
    }

    // -- Tests -----------------------------------------------------------

    #[tokio::test]
    async fn empty_queue_produces_empty_report() {
        let queue = Arc::new(InMemQueue::default());
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());
        let crawler = build(vec![], queue, log, aircraft, t(0));

        let r = crawler.run_once().await.unwrap();
        assert_eq!(r, CrawlerRunReport::default());
    }

    #[tokio::test]
    async fn single_source_hit_persists_aircraft() {
        let queue = Arc::new(InMemQueue::default());
        queue.enqueue(&icao()).await.unwrap();
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());
        let src = Arc::new(ScriptedSource::new(
            "hex",
            vec![Ok(Some(aircraft_with_type("ABCDEF", "A320", "hex")))],
        ));
        let crawler = build(
            vec![src.clone()],
            queue.clone(),
            log.clone(),
            aircraft.clone(),
            t(0),
        );

        let r = crawler.run_once().await.unwrap();
        assert_eq!(r.processed, 1);
        assert_eq!(r.upserted, 1);
        assert_eq!(r.source_successes, 1);
        assert_eq!(aircraft.upserts.lock().unwrap().len(), 1);
        assert_eq!(queue.attempts.lock().unwrap().len(), 1);
        assert!(queue.attempts.lock().unwrap()[0].1); // success recorded
    }

    #[tokio::test]
    async fn missing_data_does_not_upsert() {
        let queue = Arc::new(InMemQueue::default());
        queue.enqueue(&icao()).await.unwrap();
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());
        let src = Arc::new(ScriptedSource::new("hex", vec![Ok(None)]));
        let crawler = build(vec![src], queue.clone(), log, aircraft.clone(), t(0));

        let r = crawler.run_once().await.unwrap();
        assert_eq!(r.processed, 1);
        assert_eq!(r.upserted, 0);
        assert!(aircraft.upserts.lock().unwrap().is_empty());
        assert!(!queue.attempts.lock().unwrap()[0].1); // failure recorded
    }

    #[tokio::test]
    async fn multi_source_merge_stops_when_complete() {
        let queue = Arc::new(InMemQueue::default());
        queue.enqueue(&icao()).await.unwrap();
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());

        // First source returns a complete record → second should not be hit.
        let first = Arc::new(ScriptedSource::new(
            "a",
            vec![Ok(Some(aircraft_with_type("ABCDEF", "A320", "a")))],
        ));
        let second = Arc::new(ScriptedSource::new("b", vec![Ok(None)]));
        let crawler = build(
            vec![first.clone(), second.clone()],
            queue,
            log,
            aircraft,
            t(0),
        );

        crawler.run_once().await.unwrap();
        assert_eq!(first.call_count(), 1);
        assert_eq!(second.call_count(), 0);
    }

    #[tokio::test]
    async fn partial_sources_merge() {
        let queue = Arc::new(InMemQueue::default());
        queue.enqueue(&icao()).await.unwrap();
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());

        let mut a1 = Aircraft::new(icao());
        a1.registration = Some("HB-ABC".into());
        let mut a2 = Aircraft::new(icao());
        a2.type_code = Some("A320".into());
        a2.type_description = Some("Airbus A320".into());

        let first = Arc::new(ScriptedSource::new("a", vec![Ok(Some(a1))]));
        let second = Arc::new(ScriptedSource::new("b", vec![Ok(Some(a2))]));
        let crawler = build(vec![first, second], queue, log, aircraft.clone(), t(0));

        crawler.run_once().await.unwrap();
        let upserted = &aircraft.upserts.lock().unwrap()[0];
        assert_eq!(upserted.registration.as_deref(), Some("HB-ABC"));
        assert_eq!(upserted.type_code.as_deref(), Some("A320"));
    }

    #[tokio::test]
    async fn breaker_opens_after_repeated_failures() {
        let queue = Arc::new(InMemQueue::default());
        // 6 work items, all going to the same broken source.
        for _ in 0..6 {
            queue.enqueue(&icao()).await.unwrap();
        }
        let log = Arc::new(InMemLog::default());
        let aircraft = Arc::new(InMemAircraft::default());
        let failing = Arc::new(ScriptedSource::new(
            "broken",
            (0..6)
                .map(|_| Err(MetadataError::Unavailable("down".into())))
                .collect(),
        ));
        let crawler = build(vec![failing.clone()], queue, log, aircraft, t(0));

        let report = crawler.run_once().await.unwrap();
        // After the 5th consecutive failure the breaker opens; the 6th call
        // is skipped without hitting the source.
        assert!(report.source_failures >= 5);
        assert!(report.skipped_by_breaker >= 1);
        assert!(failing.call_count() < 6);
    }
}
