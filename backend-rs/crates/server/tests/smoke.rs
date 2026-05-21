//! End-to-end smoke test: build the full ComposedApp with in-memory
//! adapters, drive real HTTP requests through the router, and verify
//! the wiring is sound.
//!
//! This exercises every layer:
//!   Config → Dependencies → build_app → AppState → router → handler →
//!   use case → port (in-memory fake).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use futures::Stream;
use http_body_util::BodyExt;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use flightradar_adapter_metadata::StaticAirlineDirectory;
use flightradar_api::{build_router, middleware::MiddlewareConfig};
use flightradar_domain::ports::clock::{Clock, SystemClock};
use flightradar_domain::ports::metadata_source::MetadataSource;
use flightradar_domain::ports::radar_source::{PositionStream, RadarError, RadarSource};
use flightradar_domain::ports::repositories::{
    AircraftRepository, CrawlerLogEntry, CrawlerLogRepository, CrawlerQueueEntry,
    CrawlerQueueRepository, FlightFilter, FlightRepository, Page, PageRequest, PositionRepository,
    RepoResult, RepositoryError, UserRepository,
};
use flightradar_domain::{
    Aircraft, Airline, AirlineIcao, Flight, FlightId, Icao24, PositionReport, User, UserId,
};
use flightradar_server::composition::Dependencies;
use flightradar_server::{build_app, Config};

// ---------------------------------------------------------------------------
// Minimal in-memory fakes (one per port)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct FlightStub(StdMutex<HashMap<String, Flight>>);
#[async_trait]
impl FlightRepository for FlightStub {
    async fn upsert(&self, f: &Flight) -> RepoResult<()> {
        self.0
            .lock()
            .unwrap()
            .insert(f.id.as_str().to_owned(), f.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: &FlightId) -> RepoResult<Flight> {
        self.0
            .lock()
            .unwrap()
            .get(id.as_str())
            .cloned()
            .ok_or(RepositoryError::NotFound)
    }
    async fn find_open_for_icao24(&self, _icao24: &Icao24) -> RepoResult<Option<Flight>> {
        Ok(None)
    }
    async fn list(&self, _f: &FlightFilter, page: PageRequest) -> RepoResult<Page<Flight>> {
        let items: Vec<_> = self.0.lock().unwrap().values().cloned().collect();
        let total = items.len() as u64;
        Ok(Page {
            items,
            total,
            page: page.page,
            page_size: page.page_size,
        })
    }
}

#[derive(Debug, Default)]
struct PosStub;
#[async_trait]
impl PositionRepository for PosStub {
    async fn append(&self, _id: &FlightId, _p: &PositionReport) -> RepoResult<()> {
        Ok(())
    }
    async fn append_batch(&self, _e: &[(FlightId, PositionReport)]) -> RepoResult<()> {
        Ok(())
    }
    async fn history(&self, _id: &FlightId) -> RepoResult<Vec<PositionReport>> {
        Ok(vec![])
    }
}

#[derive(Debug, Default)]
struct AircraftStub(StdMutex<HashMap<String, Aircraft>>);
#[async_trait]
impl AircraftRepository for AircraftStub {
    async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
        Ok(self.0.lock().unwrap().get(&icao24.to_string()).cloned())
    }
    async fn find_many(&self, _icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
        Ok(vec![])
    }
    async fn upsert(&self, _ac: &Aircraft) -> RepoResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct QueueStub;
#[async_trait]
impl CrawlerQueueRepository for QueueStub {
    async fn enqueue(&self, _icao24: &Icao24) -> RepoResult<()> {
        Ok(())
    }
    async fn next_batch(&self, _n: u32) -> RepoResult<Vec<CrawlerQueueEntry>> {
        Ok(vec![])
    }
    async fn record_attempt(&self, _icao24: &Icao24, _success: bool) -> RepoResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct LogStub;
#[async_trait]
impl CrawlerLogRepository for LogStub {
    async fn record(&self, _entry: &CrawlerLogEntry) -> RepoResult<()> {
        Ok(())
    }
    async fn recent_for(&self, _icao24: &Icao24, _limit: u32) -> RepoResult<Vec<CrawlerLogEntry>> {
        Ok(vec![])
    }
}

#[derive(Debug, Default)]
struct UserStub(StdMutex<HashMap<String, (User, Option<String>)>>);
#[async_trait]
impl UserRepository for UserStub {
    async fn find_by_id(&self, _id: &UserId) -> RepoResult<Option<User>> {
        Ok(None)
    }
    async fn find_by_email(&self, email: &str) -> RepoResult<Option<User>> {
        Ok(self.0.lock().unwrap().get(email).map(|(u, _)| u.clone()))
    }
    async fn upsert(&self, user: &User, hash: Option<&str>) -> RepoResult<()> {
        self.0
            .lock()
            .unwrap()
            .insert(user.email.clone(), (user.clone(), hash.map(str::to_owned)));
        Ok(())
    }
    async fn read_password_hash(&self, _id: &UserId) -> RepoResult<Option<String>> {
        Ok(None)
    }
    async fn touch_last_login(&self, _id: &UserId, _when: OffsetDateTime) -> RepoResult<()> {
        Ok(())
    }
}

/// Radar source that yields no items — keeps the supervisor task quiet
/// while the smoke test drives HTTP requests.
struct SilentRadar;
impl std::fmt::Debug for SilentRadar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SilentRadar")
    }
}
#[async_trait]
impl RadarSource for SilentRadar {
    fn name(&self) -> &'static str {
        "silent"
    }
    async fn stream(&self) -> Result<PositionStream, RadarError> {
        let s: std::pin::Pin<Box<dyn Stream<Item = PositionReport> + Send + 'static>> =
            Box::pin(futures::stream::empty());
        Ok(s)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_config() -> Config {
    Config {
        bind_addr: "127.0.0.1:0".into(),
        allowed_origins: vec!["http://localhost".into()],
        mongo_uri: "unused".into(),
        mongo_db: "unused".into(),
        radar_kind: flightradar_server::config::RadarKind::Dump1090,
        radar_endpoint: "unused".into(),
        flush_interval: Duration::from_secs(2),
        position_ttl: Duration::from_secs(60),
        military_only: false,
        nighthawk_base_url: None,
        airlines_file: None,
        jwt_secret: "this-is-a-32-byte-test-secret!12".into(),
        cookie_key: None,
        token_lifetime: Duration::from_secs(900),
        admin_email: None,
        admin_password: None,
        crawler_enabled: false,
        crawler_interval: Duration::from_secs(60),
        build_commit: "smoke".into(),
        build_timestamp: "1970".into(),
    }
}

fn deps() -> Dependencies {
    let mut airline_dir = StaticAirlineDirectory::from_airlines(vec![Airline::new(
        AirlineIcao::new("AFR").unwrap(),
        "Air France",
    )]);
    // Drop unused mut warning. (StaticAirlineDirectory::from_airlines returns owned)
    let _ = &mut airline_dir;

    Dependencies {
        flight_repo: Arc::new(FlightStub::default()),
        position_repo: Arc::new(PosStub),
        aircraft_repo: Arc::new(AircraftStub::default()),
        crawler_queue: Arc::new(QueueStub),
        crawler_log: Arc::new(LogStub),
        user_repo: Arc::new(UserStub::default()),
        radar: Arc::new(SilentRadar),
        metadata_sources: vec![Arc::new(SilentMetadata) as Arc<dyn MetadataSource>],
        airline_dir: Arc::new(airline_dir),
        clock: Arc::new(SystemClock) as Arc<dyn Clock>,
    }
}

#[derive(Debug)]
struct SilentMetadata;
#[async_trait]
impl MetadataSource for SilentMetadata {
    fn name(&self) -> &'static str {
        "silent"
    }
    async fn fetch(
        &self,
        _icao24: &Icao24,
    ) -> Result<Option<Aircraft>, flightradar_domain::ports::metadata_source::MetadataError> {
        Ok(None)
    }
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_app_serves_info_endpoint() {
    let app = build_app(&test_config(), deps()).await.unwrap();
    let router = build_router(app.state, &MiddlewareConfig::default());

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["commit"], "smoke");
}

