//! Drives `Dump1090Source::stream()` end-to-end against a `wiremock` fake.

use std::time::Duration;

use futures::StreamExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use flightradar_adapter_radar::{Dump1090Config, Dump1090Source};
use flightradar_domain::ports::radar_source::RadarSource;

const RESPONSE_BODY: &str = r#"{
    "now": 1700000000,
    "aircraft": [
        {"hex": "abcdef", "flight": "AFR990", "lat": 47.4, "lon": 8.5, "alt_geom": 30000},
        {"hex": "123456", "lat": 46.0, "lon": 7.0, "alt_geom": 12000}
    ]
}"#;

#[tokio::test]
async fn stream_yields_parsed_aircraft_from_polled_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data/aircraft.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RESPONSE_BODY, "application/json"))
        .mount(&server)
        .await;

    let mut config = Dump1090Config::new(server.uri());
    config.poll_interval = Duration::from_millis(20);
    let source = Dump1090Source::new(config).unwrap();

    let mut stream = source.stream().await.unwrap();
    // Take the first two items the source emits — both from the same poll.
    let first = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("first item")
        .expect("stream not closed");
    let second = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("second item")
        .expect("stream not closed");

    let mut hex: Vec<_> = [first, second]
        .iter()
        .map(|p| p.icao24.to_string())
        .collect();
    hex.sort();
    assert_eq!(hex, vec!["123456".to_string(), "ABCDEF".to_string()]);
}

#[tokio::test]
async fn stream_keeps_polling_after_server_error() {
    let server = MockServer::start().await;

    // First request: error. Subsequent requests: success.
    Mock::given(method("GET"))
        .and(path("/data/aircraft.json"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/data/aircraft.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(RESPONSE_BODY, "application/json"))
        .mount(&server)
        .await;

    let mut config = Dump1090Config::new(server.uri());
    config.poll_interval = Duration::from_millis(20);
    let source = Dump1090Source::new(config).unwrap();

    let mut stream = source.stream().await.unwrap();
    // Despite the initial 500, the stream should still yield from the
    // following poll.
    let next = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .expect("eventually a poll succeeds")
        .expect("stream not closed");
    assert!(["ABCDEF", "123456"].contains(&next.icao24.as_str()));
}
