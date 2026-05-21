//! Composition root: wires every adapter into the `AppState` and the
//! supervisor tasks.
//!
//! The only place in the codebase that knows the full dependency graph.
//! Tests use [`build_app`] with a pre-constructed [`Dependencies`] to
//! drive the same wiring.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum_extra::extract::cookie::Key;

use flightradar_adapter_auth::{Argon2PasswordHasher, JwtSigner, JwtTokenIssuer, JwtTokenVerifier};
use flightradar_adapter_metadata::{
    discover_nighthawk_sources, NighthawkSource, StaticAirlineDirectory,
};
use flightradar_adapter_mongo::{
    ensure_schema, schema::SchemaConfig, MongoAircraftRepository, MongoConfig, MongoConnection,
    MongoCrawlerLogRepository, MongoCrawlerQueueRepository, MongoFlightRepository,
    MongoPositionRepository, MongoUserRepository,
};
use flightradar_adapter_radar::{Dump1090Config, Dump1090Source, GrpcAdsbConfig, GrpcAdsbSource};
use flightradar_api::state::{AppState, AuthState, BuildInfo};
use flightradar_application::{
    AircraftCrawler, AircraftCrawlerConfig, AircraftQuery, AirlineQuery, AuthService,
    AuthServiceConfig, FlightQuery, FlightUpdater, FlightUpdaterConfig, LiveState,
    TokioBroadcastBus,
};
use flightradar_domain::policy::modes::ModeSClassifier;
use flightradar_domain::ports::airline_directory::AirlineDirectory;
use flightradar_domain::ports::auth::{PasswordHasher, TokenIssuer, TokenVerifier};
use flightradar_domain::ports::clock::{Clock, SystemClock};
use flightradar_domain::ports::metadata_source::MetadataSource;
use flightradar_domain::ports::radar_source::RadarSource;
use flightradar_domain::ports::repositories::{
    AircraftRepository, CrawlerLogRepository, CrawlerQueueRepository, FlightRepository,
    PositionRepository, UserRepository,
};
use flightradar_domain::{Role, User, UserId};

use crate::config::{Config, RadarKind};

/// The fully wired application. Holds a clone of the AppState (for the
/// HTTP router) plus the long-running services that supervisors will
/// drive.
pub struct ComposedApp {
    pub state: AppState,
    pub flight_updater: Arc<FlightUpdater>,
    pub aircraft_crawler: Option<Arc<AircraftCrawler>>,
}

impl std::fmt::Debug for ComposedApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposedApp")
            .field("has_crawler", &self.aircraft_crawler.is_some())
            .finish_non_exhaustive()
    }
}

/// Pre-built dependencies — used by tests to inject fakes without going
/// through the full Mongo/HTTP setup path. In production, `build_app`
/// constructs these from the [`Config`].
#[allow(missing_debug_implementations)] // trait-object fields are not Debug
pub struct Dependencies {
    pub flight_repo: Arc<dyn FlightRepository>,
    pub position_repo: Arc<dyn PositionRepository>,
    pub aircraft_repo: Arc<dyn AircraftRepository>,
    pub crawler_queue: Arc<dyn CrawlerQueueRepository>,
    pub crawler_log: Arc<dyn CrawlerLogRepository>,
    pub user_repo: Arc<dyn UserRepository>,
    pub radar: Arc<dyn RadarSource>,
    pub metadata_sources: Vec<Arc<dyn MetadataSource>>,
    pub airline_dir: Arc<dyn AirlineDirectory>,
    pub clock: Arc<dyn Clock>,
}

