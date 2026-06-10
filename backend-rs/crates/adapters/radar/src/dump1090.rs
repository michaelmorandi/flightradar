//! dump1090 HTTP source.
//!
//! Polls `GET {base_url}/data/aircraft.json` at a fixed interval and
//! re-streams each parsed aircraft as a `PositionReport`. The JSON parsing
//! is a pure function ([`parse_aircraft_json`]) so it can be exhaustively
//! unit-tested without the stream machinery.

use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use time::OffsetDateTime;
use tracing::{debug, warn};

use flightradar_domain::ports::radar_source::{PositionStream, RadarError, RadarSource};
use flightradar_domain::{Callsign, Icao24, PositionReport};

// ---------------------------------------------------------------------------
// Config + source struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Dump1090Config {
    pub base_url: String,
    pub poll_interval: Duration,
    pub request_timeout: Duration,
}

impl Dump1090Config {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            poll_interval: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dump1090Source {
    client: Client,
    config: Dump1090Config,
}

impl Dump1090Source {
    pub fn new(config: Dump1090Config) -> Result<Self, RadarError> {
        let client = Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| RadarError::Transport(Box::new(e)))?;
        Ok(Self { client, config })
    }

    fn aircraft_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{base}/data/aircraft.json")
    }

    async fn fetch_once(&self) -> Result<Vec<PositionReport>, RadarError> {
        let url = self.aircraft_url();
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| RadarError::Transport(Box::new(e)))?;
        let status = response.status();
        if !status.is_success() {
            return Err(RadarError::Unavailable(format!(
                "dump1090 returned HTTP {status}"
            )));
        }
        let body = response
            .text()
            .await
            .map_err(|e| RadarError::Transport(Box::new(e)))?;
        let observed_at = OffsetDateTime::now_utc();
        parse_aircraft_json(&body, observed_at)
    }
}

#[async_trait]
impl RadarSource for Dump1090Source {
    fn name(&self) -> &'static str {
        "dump1090"
    }

    async fn stream(&self) -> Result<PositionStream, RadarError> {
        let this = self.clone();
        let stream = async_stream::stream! {
            let mut ticker = tokio::time::interval(this.config.poll_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                match this.fetch_once().await {
                    Ok(reports) => {
                        for r in reports {
                            yield r;
                        }
                    }
                    Err(err) => {
                        warn!(source = "dump1090", error = %err, "poll failed");
                    }
                }
            }
        };
        let pinned: Pin<Box<dyn Stream<Item = PositionReport> + Send + 'static>> = Box::pin(stream);
        Ok(pinned)
    }
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

/// Top-level envelope. We deliberately accept *anything* in `aircraft`
/// (`Vec<Value>`) and decode each entry individually, so one malformed
/// row never poisons a whole poll.
#[derive(Debug, Deserialize)]
struct AircraftResponse {
    #[serde(default)]
    aircraft: Vec<Value>,
}

/// Parse a dump1090 `aircraft.json` body into [`PositionReport`]s.
///
/// Lenient by design:
///   - Each aircraft is decoded individually; failures are logged and
///     skipped, never propagated up.
///   - Altitude can be reported as the string `"ground"` (surface
///     vehicles) — coerced to `0`.
///   - Either `alt_geom` *or* `alt_baro` may be set; we prefer
///     `alt_geom` (GNSS) and fall back to `alt_baro`. Some receivers
///     emit both fields on the same aircraft; we tolerate that too.
///
/// Aircraft without lat/lon are dropped — we cannot place them on the
/// map and the in-memory state has no use for them.
pub fn parse_aircraft_json(
    body: &str,
    observed_at: OffsetDateTime,
) -> Result<Vec<PositionReport>, RadarError> {
    let parsed: AircraftResponse =
        serde_json::from_str(body).map_err(|e| RadarError::MalformedPayload(e.to_string()))?;

    let mut out = Vec::with_capacity(parsed.aircraft.len());
    let mut skipped = 0_usize;
    for raw in &parsed.aircraft {
        match decode_aircraft(raw, observed_at) {
            Some(report) => out.push(report),
            None => skipped += 1,
        }
    }
    if skipped > 0 {
        debug!(skipped, kept = out.len(), "dump1090: skipped some entries");
    }
    Ok(out)
}

fn decode_aircraft(raw: &Value, observed_at: OffsetDateTime) -> Option<PositionReport> {
    let obj = raw.as_object()?;

    let hex = obj.get("hex").and_then(Value::as_str)?;
    let icao24 = Icao24::new(hex).ok()?;
    let lat = obj.get("lat").and_then(Value::as_f64)?;
    let lon = obj.get("lon").and_then(Value::as_f64)?;
    let mut pr = PositionReport::new(icao24, lat, lon, observed_at).ok()?;

    pr.altitude_ft = decode_altitude(obj);
    pr.ground_speed_kt = obj.get("gs").and_then(Value::as_f64);
    pr.track_deg = obj.get("track").and_then(Value::as_f64);

    if let Some(flight) = obj.get("flight").and_then(Value::as_str) {
        let trimmed = flight.trim();
        if !trimmed.is_empty() {
            if let Ok(cs) = Callsign::new(trimmed) {
                pr.callsign = Some(cs);
            }
        }
    }
    Some(pr)
}

