//! gRPC ADS-B source.
//!
//! Subscribes to `PlaneTrackingService.StreamUpdates` and forwards each
//! `PlaneUpdate` (ADD or UPDATE; REMOVE is handled via the live-state TTL
//! upstream). Conversion from the generated protobuf type to the domain
//! `PositionReport` lives in [`plane_update_to_position_report`] — a pure
//! function — so the wire ↔ domain mapping is exhaustively unit-testable
//! without a live channel.

use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, warn};

use flightradar_domain::ports::radar_source::{PositionStream, RadarError, RadarSource};
use flightradar_domain::{AircraftCategory, Callsign, Icao24, PositionReport};

use crate::proto::plane_tracking_service_client::PlaneTrackingServiceClient;
use crate::proto::{PlaneUpdate, StreamUpdatesRequest, UpdateType};

// ---------------------------------------------------------------------------
// Config + source struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GrpcAdsbConfig {
    /// gRPC endpoint, e.g. `http://localhost:50051`.
    pub endpoint: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    /// Server-side throttle: minimum interval between updates per aircraft.
    pub update_interval_ms: u32,
    /// Channel capacity between the gRPC consumer task and the stream
    /// returned to the application. Bounded → backpressure rather than
    /// unbounded memory growth.
    pub channel_capacity: usize,
    /// If the server stream ends or errors, wait this long before
    /// reconnecting.
    pub reconnect_backoff: Duration,
}

impl GrpcAdsbConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            update_interval_ms: 500,
            channel_capacity: 1024,
            reconnect_backoff: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcAdsbSource {
    config: GrpcAdsbConfig,
}

impl GrpcAdsbSource {
    pub fn new(config: GrpcAdsbConfig) -> Self {
        Self { config }
    }

    async fn build_channel(&self) -> Result<Channel, RadarError> {
        let endpoint = Endpoint::from_shared(self.config.endpoint.clone())
            .map_err(|e| RadarError::Unavailable(e.to_string()))?
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout);
        endpoint
            .connect()
            .await
            .map_err(|e| RadarError::Transport(Box::new(e)))
    }
}