pub async fn build_app(config: &Config, deps: Dependencies) -> Result<ComposedApp> {
    // --- Auth -------------------------------------------------------------
    let signer = JwtSigner::from_secret(config.jwt_secret.as_bytes(), "flightradar-backend")
        .context("invalid JWT secret")?;
    let issuer: Arc<dyn TokenIssuer> = Arc::new(JwtTokenIssuer::new(signer.clone()));
    let verifier: Arc<dyn TokenVerifier> = Arc::new(JwtTokenVerifier::new(signer));
    let hasher: Arc<dyn PasswordHasher> = Arc::new(Argon2PasswordHasher);

    let cookie_key = load_or_generate_cookie_key(config.cookie_key.as_deref())?;

    let auth_service = Arc::new(AuthService::new(
        deps.user_repo.clone(),
        hasher.clone(),
        issuer,
        verifier.clone(),
        deps.clock.clone(),
        AuthServiceConfig {
            token_lifetime: time::Duration::seconds(
                i64::try_from(config.token_lifetime.as_secs()).unwrap_or(900),
            ),
            anonymous_email: "anonymous@flightradar.local",
        },
    ));

    seed_admin_user(&*deps.user_repo, &*hasher, &*deps.clock, config).await?;

    // --- Live state + event bus -------------------------------------------
    let live = LiveState::empty();
    let event_bus = Arc::new(TokioBroadcastBus::new(live.clone()));

    // --- Flight updater ---------------------------------------------------
    let classifier = Arc::new(ModeSClassifier::default());
    let updater = Arc::new(FlightUpdater::new(
        deps.radar.clone(),
        deps.flight_repo.clone(),
        deps.position_repo.clone(),
        deps.crawler_queue.clone(),
        event_bus.clone(),
        live.clone(),
        classifier,
        deps.clock.clone(),
        FlightUpdaterConfig {
            flight_gap_seconds: 3600,
            position_ttl_seconds: i64::try_from(config.position_ttl.as_secs()).unwrap_or(60),
            military_only: config.military_only,
            enqueue_unknown_aircraft: config.crawler_enabled,
        },
    ));

    // --- Aircraft crawler (optional) --------------------------------------
    let crawler = if config.crawler_enabled {
        Some(Arc::new(AircraftCrawler::new(
            deps.metadata_sources.clone(),
            deps.crawler_queue.clone(),
            deps.crawler_log.clone(),
            deps.aircraft_repo.clone(),
            deps.clock.clone(),
            AircraftCrawlerConfig::default(),
        )))
    } else {
        None
    };

    // --- Read-side queries ------------------------------------------------
    let flight_query = Arc::new(FlightQuery::new(
        deps.flight_repo.clone(),
        deps.position_repo.clone(),
        live,
    ));
    let aircraft_query = Arc::new(AircraftQuery::new(deps.aircraft_repo.clone()));
    let airline_query = Arc::new(AirlineQuery::new(deps.airline_dir));

    let state = AppState {
        flights: flight_query,
        aircraft: aircraft_query,
        airlines: airline_query,
        auth: AuthState {
            service: auth_service,
            verifier,
            cookie_key,
        },
        events: event_bus,
        build: BuildInfo {
            commit: config.build_commit.clone(),
            build_timestamp: config.build_timestamp.clone(),
        },
    };

    Ok(ComposedApp {
        state,
        flight_updater: updater,
        aircraft_crawler: crawler,
    })
}