#[tokio::test]
async fn full_app_anonymous_login_then_list_flights() {
    let app = build_app(&test_config(), deps()).await.unwrap();
    let router = build_router(app.state, &MiddlewareConfig::default());

    // POST /auth/anonymous → cookie
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/anonymous")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            s.starts_with("fr_session=")
                .then(|| s.split(';').next().unwrap().to_owned())
        })
        .expect("session cookie set");

    // GET /flights with cookie
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/flights")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(body["items"].is_array());
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn full_app_health_endpoints_are_open() {
    let app = build_app(&test_config(), deps()).await.unwrap();
    let router = build_router(app.state, &MiddlewareConfig::default());

    for path in ["/api/v1/health/alive", "/api/v1/health/ready"] {
        let resp = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "path {path}");
    }
}

#[tokio::test]
async fn full_app_airline_lookup_returns_seeded_airline() {
    let app = build_app(&test_config(), deps()).await.unwrap();
    let router = build_router(app.state, &MiddlewareConfig::default());

    // Anonymous login.
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/anonymous")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|v| v.to_str().ok().map(str::to_owned))
        .unwrap();
    let cookie = cookie.split(';').next().unwrap().to_owned();

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/airlines/AFR")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["name"], "Air France");
}

#[tokio::test]
async fn build_app_does_not_spawn_crawler_when_disabled() {
    let app = build_app(&test_config(), deps()).await.unwrap();
    assert!(app.aircraft_crawler.is_none());
}

#[tokio::test]
async fn build_app_spawns_crawler_when_enabled() {
    let mut cfg = test_config();
    cfg.crawler_enabled = true;
    let app = build_app(&cfg, deps()).await.unwrap();
    assert!(app.aircraft_crawler.is_some());
}

#[tokio::test]
async fn admin_user_seeded_when_env_present() {
    let mut cfg = test_config();
    cfg.admin_email = Some("admin@example.com".into());
    cfg.admin_password = Some("hunter2hunter2".into());

    let users = Arc::new(UserStub::default());
    let mut deps = deps();
    deps.user_repo = users.clone();

    let _ = build_app(&cfg, deps).await.unwrap();
    let map = users.0.lock().unwrap();
    assert!(map.contains_key("admin@example.com"));
}
