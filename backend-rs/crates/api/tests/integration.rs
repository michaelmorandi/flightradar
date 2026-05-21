//! End-to-end HTTP tests that exercise the full Axum router with
//! in-memory ports backing every use case.
//!
//! These tests are the strongest signal we have that wiring is sound:
//! extractors, routing, JSON envelopes, error mapping, and cookie auth all
//! flow through the real Axum stack.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum_extra::extract::cookie::Key;
use http_body_util::BodyExt;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use flightradar_adapter_auth::{Argon2PasswordHasher, JwtSigner, JwtTokenIssuer, JwtTokenVerifier};
use flightradar_api::extractors::auth::AUTH_COOKIE;
use flightradar_api::middleware::MiddlewareConfig;
use flightradar_api::state::{AppState, AuthState, BuildInfo};
use flightradar_api::{build_router, ApiError};
use flightradar_application::{
    AdminService, AircraftQuery, AirlineQuery, AuthService, AuthServiceConfig, FlightQuery,
    LiveState, TokioBroadcastBus,
};
use flightradar_domain::ports::airline_directory::{AirlineDirectory, AirlineDirectoryError};
use flightradar_domain::ports::repositories::{
    AircraftRepository, FlightFilter, FlightRepository, Page, PageRequest, PositionRepository,
    RepoResult, RepositoryError, UserRepository,
};
use flightradar_domain::{
    Aircraft, Airline, AirlineIcao, Callsign, Flight, FlightId, Icao24, PositionReport, Role, User,
    UserId,
};