/// Pick altitude with `alt_geom` preferred, `alt_baro` fallback.
/// `"ground"` strings (surface vehicles) collapse to `Some(0)`; junk
/// values yield `None`.
#[allow(clippy::cast_possible_truncation)] // values clamped to i32 range
fn decode_altitude(obj: &serde_json::Map<String, Value>) -> Option<i32> {
    for key in ["alt_geom", "alt_baro"] {
        if let Some(value) = obj.get(key) {
            if let Some(n) = value.as_i64() {
                return Some(n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
            }
            if let Some(f) = value.as_f64() {
                if f.is_finite() {
                    return Some(f.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32);
                }
            }
            if value.as_str() == Some("ground") {
                return Some(0);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)] // values flow through unchanged from JSON
mod tests {
    use super::*;

    fn t() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn parses_typical_aircraft_payload() {
        let body = r#"{
            "now": 1700000000,
            "aircraft": [
                {
                    "hex": "abcdef",
                    "flight": "AFR990  ",
                    "lat": 47.4,
                    "lon": 8.5,
                    "alt_geom": 30000,
                    "gs": 450.0,
                    "track": 180.0
                }
            ]
        }"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed.len(), 1);
        let p = &parsed[0];
        assert_eq!(p.icao24.as_str(), "ABCDEF");
        assert_eq!(p.latitude, 47.4);
        assert_eq!(p.longitude, 8.5);
        assert_eq!(p.altitude_ft, Some(30_000));
        assert_eq!(p.ground_speed_kt, Some(450.0));
        assert_eq!(p.track_deg, Some(180.0));
        assert_eq!(p.callsign.as_ref().unwrap().as_str(), "AFR990");
    }

    #[test]
    fn alt_baro_used_when_alt_geom_missing() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0, "alt_baro": 25000 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed[0].altitude_ft, Some(25_000));
    }

    #[test]
    fn alt_geom_preferred_when_both_present() {
        // Real dump1090 emits both fields on the same aircraft.
        // We prefer alt_geom (GNSS) and tolerate the duplicate without
        // erroring out the way the old strict parser did.
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0,
              "alt_geom": 25000, "alt_baro": 25200 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed[0].altitude_ft, Some(25_000));
    }

    #[test]
    fn ground_altitude_coerced_to_zero() {
        // Surface vehicles emit `"alt_baro": "ground"`; the strict
        // parser used to drop the entire batch on this.
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0, "alt_baro": "ground" }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed[0].altitude_ft, Some(0));
    }

    #[test]
    fn one_bad_aircraft_does_not_kill_the_batch() {
        // A real receiver dumps dozens of aircraft per poll; the old
        // parser would error on the *whole* batch if any one entry was
        // malformed. We now decode entries individually.
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0 },
            { "hex": "garbage", "lat": "not-a-number" },
            { "hex": "123456", "lat": 46.0, "lon": 7.0 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        let codes: Vec<_> = parsed
            .iter()
            .map(|p| p.icao24.as_str().to_owned())
            .collect();
        assert_eq!(codes, vec!["ABCDEF".to_string(), "123456".to_string()]);
    }

    #[test]
    fn altitude_as_float_is_rounded() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0, "alt_geom": 30000.6 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed[0].altitude_ft, Some(30_001));
    }

    #[test]
    fn drops_aircraft_without_position() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "flight": "AFR990" }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn drops_aircraft_with_only_latitude() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 47.0 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn skips_aircraft_with_invalid_icao24() {
        let body = r#"{ "aircraft": [
            { "hex": "ZZZZZZ", "lat": 47.0, "lon": 8.0 },
            { "hex": "abcdef", "lat": 47.0, "lon": 8.0 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].icao24.as_str(), "ABCDEF");
    }

    #[test]
    fn trims_callsign_whitespace() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "flight": "   ", "lat": 47.0, "lon": 8.0 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        // Empty callsign after trim → None.
        assert!(parsed[0].callsign.is_none());
    }

    #[test]
    fn out_of_range_latitude_drops_aircraft() {
        let body = r#"{ "aircraft": [
            { "hex": "abcdef", "lat": 91.0, "lon": 0.0 }
        ]}"#;
        let parsed = parse_aircraft_json(body, t()).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn missing_aircraft_key_returns_empty() {
        let body = r#"{ "now": 1700000000 }"#;
        assert!(parse_aircraft_json(body, t()).unwrap().is_empty());
    }

    #[test]
    fn malformed_json_returns_error() {
        let err = parse_aircraft_json("not json", t()).unwrap_err();
        assert!(matches!(err, RadarError::MalformedPayload(_)));
    }

    #[test]
    fn aircraft_url_handles_trailing_slash() {
        let s = Dump1090Source::new(Dump1090Config::new("http://radar.local/")).unwrap();
        assert_eq!(s.aircraft_url(), "http://radar.local/data/aircraft.json");
        let s = Dump1090Source::new(Dump1090Config::new("http://radar.local")).unwrap();
        assert_eq!(s.aircraft_url(), "http://radar.local/data/aircraft.json");
    }

    #[test]
    fn name_is_stable() {
        let s = Dump1090Source::new(Dump1090Config::new("http://x")).unwrap();
        assert_eq!(s.name(), "dump1090");
    }
}