#[async_trait]
impl RadarSource for GrpcAdsbSource {
    fn name(&self) -> &'static str {
        "grpc"
    }

    async fn stream(&self) -> Result<PositionStream, RadarError> {
        // Validate connectivity upfront so first-stream errors are returned
        // to the caller rather than swallowed by the background task.
        let initial = self.build_channel().await?;
        let (tx, rx) = mpsc::channel::<PositionReport>(self.config.channel_capacity);
        let config = self.config.clone();
        let source = self.clone();

        tokio::spawn(async move {
            let mut channel = initial;
            loop {
                if let Err(err) = run_subscription(channel.clone(), &config, &tx).await {
                    warn!(error = %err, "grpc subscription ended");
                }
                if tx.is_closed() {
                    debug!("downstream consumer dropped, exiting grpc reader");
                    return;
                }
                tokio::time::sleep(config.reconnect_backoff).await;
                match source.build_channel().await {
                    Ok(c) => channel = c,
                    Err(err) => {
                        warn!(error = %err, "grpc reconnect failed");
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        let pinned: Pin<Box<dyn Stream<Item = PositionReport> + Send + 'static>> = Box::pin(stream);
        Ok(pinned)
    }
}

async fn run_subscription(
    channel: Channel,
    config: &GrpcAdsbConfig,
    tx: &mpsc::Sender<PositionReport>,
) -> Result<(), RadarError> {
    let mut client = PlaneTrackingServiceClient::new(channel);
    let request = StreamUpdatesRequest {
        include_initial_snapshot: true,
        update_interval_ms: Some(config.update_interval_ms),
    };
    let mut stream = client
        .stream_updates(request)
        .await
        .map_err(|e| RadarError::Transport(Box::new(e)))?
        .into_inner();

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(update) => {
                let now = OffsetDateTime::now_utc();
                if let Some(pr) = plane_update_to_position_report(&update, now) {
                    if tx.send(pr).await.is_err() {
                        return Ok(()); // consumer gone
                    }
                }
            }
            Err(status) => {
                return Err(RadarError::Transport(Box::new(status)));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pure conversion
// ---------------------------------------------------------------------------

/// Convert a `PlaneUpdate` protobuf message into a domain `PositionReport`.
/// Returns `None` if the update is a REMOVE (handled via TTL upstream) or
/// the plane lacks the minimum required fields (icao24 + position).
///
/// `observed_at` is provided by the caller so this function stays pure.
pub fn plane_update_to_position_report(
    update: &PlaneUpdate,
    observed_at: OffsetDateTime,
) -> Option<PositionReport> {
    let update_type = UpdateType::try_from(update.update_type).ok()?;
    if matches!(update_type, UpdateType::Remove | UpdateType::Unspecified) {
        return None;
    }
    let plane = update.plane.as_ref()?;
    let icao24 = Icao24::new(&plane.icao_address).ok()?;
    let position = plane.position.as_ref()?;
    let mut pr =
        PositionReport::new(icao24, position.latitude, position.longitude, observed_at).ok()?;
    pr.altitude_ft = plane.altitude_gnss_feet.or(plane.altitude_feet);
    if let Some(v) = plane.velocity.as_ref() {
        pr.ground_speed_kt = Some(v.ground_speed_knots);
        pr.track_deg = Some(v.heading_degrees);
    }
    let trimmed = plane.callsign.trim();
    if !trimmed.is_empty() {
        if let Ok(cs) = Callsign::new(trimmed) {
            pr.callsign = Some(cs);
        }
    }
    let cat_u8 = u8::try_from(plane.category).ok()?;
    if let Ok(cat) = AircraftCategory::try_from(cat_u8) {
        pr.category = Some(cat);
    }
    Some(pr)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)] // values flow through unchanged
mod tests {
    use super::*;
    use crate::proto::{PlaneState, Position, Velocity};

    fn t() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn plane() -> PlaneState {
        PlaneState {
            icao_address: "abcdef".into(),
            callsign: "AFR990 ".into(),
            position: Some(Position {
                latitude: 47.4,
                longitude: 8.5,
                on_ground: false,
                mlat: false,
            }),
            altitude_feet: Some(30_000),
            velocity: Some(Velocity {
                ground_speed_knots: 450.0,
                heading_degrees: 180.0,
                vertical_rate_fpm: 0,
            }),
            last_seen_ms: 1_700_000_000_000,
            first_seen_ms: 1_700_000_000_000,
            message_count: 1,
            squawk: String::new(),
            emergency: 0,
            category: 3, // Medium1
            adsb_capable: true,
            altitude_gnss_feet: None,
        }
    }

    fn update_of(ty: UpdateType, plane: Option<PlaneState>) -> PlaneUpdate {
        PlaneUpdate {
            update_type: ty as i32,
            plane,
            removed_icao: None,
        }
    }

    #[test]
    fn converts_typical_update() {
        let pr =
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(plane())), t())
                .unwrap();
        assert_eq!(pr.icao24.as_str(), "ABCDEF");
        assert_eq!(pr.latitude, 47.4);
        assert_eq!(pr.longitude, 8.5);
        assert_eq!(pr.altitude_ft, Some(30_000));
        assert_eq!(pr.ground_speed_kt, Some(450.0));
        assert_eq!(pr.track_deg, Some(180.0));
        assert_eq!(pr.callsign.as_ref().unwrap().as_str(), "AFR990");
        assert_eq!(pr.category, Some(AircraftCategory::Medium1));
    }

    #[test]
    fn remove_updates_dropped() {
        assert!(plane_update_to_position_report(
            &update_of(UpdateType::Remove, Some(plane())),
            t()
        )
        .is_none());
    }

    #[test]
    fn unspecified_updates_dropped() {
        assert!(plane_update_to_position_report(
            &update_of(UpdateType::Unspecified, Some(plane())),
            t()
        )
        .is_none());
    }

    #[test]
    fn invalid_update_type_dropped() {
        let u = PlaneUpdate {
            update_type: 999,
            plane: Some(plane()),
            removed_icao: None,
        };
        assert!(plane_update_to_position_report(&u, t()).is_none());
    }

    #[test]
    fn missing_plane_payload_returns_none() {
        assert!(plane_update_to_position_report(&update_of(UpdateType::Add, None), t()).is_none());
    }

    #[test]
    fn missing_position_returns_none() {
        let mut p = plane();
        p.position = None;
        assert!(
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).is_none()
        );
    }

    #[test]
    fn invalid_icao24_returns_none() {
        let mut p = plane();
        p.icao_address = "NOT-HEX".into();
        assert!(
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).is_none()
        );
    }

    #[test]
    fn add_updates_accepted() {
        assert!(
            plane_update_to_position_report(&update_of(UpdateType::Add, Some(plane())), t())
                .is_some()
        );
    }

    #[test]
    fn altitude_gnss_takes_precedence() {
        let mut p = plane();
        p.altitude_gnss_feet = Some(30_500);
        let pr =
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).unwrap();
        assert_eq!(pr.altitude_ft, Some(30_500));
    }

    #[test]
    fn empty_callsign_omitted() {
        let mut p = plane();
        p.callsign = "   ".into();
        let pr =
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).unwrap();
        assert!(pr.callsign.is_none());
    }

    #[test]
    fn missing_velocity_leaves_speed_track_none() {
        let mut p = plane();
        p.velocity = None;
        let pr =
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).unwrap();
        assert!(pr.ground_speed_kt.is_none());
        assert!(pr.track_deg.is_none());
    }

    #[test]
    fn unknown_category_leaves_category_none() {
        let mut p = plane();
        p.category = 99; // out of enum range
        let pr =
            plane_update_to_position_report(&update_of(UpdateType::Update, Some(p)), t()).unwrap();
        assert!(pr.category.is_none());
    }

    #[test]
    fn source_name_is_stable() {
        let s = GrpcAdsbSource::new(GrpcAdsbConfig::new("http://localhost:50051"));
        assert_eq!(s.name(), "grpc");
    }
}