/// Helper for production wiring: build a `Dependencies` from real Mongo,
/// the configured radar source, and the nighthawk-proxy.
pub async fn build_production_deps(config: &Config) -> Result<Dependencies> {
    // --- Mongo ------------------------------------------------------------
    let mongo_cfg = MongoConfig::new(&config.mongo_uri, &config.mongo_db);
    let mongo = MongoConnection::connect(&mongo_cfg)
        .await
        .context("connect to mongo")?;
    ensure_schema(mongo.database(), SchemaConfig::default())
        .await
        .context("ensure mongo schema")?;

    let flight_repo: Arc<dyn FlightRepository> =
        Arc::new(MongoFlightRepository::new(mongo.database()));
    let position_repo: Arc<dyn PositionRepository> =
        Arc::new(MongoPositionRepository::new(mongo.database()));
    let aircraft_repo: Arc<dyn AircraftRepository> =
        Arc::new(MongoAircraftRepository::new(mongo.database()));
    let crawler_queue: Arc<dyn CrawlerQueueRepository> =
        Arc::new(MongoCrawlerQueueRepository::new(mongo.database()));
    let crawler_log: Arc<dyn CrawlerLogRepository> =
        Arc::new(MongoCrawlerLogRepository::new(mongo.database()));
    let user_repo: Arc<dyn UserRepository> = Arc::new(MongoUserRepository::new(mongo.database()));

    // --- Radar ------------------------------------------------------------
    let radar: Arc<dyn RadarSource> = match config.radar_kind {
        RadarKind::Dump1090 => Arc::new(
            Dump1090Source::new(Dump1090Config::new(&config.radar_endpoint))
                .context("build dump1090 source")?,
        ),
        RadarKind::Grpc => Arc::new(GrpcAdsbSource::new(GrpcAdsbConfig::new(
            &config.radar_endpoint,
        ))),
    };

    // --- Metadata sources -------------------------------------------------
    let metadata_sources: Vec<Arc<dyn MetadataSource>> = if let Some(url) =
        config.nighthawk_base_url.as_deref()
    {
        match discover_nighthawk_sources(url).await {
            Ok(sources) => {
                let count = sources.len();
                tracing::info!(count, "discovered nighthawk sources");
                sources
                    .into_iter()
                    .map(|s: NighthawkSource| Arc::new(s) as Arc<dyn MetadataSource>)
                    .collect()
            }
            Err(err) => {
                tracing::warn!(error = %err, "nighthawk discovery failed; crawler will have no sources");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // --- Airline directory ------------------------------------------------
    let airline_dir: Arc<dyn AirlineDirectory> = if let Some(p) = config.airlines_file.as_deref() {
        Arc::new(
            StaticAirlineDirectory::from_file(p)
                .with_context(|| format!("load airlines file {}", p.display()))?,
        )
    } else {
        tracing::warn!("no AIRLINES_FILE configured; airline directory is empty");
        Arc::new(StaticAirlineDirectory::empty())
    };

    Ok(Dependencies {
        flight_repo,
        position_repo,
        aircraft_repo,
        crawler_queue,
        crawler_log,
        user_repo,
        radar,
        metadata_sources,
        airline_dir,
        clock: Arc::new(SystemClock),
    })
}

fn load_or_generate_cookie_key(hex: Option<&str>) -> Result<Key> {
    let bytes = if let Some(hex) = hex {
        decode_hex(hex).context("invalid COOKIE_KEY hex")?
    } else {
        // No persisted key — generate a fresh one. Sessions don't survive
        // a restart in this mode, which is fine for clean-slate auth.
        tracing::warn!("no COOKIE_KEY configured; generating ephemeral key");
        return Ok(Key::generate());
    };
    if bytes.len() < 64 {
        anyhow::bail!("COOKIE_KEY must decode to at least 64 bytes");
    }
    Ok(Key::from(&bytes))
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        anyhow::bail!("odd-length hex string");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        let pair = std::str::from_utf8(chunk).context("non-utf8 hex")?;
        out.push(u8::from_str_radix(pair, 16).context("invalid hex char")?);
    }
    Ok(out)
}

async fn seed_admin_user(
    repo: &dyn UserRepository,
    hasher: &dyn PasswordHasher,
    clock: &dyn Clock,
    config: &Config,
) -> Result<()> {
    let (Some(email), Some(password)) = (
        config.admin_email.as_deref(),
        config.admin_password.as_deref(),
    ) else {
        return Ok(());
    };

    if repo.find_by_email(email).await?.is_some() {
        return Ok(());
    }

    let hash = hasher.hash(password).await.context("hash admin password")?;
    let user = User {
        id: UserId::new(format!("admin-{}", email.replace('@', "_"))),
        email: email.into(),
        role: Role::Admin,
        display_name: Some("Admin".into()),
        is_active: true,
        created_at: clock.now(),
        last_login: None,
    };
    repo.upsert(&user, Some(&hash)).await?;
    tracing::info!(%email, "seeded admin user");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_hex_round_trip() {
        let bytes = decode_hex("00ff10").unwrap();
        assert_eq!(bytes, vec![0x00, 0xFF, 0x10]);
    }

    #[test]
    fn decode_hex_rejects_odd_length() {
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn decode_hex_rejects_non_hex_char() {
        assert!(decode_hex("zz").is_err());
    }

    #[test]
    fn load_or_generate_returns_generated_key_when_none() {
        // We just verify it does not error and produces a key. Generated
        // keys are random — no further assertion is possible without
        // hooking into the crate's internals.
        let key = load_or_generate_cookie_key(None).unwrap();
        // Sanity: signing master is non-empty.
        let _ = key.master();
    }

    #[test]
    fn load_or_generate_rejects_short_key() {
        let too_short_hex: String = (0..30).map(|_| "ab").collect();
        let err = load_or_generate_cookie_key(Some(&too_short_hex)).unwrap_err();
        assert!(err.to_string().contains("64 bytes"));
    }

    #[test]
    fn load_or_generate_accepts_64_byte_key() {
        let hex_64_bytes: String = (0..64).map(|_| "ab").collect();
        let key = load_or_generate_cookie_key(Some(&hex_64_bytes)).unwrap();
        let _ = key.master();
    }
}
