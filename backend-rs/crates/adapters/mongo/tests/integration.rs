//! Integration tests against a real Mongo instance.
//!
//! These tests are skipped by default (they require a live MongoDB and a
//! `MONGO_TEST_URI` env var). To run them locally:
//!
//! ```bash
//! docker run --rm -d -p 27017:27017 --name fr-mongo-test mongo:7
//! MONGO_TEST_URI=mongodb://localhost:27017 \
//!     cargo test -p flightradar-adapter-mongo --test integration -- --ignored
//! ```
//!
//! Each test isolates itself in a unique database (UUID-suffixed) and
//! drops it on completion, so they can run in parallel.

use std::time::Duration;

use flightradar_domain::ports::repositories::{
    AircraftRepository, CrawlerLogEntry, CrawlerLogRepository, CrawlerQueueRepository,
    FlightFilter, FlightRepository, PageRequest, PositionRepository, UserRepository,
};
use flightradar_domain::{
    Aircraft, AirlineIcao, Callsign, Flight, FlightId, Icao24, PositionReport, Role, User, UserId,
};
use mongodb::bson::oid::ObjectId;
use time::OffsetDateTime;

use flightradar_adapter_mongo::schema::SchemaConfig;
use flightradar_adapter_mongo::{
    ensure_schema, MongoAircraftRepository, MongoConfig, MongoConnection,
    MongoCrawlerLogRepository, MongoCrawlerQueueRepository, MongoFlightRepository,
    MongoPositionRepository, MongoUserRepository,
};

fn test_uri() -> Option<String> {
    std::env::var("MONGO_TEST_URI").ok()
}

async fn fresh_db() -> Option<MongoConnection> {
    let uri = test_uri()?;
    let db_name = format!("fr_test_{}", uuid_like());
    let cfg = MongoConfig::new(uri, db_name);
    let conn = MongoConnection::connect(&cfg).await.ok()?;
    let _ = ensure_schema(
        conn.database(),
        SchemaConfig {
            flight_retention: None,
            crawler_log_retention: None,
        },
    )
    .await;
    Some(conn)
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "{}_{:?}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::thread::current().id()
    )
}

async fn drop_db(conn: &MongoConnection) {
    let _ = conn.database().drop().await;
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn icao() -> Icao24 {
    Icao24::new("ABCDEF").unwrap()
}

// ---------------------------------------------------------------------------
// Flights
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn flight_repo_upsert_and_find_open_for_icao24() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoFlightRepository::new(conn.database());

    let f = Flight {
        id: FlightId::new(ObjectId::new().to_hex()),
        icao24: icao(),
        callsign: Some(Callsign::new("AFR990").unwrap()),
        airline_icao: Some(AirlineIcao::new("AFR").unwrap()),
        is_military: false,
        first_contact: now(),
        last_contact: now(),
    };
    repo.upsert(&f).await.unwrap();

    let found = repo.find_open_for_icao24(&icao()).await.unwrap().unwrap();
    assert_eq!(found.icao24, f.icao24);
    assert_eq!(found.callsign, f.callsign);
    drop_db(&conn).await;
}

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn flight_repo_list_paginates() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoFlightRepository::new(conn.database());

    for i in 0..7 {
        let f = Flight {
            id: FlightId::new(ObjectId::new().to_hex()),
            icao24: Icao24::new(&format!("{i:06X}")).unwrap(),
            callsign: None,
            airline_icao: None,
            is_military: false,
            first_contact: now(),
            last_contact: now() + time::Duration::seconds(i),
        };
        repo.upsert(&f).await.unwrap();
    }

    let page = repo
        .list(
            &FlightFilter::default(),
            PageRequest {
                page: 1,
                page_size: 3,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.total, 7);
    drop_db(&conn).await;
}