// ---------------------------------------------------------------------------
// Stub repos
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct StubFlightRepo {
    flights: StdMutex<HashMap<String, Flight>>,
}
impl StubFlightRepo {
    fn seed(&self, f: Flight) {
        self.flights
            .lock()
            .unwrap()
            .insert(f.id.as_str().to_owned(), f);
    }
}
#[async_trait]
impl FlightRepository for StubFlightRepo {
    async fn upsert(&self, _f: &Flight) -> RepoResult<()> {
        Ok(())
    }
    async fn find_by_id(&self, id: &FlightId) -> RepoResult<Flight> {
        self.flights
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
        let items: Vec<_> = self.flights.lock().unwrap().values().cloned().collect();
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
struct StubPositionRepo {
    history: StdMutex<Vec<PositionReport>>,
}
#[async_trait]
impl PositionRepository for StubPositionRepo {
    async fn append(&self, _id: &FlightId, _p: &PositionReport) -> RepoResult<()> {
        Ok(())
    }
    async fn append_batch(&self, _e: &[(FlightId, PositionReport)]) -> RepoResult<()> {
        Ok(())
    }
    async fn history(&self, _id: &FlightId) -> RepoResult<Vec<PositionReport>> {
        Ok(self.history.lock().unwrap().clone())
    }
}

#[derive(Debug, Default)]
struct StubAircraftRepo {
    by_icao: StdMutex<HashMap<String, Aircraft>>,
}
impl StubAircraftRepo {
    fn seed(&self, ac: Aircraft) {
        self.by_icao
            .lock()
            .unwrap()
            .insert(ac.icao24.to_string(), ac);
    }
}
#[async_trait]
impl AircraftRepository for StubAircraftRepo {
    async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
        Ok(self
            .by_icao
            .lock()
            .unwrap()
            .get(&icao24.to_string())
            .cloned())
    }
    async fn find_many(&self, icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
        let map = self.by_icao.lock().unwrap();
        Ok(icao24s
            .iter()
            .filter_map(|i| map.get(&i.to_string()).cloned())
            .collect())
    }
    async fn upsert(&self, _ac: &Aircraft) -> RepoResult<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct StubAirlineDirectory {
    by_icao: StdMutex<HashMap<String, Airline>>,
}
impl StubAirlineDirectory {
    fn seed(&self, a: Airline) {
        self.by_icao.lock().unwrap().insert(a.icao.to_string(), a);
    }
}
#[async_trait]
impl AirlineDirectory for StubAirlineDirectory {
    async fn find(&self, icao: &AirlineIcao) -> Result<Option<Airline>, AirlineDirectoryError> {
        Ok(self.by_icao.lock().unwrap().get(&icao.to_string()).cloned())
    }
    async fn search(
        &self,
        _query: &str,
        _limit: u32,
    ) -> Result<Vec<Airline>, AirlineDirectoryError> {
        Ok(self.by_icao.lock().unwrap().values().cloned().collect())
    }
    async fn all(&self) -> Result<Vec<Airline>, AirlineDirectoryError> {
        Ok(self.by_icao.lock().unwrap().values().cloned().collect())
    }
}

#[derive(Debug, Default)]
struct StubUserRepo {
    by_email: StdMutex<HashMap<String, (User, Option<String>)>>,
}
#[async_trait]
impl UserRepository for StubUserRepo {
    async fn find_by_id(&self, _id: &UserId) -> RepoResult<Option<User>> {
        Ok(None)
    }
    async fn find_by_email(&self, email: &str) -> RepoResult<Option<User>> {
        Ok(self
            .by_email
            .lock()
            .unwrap()
            .get(email)
            .map(|(u, _)| u.clone()))
    }
    async fn upsert(&self, user: &User, hash: Option<&str>) -> RepoResult<()> {
        self.by_email
            .lock()
            .unwrap()
            .insert(user.email.clone(), (user.clone(), hash.map(str::to_owned)));
        Ok(())
    }
    async fn read_password_hash(&self, id: &UserId) -> RepoResult<Option<String>> {
        for (_, (u, h)) in self.by_email.lock().unwrap().iter() {
            if u.id == *id {
                return Ok(h.clone());
            }
        }
        Ok(None)
    }
    async fn touch_last_login(&self, _id: &UserId, _when: OffsetDateTime) -> RepoResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    router: axum::Router,
    cookie_key: Key,
    users: Arc<StubUserRepo>,
}

fn harness() -> Harness {
    let flights_repo = Arc::new(StubFlightRepo::default());
    flights_repo.seed(Flight {
        id: FlightId::new("flight-1"),
        icao24: Icao24::new("ABCDEF").unwrap(),
        callsign: Some(Callsign::new("AFR990").unwrap()),
        airline_icao: Some(AirlineIcao::new("AFR").unwrap()),
        is_military: false,
        first_contact: OffsetDateTime::from_unix_timestamp(0).unwrap(),
        last_contact: OffsetDateTime::from_unix_timestamp(60).unwrap(),
    });
    let positions_repo = Arc::new(StubPositionRepo::default());
    let aircraft_repo = Arc::new(StubAircraftRepo::default());
    aircraft_repo.seed({
        let mut a = Aircraft::new(Icao24::new("ABCDEF").unwrap());
        a.registration = Some("HB-JCS".into());
        a.type_code = Some("A320".into());
        a
    });
    let airline_dir = Arc::new(StubAirlineDirectory::default());
    airline_dir.seed(Airline::new(AirlineIcao::new("AFR").unwrap(), "Air France"));
    let users = Arc::new(StubUserRepo::default());

    let signer = JwtSigner::from_secret(&[0xAB; 32], "flightradar-test").unwrap();
    let issuer: Arc<dyn flightradar_domain::ports::auth::TokenIssuer> =
        Arc::new(JwtTokenIssuer::new(signer.clone()));
    let verifier: Arc<dyn flightradar_domain::ports::auth::TokenVerifier> =
        Arc::new(JwtTokenVerifier::new(signer));
    let hasher = Arc::new(Argon2PasswordHasher);

    let live = LiveState::empty();
    let events = Arc::new(TokioBroadcastBus::new(live.clone()));
    let auth_service = Arc::new(AuthService::new(
        users.clone(),
        hasher,
        issuer,
        verifier.clone(),
        Arc::new(flightradar_domain::ports::clock::SystemClock),
        AuthServiceConfig::default(),
    ));
    let cookie_key = Key::generate();

    let admin_service = Arc::new(AdminService::new(
        flights_repo.clone() as Arc<dyn FlightRepository>,
        aircraft_repo.clone() as Arc<dyn AircraftRepository>,
    ));
    let state = AppState {
        flights: Arc::new(FlightQuery::new(flights_repo, positions_repo, live)),
        aircraft: Arc::new(AircraftQuery::new(aircraft_repo)),
        airlines: Arc::new(AirlineQuery::new(airline_dir)),
        admin: admin_service,
        auth: AuthState {
            service: auth_service,
            verifier,
            cookie_key: cookie_key.clone(),
        },
        events,
        build: BuildInfo {
            commit: "test".into(),
            build_timestamp: "1970".into(),
        },
    };
    let router = build_router(state, &MiddlewareConfig::default());
    Harness {
        router,
        cookie_key,
        users,
    }
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn login_cookie(resp: &axum::response::Response) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            if s.starts_with(&format!("{AUTH_COOKIE}=")) {
                Some(s.split(';').next().unwrap().to_owned())
            } else {
                None
            }
        })
}

async fn anon_login(h: &Harness) -> String {
    let resp = h
        .router
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
    login_cookie(&resp).expect("expected session cookie")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn info_endpoint_returns_build_metadata() {
    let h = harness();
    let resp = h
        .router
        .clone()
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
    assert_eq!(body["commit"], "test");
}

#[tokio::test]
async fn health_endpoints_are_unauthenticated() {
    let h = harness();
    for path in ["/api/v1/health/alive", "/api/v1/health/ready"] {
        let resp = h
            .router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "path {path}");
    }
}

#[tokio::test]
async fn protected_routes_require_auth() {
    let h = harness();
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/flights")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "unauthenticated");
}

