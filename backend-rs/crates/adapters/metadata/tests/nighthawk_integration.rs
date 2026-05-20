//! End-to-end tests against a `wiremock` nighthawk-proxy.

use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use flightradar_adapter_metadata::{discover_nighthawk_sources, NighthawkClient, NighthawkSource};
use flightradar_domain::ports::metadata_source::{MetadataError, MetadataSource};
use flightradar_domain::Icao24;

fn icao() -> Icao24 {
    Icao24::new("ABCDEF").unwrap()
}

#[tokio::test]
async fn fetch_ok_returns_aircraft() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/aircraft/source/hexdb/ABCDEF"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"icao":"abcdef","registration":"HB-JCS","type_code":"A320","owner":"Swiss"}"#,
        ))
        .mount(&server)
        .await;

    let client = Arc::new(NighthawkClient::new(server.uri()).unwrap());
    let source = NighthawkSource::new(client, "hexdb");
    let ac = source.fetch(&icao()).await.unwrap().unwrap();
    assert_eq!(ac.registration.as_deref(), Some("HB-JCS"));
    assert_eq!(ac.operator.as_deref(), Some("Swiss"));
}

#[tokio::test]
async fn fetch_404_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/aircraft/source/hexdb/ABCDEF"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = Arc::new(NighthawkClient::new(server.uri()).unwrap());
    let source = NighthawkSource::new(client, "hexdb");
    assert!(source.fetch(&icao()).await.unwrap().is_none());
}

#[tokio::test]
async fn fetch_429_returns_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/aircraft/source/hexdb/ABCDEF"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let client = Arc::new(NighthawkClient::new(server.uri()).unwrap());
    let source = NighthawkSource::new(client, "hexdb");
    let err = source.fetch(&icao()).await.unwrap_err();
    assert!(matches!(err, MetadataError::RateLimited));
}

#[tokio::test]
async fn fetch_5xx_returns_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/aircraft/source/hexdb/ABCDEF"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = Arc::new(NighthawkClient::new(server.uri()).unwrap());
    let source = NighthawkSource::new(client, "hexdb");
    let err = source.fetch(&icao()).await.unwrap_err();
    assert!(matches!(err, MetadataError::Unavailable(_)));
}

#[tokio::test]
async fn discovery_returns_sources_sorted_by_priority() {
    let server = MockServer::start().await;
    let body = r#"{
        "sources": [
            { "name": "planespotters", "priority": 50 },
            { "name": "hexdb",         "priority": 10 },
            { "name": "openskynet",    "priority": 30 }
        ]
    }"#;
    Mock::given(method("GET"))
        .and(path("/sources"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let sources = discover_nighthawk_sources(server.uri()).await.unwrap();
    let names: Vec<_> = sources
        .iter()
        .map(|s| s.source_endpoint().to_owned())
        .collect();
    assert_eq!(names, vec!["hexdb", "openskynet", "planespotters"]);
}

#[tokio::test]
async fn discovery_handles_missing_priority_with_default() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sources"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"{ "sources": [{ "name": "x" }, { "name": "y", "priority": 5 }] }"#,
            ),
        )
        .mount(&server)
        .await;

    let sources = discover_nighthawk_sources(server.uri()).await.unwrap();
    // y(5) before x(default 100)
    assert_eq!(sources[0].source_endpoint(), "y");
    assert_eq!(sources[1].source_endpoint(), "x");
}