// ---------------------------------------------------------------------------
// Aircraft
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn aircraft_repo_upsert_and_find_many() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoAircraftRepository::new(conn.database());

    let mut a = Aircraft::new(icao());
    a.registration = Some("HB-JCS".into());
    a.type_code = Some("A320".into());
    a.type_description = Some("Airbus A320".into());
    repo.upsert(&a).await.unwrap();

    let found = repo.find(&icao()).await.unwrap().unwrap();
    assert_eq!(found, a);

    let many = repo.find_many(&[icao()]).await.unwrap();
    assert_eq!(many.len(), 1);

    let missing = repo.find(&Icao24::new("000001").unwrap()).await.unwrap();
    assert!(missing.is_none());
    drop_db(&conn).await;
}

// ---------------------------------------------------------------------------
// Positions (time-series)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn position_repo_append_batch_and_history() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoPositionRepository::new(conn.database());

    let id = FlightId::new(ObjectId::new().to_hex());
    let entries: Vec<_> = (0..3)
        .map(|i| {
            let pr = PositionReport::new(
                icao(),
                47.0 + f64::from(i),
                8.0,
                now() + time::Duration::seconds(i.into()),
            )
            .unwrap();
            (id.clone(), pr)
        })
        .collect();
    repo.append_batch(&entries).await.unwrap();

    let history = repo.history(&id).await.unwrap();
    assert_eq!(history.len(), 3);
    // Sorted ascending by observed_at.
    assert!(history[0].observed_at <= history[1].observed_at);
    drop_db(&conn).await;
}

// ---------------------------------------------------------------------------
// Crawler queue
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn crawler_queue_enqueue_is_idempotent_and_records_attempts() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoCrawlerQueueRepository::new(conn.database());

    repo.enqueue(&icao()).await.unwrap();
    repo.enqueue(&icao()).await.unwrap(); // dedupe by _id

    let batch = repo.next_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].attempts, 0);

    repo.record_attempt(&icao(), false).await.unwrap();
    let batch = repo.next_batch(10).await.unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].attempts, 1);
    assert!(batch[0].last_attempt_at.is_some());

    repo.record_attempt(&icao(), true).await.unwrap();
    let batch = repo.next_batch(10).await.unwrap();
    assert!(batch.is_empty()); // success removes from queue
    drop_db(&conn).await;
}

// ---------------------------------------------------------------------------
// Crawler log
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn crawler_log_record_and_recent_for() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoCrawlerLogRepository::new(conn.database());

    for i in 0..5 {
        let e = CrawlerLogEntry {
            icao24: icao(),
            source: "nighthawk".into(),
            success: i % 2 == 0,
            recorded_at: now() + time::Duration::seconds(i),
        };
        repo.record(&e).await.unwrap();
    }

    let recent = repo.recent_for(&icao(), 3).await.unwrap();
    assert_eq!(recent.len(), 3);
    assert!(recent[0].recorded_at >= recent[1].recorded_at); // newest first
    drop_db(&conn).await;
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires MONGO_TEST_URI"]
async fn user_repo_upsert_find_and_touch_last_login() {
    let Some(conn) = fresh_db().await else { return };
    let repo = MongoUserRepository::new(conn.database());

    let u = User {
        id: UserId::new("admin-1"),
        email: "admin@example.com".into(),
        role: Role::Admin,
        display_name: Some("Admin".into()),
        is_active: true,
        created_at: now(),
        last_login: None,
    };
    repo.upsert(&u, Some("hash")).await.unwrap();

    let by_id = repo.find_by_id(&u.id).await.unwrap().unwrap();
    assert_eq!(by_id.email, "admin@example.com");

    let by_email = repo
        .find_by_email("admin@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_email.id, u.id);

    let hash = repo.read_password_hash(&u.id).await.unwrap();
    assert_eq!(hash.as_deref(), Some("hash"));

    let later = now() + Duration::from_secs(60);
    repo.touch_last_login(&u.id, later).await.unwrap();
    let refreshed = repo.find_by_id(&u.id).await.unwrap().unwrap();
    assert!(refreshed.last_login.is_some());
    drop_db(&conn).await;
}
