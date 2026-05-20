//! Drives `GrpcAdsbSource::stream()` against an in-process tonic server.

use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use futures::StreamExt;
use tokio::net::TcpListener;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use flightradar_adapter_radar::proto::plane_tracking_service_server::{
    PlaneTrackingService, PlaneTrackingServiceServer,
};
use flightradar_adapter_radar::proto::{
    GetAllPlanesRequest, GetAllPlanesResponse, GetStatusRequest, GetStatusResponse, PlaneState,
    PlaneUpdate, Position, StreamUpdatesRequest, UpdateType, Velocity,
};
use flightradar_adapter_radar::{GrpcAdsbConfig, GrpcAdsbSource};
use flightradar_domain::ports::radar_source::RadarSource;

#[derive(Debug, Default)]
struct FakeService;

#[tonic::async_trait]
impl PlaneTrackingService for FakeService {
    async fn get_all_planes(
        &self,
        _req: Request<GetAllPlanesRequest>,
    ) -> Result<Response<GetAllPlanesResponse>, Status> {
        Ok(Response::new(GetAllPlanesResponse {
            planes: vec![],
            snapshot_time_ms: 0,
        }))
    }

    type StreamUpdatesStream =
        Pin<Box<dyn Stream<Item = Result<PlaneUpdate, Status>> + Send + 'static>>;

    async fn stream_updates(
        &self,
        _req: Request<StreamUpdatesRequest>,
    ) -> Result<Response<Self::StreamUpdatesStream>, Status> {
        let updates = vec![
            update(UpdateType::Add, plane("abcdef", 47.0, 8.0, "AFR990")),
            update(UpdateType::Update, plane("123456", 46.0, 7.0, "BAW238")),
            // REMOVE should be skipped by the source.
            PlaneUpdate {
                update_type: UpdateType::Remove as i32,
                plane: None,
                removed_icao: Some("abcdef".into()),
            },
        ];
        let stream = futures::stream::iter(updates.into_iter().map(Ok::<_, Status>));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_status(
        &self,
        _req: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        Ok(Response::new(GetStatusResponse::default()))
    }
}

fn plane(icao: &str, lat: f64, lon: f64, callsign: &str) -> PlaneState {
    PlaneState {
        icao_address: icao.into(),
        callsign: callsign.into(),
        position: Some(Position {
            latitude: lat,
            longitude: lon,
            on_ground: false,
            mlat: false,
        }),
        altitude_feet: Some(30_000),
        velocity: Some(Velocity {
            ground_speed_knots: 450.0,
            heading_degrees: 180.0,
            vertical_rate_fpm: 0,
        }),
        last_seen_ms: 0,
        first_seen_ms: 0,
        message_count: 1,
        squawk: String::new(),
        emergency: 0,
        category: 3,
        adsb_capable: true,
        altitude_gnss_feet: None,
    }
}

fn update(ty: UpdateType, plane: PlaneState) -> PlaneUpdate {
    PlaneUpdate {
        update_type: ty as i32,
        plane: Some(plane),
        removed_icao: None,
    }
}

async fn spawn_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stream = tokio_stream::wrappers::TcpListenerStream::new(listener);
    tokio::spawn(async move {
        Server::builder()
            .add_service(PlaneTrackingServiceServer::new(FakeService))
            .serve_with_incoming(stream)
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn stream_forwards_add_and_update_skipping_remove() {
    let endpoint = spawn_server().await;
    let source = GrpcAdsbSource::new(GrpcAdsbConfig::new(endpoint));
    let mut stream = source.stream().await.unwrap();

    let first = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap();
    // Both ADD and UPDATE come through; REMOVE is filtered out so the next
    // poll would either reconnect (and re-deliver from the fake) or hang.
    let mut icaos = vec![first.icao24.to_string(), second.icao24.to_string()];
    icaos.sort();
    assert_eq!(icaos, vec!["123456".to_string(), "ABCDEF".to_string()]);
}
