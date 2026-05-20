//! Nighthawk-proxy client and `MetadataSource` impl.
//!
//! Two layers:
//! - [`NighthawkClient`] — thin HTTP wrapper around the proxy. Holds the
//!   reqwest `Client` and base URL.
//! - [`NighthawkSource`] — one instance per discovered sub-source
//!   (`/aircraft/source/{name}/{icao}`). Implements the domain
//!   `MetadataSource` port.
//!
//! Discovery happens via [`discover_nighthawk_sources`]: it hits
//! `/sources`, builds one `NighthawkSource` per entry, and orders them by
//! ascending priority (lower number = higher precedence).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use flightradar_domain::ports::metadata_source::{MetadataError, MetadataSource};
use flightradar_domain::{Aircraft, AircraftSource, Icao24};

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NighthawkClient {
    client: Client,
    base_url: String,
}

impl NighthawkClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, MetadataError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("flightradar-rs/1.0")
            .build()
            .map_err(|e| MetadataError::Transport(Box::new(e)))?;
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        Ok(Self { client, base_url })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Discover the sub-sources exposed by the proxy. Returns them sorted by
    /// priority (ascending — lower number first).
    pub async fn list_sources(&self) -> Result<Vec<SourceInfo>, MetadataError> {
        let url = format!("{}/sources", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| MetadataError::Transport(Box::new(e)))?;
        if !response.status().is_success() {
            return Err(MetadataError::Unavailable(format!(
                "/sources returned {}",
                response.status()
            )));
        }
        let body: SourcesPayload = response
            .json()
            .await
            .map_err(|e| MetadataError::MalformedPayload(e.to_string()))?;
        let mut sources = body.sources;
        sources.sort_by_key(|s| s.priority);
        Ok(sources)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceInfo {
    pub name: String,
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    100
}

#[derive(Debug, Deserialize)]
struct SourcesPayload {
    #[serde(default)]
    sources: Vec<SourceInfo>,
}

// ---------------------------------------------------------------------------
// Source impl
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NighthawkSource {
    client: Arc<NighthawkClient>,
    source_name: String,
    display_name: String,
}

impl NighthawkSource {
    pub fn new(client: Arc<NighthawkClient>, source_name: impl Into<String>) -> Self {
        let source_name = source_name.into();
        let display_name = format!("nighthawk:{source_name}");
        Self {
            client,
            source_name,
            display_name,
        }
    }

    pub fn source_endpoint(&self) -> &str {
        &self.source_name
    }
}

#[async_trait]
impl MetadataSource for NighthawkSource {
    fn name(&self) -> &str {
        &self.display_name
    }

    async fn fetch(&self, icao24: &Icao24) -> Result<Option<Aircraft>, MetadataError> {
        let url = format!(
            "{}/aircraft/source/{}/{}",
            self.client.base_url, self.source_name, icao24
        );
        let response = self
            .client
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| MetadataError::Transport(Box::new(e)))?;

        match response.status() {
            StatusCode::OK => {
                let body = response
                    .text()
                    .await
                    .map_err(|e| MetadataError::Transport(Box::new(e)))?;
                parse_aircraft_payload(&body, icao24, &self.display_name)
            }
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::TOO_MANY_REQUESTS => Err(MetadataError::RateLimited),
            s if s.is_server_error() => Err(MetadataError::Unavailable(format!(
                "{} returned {}",
                self.display_name, s
            ))),
            s => Err(MetadataError::Unavailable(format!(
                "unexpected status {} from {}",
                s, self.display_name
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Pure parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AircraftPayload {
    // The upstream echoes the requested ICAO. We already know it (caller
    // passes &Icao24) but we still accept it on the wire to be tolerant.
    #[serde(default, rename = "icao")]
    _icao: Option<String>,
    #[serde(default)]
    registration: Option<String>,
    #[serde(default)]
    type_code: Option<String>,
    #[serde(default)]
    type_description: Option<String>,
    #[serde(default, alias = "operator")]
    owner: Option<String>,
    #[serde(default)]
    designator: Option<String>,
}

/// Parse a nighthawk-proxy aircraft response into an [`Aircraft`].
///
/// Returns `Ok(None)` if the payload is empty or only echoes the requested
/// ICAO without any real metadata (matches the Python "treat as not found"
/// behaviour for upstream sources that return only the address).
pub fn parse_aircraft_payload(
    body: &str,
    icao24: &Icao24,
    source: &str,
) -> Result<Option<Aircraft>, MetadataError> {
    if body.trim().is_empty() {
        return Ok(None);
    }
    let payload: AircraftPayload =
        serde_json::from_str(body).map_err(|e| MetadataError::MalformedPayload(e.to_string()))?;

    let mut ac = Aircraft::new(icao24.clone());
    ac.registration = normalise(payload.registration);
    ac.type_code = normalise(payload.type_code);
    ac.type_description = normalise(payload.type_description);
    ac.operator = normalise(payload.owner);
    ac.designator = normalise(payload.designator);
    ac.source = Some(AircraftSource::new(source));

    if ac.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ac))
    }
}

fn normalise(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Discovery helper
// ---------------------------------------------------------------------------

/// Discover the sub-sources exposed by the proxy and return one
/// `NighthawkSource` per entry, ordered by priority. Sharing the same
/// `Arc<NighthawkClient>` across all returned sources keeps the connection
/// pool warm.
pub async fn discover_nighthawk_sources(
    base_url: impl Into<String>,
) -> Result<Vec<NighthawkSource>, MetadataError> {
    let client = Arc::new(NighthawkClient::new(base_url)?);
    let infos = client.list_sources().await?;
    Ok(infos
        .into_iter()
        .map(|s| NighthawkSource::new(client.clone(), s.name))
        .collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn icao() -> Icao24 {
        Icao24::new("ABCDEF").unwrap()
    }

    // -- parse_aircraft_payload ----------------------------------------

    #[test]
    fn parses_full_payload() {
        let body = r#"{
            "icao": "abcdef",
            "registration": " HB-JCS ",
            "type_code": "A320",
            "type_description": "Airbus A320",
            "owner": "Swiss",
            "designator": "SWR"
        }"#;
        let ac = parse_aircraft_payload(body, &icao(), "nighthawk:hexdb")
            .unwrap()
            .unwrap();
        assert_eq!(ac.icao24, icao());
        assert_eq!(ac.registration.as_deref(), Some("HB-JCS")); // trimmed
        assert_eq!(ac.type_code.as_deref(), Some("A320"));
        assert_eq!(ac.type_description.as_deref(), Some("Airbus A320"));
        assert_eq!(ac.operator.as_deref(), Some("Swiss"));
        assert_eq!(ac.designator.as_deref(), Some("SWR"));
        assert_eq!(ac.source.as_ref().unwrap().as_str(), "nighthawk:hexdb");
    }

    #[test]
    fn accepts_operator_alias_for_owner() {
        let body = r#"{ "icao": "abcdef", "registration": "X", "operator": "SAS" }"#;
        let ac = parse_aircraft_payload(body, &icao(), "src")
            .unwrap()
            .unwrap();
        assert_eq!(ac.operator.as_deref(), Some("SAS"));
    }

    #[test]
    fn empty_body_is_not_found() {
        assert!(parse_aircraft_payload("", &icao(), "src")
            .unwrap()
            .is_none());
        assert!(parse_aircraft_payload("   ", &icao(), "src")
            .unwrap()
            .is_none());
    }

    #[test]
    fn icao_only_payload_is_not_found() {
        let body = r#"{ "icao": "abcdef" }"#;
        assert!(parse_aircraft_payload(body, &icao(), "src")
            .unwrap()
            .is_none());
    }

    #[test]
    fn all_whitespace_fields_are_not_found() {
        let body = r#"{ "registration": "  ", "type_code": "" }"#;
        assert!(parse_aircraft_payload(body, &icao(), "src")
            .unwrap()
            .is_none());
    }

    #[test]
    fn malformed_json_returns_malformed_payload() {
        let err = parse_aircraft_payload("not json", &icao(), "src").unwrap_err();
        assert!(matches!(err, MetadataError::MalformedPayload(_)));
    }

    #[test]
    fn partial_payload_returns_aircraft() {
        let body = r#"{ "icao": "abcdef", "registration": "HB-JCS" }"#;
        let ac = parse_aircraft_payload(body, &icao(), "src")
            .unwrap()
            .unwrap();
        assert!(ac.registration.is_some());
        assert!(ac.type_code.is_none());
        assert!(ac.operator.is_none());
    }

    // -- NighthawkClient + Source instantiation ------------------------

    #[test]
    fn trims_trailing_slash_from_base_url() {
        let c = NighthawkClient::new("http://nighthawk/").unwrap();
        assert_eq!(c.base_url(), "http://nighthawk");
    }

    #[test]
    fn source_display_name_includes_endpoint() {
        let client = Arc::new(NighthawkClient::new("http://nighthawk").unwrap());
        let src = NighthawkSource::new(client, "hexdb");
        assert_eq!(src.name(), "nighthawk:hexdb");
        assert_eq!(src.source_endpoint(), "hexdb");
    }

    #[test]
    fn source_info_default_priority() {
        let info: SourceInfo = serde_json::from_str(r#"{ "name": "x" }"#).unwrap();
        assert_eq!(info.priority, 100);
    }
}