#[tokio::test]
async fn anonymous_login_sets_session_cookie_and_returns_user() {
    let h = harness();
    let resp = h
        .router
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
    assert!(login_cookie(&resp).is_some());
    let body = body_json(resp).await;
    assert_eq!(body["user"]["role"], "anonymous");
    assert_eq!(body["user"]["is_admin"], false);
}

#[tokio::test]
async fn flights_list_returns_seeded_flights_when_authenticated() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/flights")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["icao24"], "ABCDEF");
    assert_eq!(body["total"], 1);
}

#[tokio::test]
async fn flight_not_found_returns_404_envelope() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/flights/missing")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn aircraft_lookup_returns_dto() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/aircraft/ABCDEF")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["icao24"], "ABCDEF");
    assert_eq!(body["registration"], "HB-JCS");
}

#[tokio::test]
async fn invalid_icao_returns_400() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/aircraft/ZZZ")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn airline_search_returns_seeded_airlines() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/airlines/search?q=air")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["icao"], "AFR");
}

#[tokio::test]
async fn logout_clears_session() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    // The Set-Cookie header should clear the cookie (max-age=0 / past expiry).
    let set_cookie = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|v| v.to_str().ok().map(str::to_owned))
        .expect("logout sets a removal cookie");
    assert!(set_cookie.starts_with(&format!("{AUTH_COOKIE}=")));
}

#[tokio::test]
async fn aircraft_bulk_rejects_invalid_icaos() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/aircraft")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"icao24s":["NOPE"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn aircraft_bulk_returns_subset() {
    let h = harness();
    let cookie = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/aircraft")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"icao24s":["ABCDEF","000001"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["requested"], 2);
    assert_eq!(body["found"], 1);
}

#[tokio::test]
async fn admin_login_succeeds_with_seeded_admin() {
    let h = harness();
    h.users.upsert_admin().await;

    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"email":"admin@example.com","password":"hunter2hunter2hunter2"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = login_cookie(&resp).expect("admin login sets cookie");
    let body = body_json(resp).await;
    assert_eq!(body["user"]["is_admin"], true);
    // Cookie is usable on protected routes.
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/flights")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_login_with_bad_password_returns_401() {
    let h = harness();
    h.users.upsert_admin().await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"email":"admin@example.com","password":"wrong"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unused_helpers_compile() {
    // Just exercise the harness cookie_key getter — guards against the
    // field being dropped accidentally.
    let h = harness();
    let _ = &h.cookie_key;
    let _ = ApiError::NotFound;
}

async fn admin_cookie(h: &Harness) -> String {
    h.users.upsert_admin().await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"email":"admin@example.com","password":"hunter2hunter2hunter2"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    login_cookie(&resp).expect("admin cookie")
}

#[tokio::test]
async fn admin_stats_requires_admin_role() {
    let h = harness();
    // Anonymous cookie is not enough.
    let anon = anon_login(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/stats")
                .header(header::COOKIE, &anon)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_stats_returns_flight_count() {
    let h = harness();
    let cookie = admin_cookie(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/stats")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["flight_count"], 1); // one seeded flight
}

#[tokio::test]
async fn admin_get_aircraft_returns_dto() {
    let h = harness();
    let cookie = admin_cookie(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/aircraft/ABCDEF")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["icao24"], "ABCDEF");
    assert_eq!(body["registration"], "HB-JCS");
}

#[tokio::test]
async fn admin_put_aircraft_persists_patch() {
    let h = harness();
    let cookie = admin_cookie(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/admin/aircraft/ABCDEF")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"operator":"Swiss","designator":"SWR"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["operator"], "Swiss");
    assert_eq!(body["designator"], "SWR");
    // Source is stamped automatically.
    assert_eq!(body["source"], "admin");
    // Untouched fields preserved.
    assert_eq!(body["registration"], "HB-JCS");
}

#[tokio::test]
async fn admin_put_aircraft_creates_when_absent() {
    let h = harness();
    let cookie = admin_cookie(&h).await;
    let resp = h
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/admin/aircraft/000001")
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"registration":"NEW-REG"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["icao24"], "000001");
    assert_eq!(body["registration"], "NEW-REG");
}

// ---------------------------------------------------------------------------
// StubUserRepo extension: lazy admin seeding using a real Argon2 hash so
// admin_login tests exercise the actual hasher path.
// ---------------------------------------------------------------------------

impl StubUserRepo {
    async fn upsert_admin(&self) {
        use flightradar_domain::ports::auth::PasswordHasher as _;
        let hasher = Argon2PasswordHasher;
        let hash = hasher.hash("hunter2hunter2hunter2").await.unwrap();
        let user = User {
            id: UserId::new("admin-1"),
            email: "admin@example.com".into(),
            role: Role::Admin,
            display_name: Some("Admin".into()),
            is_active: true,
            created_at: OffsetDateTime::now_utc(),
            last_login: None,
        };
        self.by_email
            .lock()
            .unwrap()
            .insert(user.email.clone(), (user, Some(hash)));
    }
}
