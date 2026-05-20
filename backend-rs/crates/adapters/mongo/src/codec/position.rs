//! Position ↔ BSON. Designed for a time-series collection where the meta
//! field is `flight_id` and the time field is `observed_at`.

use bson::{doc, oid::ObjectId, Document};

use flightradar_domain::{FlightId, Icao24, PositionReport};

use super::flight::{read_datetime, unix_ms};
use crate::error::CodecError;

pub fn position_to_document(
    flight_id: &FlightId,
    pr: &PositionReport,
) -> Result<Document, CodecError> {
    let id = ObjectId::parse_str(flight_id.as_str())
        .map_err(|_| CodecError::InvalidValue("flight_id", flight_id.as_str().to_string()))?;

    let mut doc = doc! {
        "flight_id": id,
        "icao24": pr.icao24.as_str(),
        "lat": pr.latitude,
        "lon": pr.longitude,
        "observed_at": bson::DateTime::from_millis(unix_ms(pr.observed_at)),
    };
    if let Some(alt) = pr.altitude_ft {
        doc.insert("alt_ft", i64::from(alt));
    }
    if let Some(gs) = pr.ground_speed_kt {
        doc.insert("ground_speed_kt", gs);
    }
    if let Some(track) = pr.track_deg {
        doc.insert("track_deg", track);
    }
    if let Some(cs) = &pr.callsign {
        doc.insert("callsign", cs.as_str());
    }
    if let Some(cat) = pr.category {
        doc.insert("category", i32::from(cat.as_u8()));
    }
    Ok(doc)
}

pub fn document_to_position(doc: &Document) -> Result<PositionReport, CodecError> {
    let icao24_raw = doc
        .get_str("icao24")
        .map_err(|_| CodecError::MissingField("icao24"))?;
    let icao24 =
        Icao24::new(icao24_raw).map_err(|e| CodecError::InvalidValue("icao24", e.to_string()))?;
    let lat = doc
        .get_f64("lat")
        .map_err(|_| CodecError::MissingField("lat"))?;
    let lon = doc
        .get_f64("lon")
        .map_err(|_| CodecError::MissingField("lon"))?;
    let observed_at = read_datetime(doc, "observed_at")?;

    let mut pr = PositionReport::new(icao24, lat, lon, observed_at)
        .map_err(|e| CodecError::InvalidValue("position", e.to_string()))?;

    if let Ok(alt) = doc.get_i64("alt_ft") {
        pr.altitude_ft = i32::try_from(alt).ok();
    } else if let Ok(alt) = doc.get_i32("alt_ft") {
        pr.altitude_ft = Some(alt);
    }
    if let Ok(gs) = doc.get_f64("ground_speed_kt") {
        pr.ground_speed_kt = Some(gs);
    }
    if let Ok(t) = doc.get_f64("track_deg") {
        pr.track_deg = Some(t);
    }
    if let Ok(cs) = doc.get_str("callsign") {
        pr.callsign = Some(
            flightradar_domain::Callsign::new(cs)
                .map_err(|e| CodecError::InvalidValue("callsign", e.to_string()))?,
        );
    }
    if let Ok(cat) = doc.get_i32("category") {
        let v =
            u8::try_from(cat).map_err(|_| CodecError::InvalidValue("category", cat.to_string()))?;
        let category = flightradar_domain::AircraftCategory::try_from(v)
            .map_err(|e| CodecError::InvalidValue("category", e.to_string()))?;
        pr.category = Some(category);
    }
    Ok(pr)
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use flightradar_domain::{AircraftCategory, Callsign};

    use super::*;

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    fn flight_id() -> FlightId {
        FlightId::new(ObjectId::new().to_hex())
    }

    fn pr() -> PositionReport {
        let mut pr = PositionReport::new(Icao24::new("ABCDEF").unwrap(), 47.0, 8.0, t(0)).unwrap();
        pr.altitude_ft = Some(30_000);
        pr.ground_speed_kt = Some(420.0);
        pr.track_deg = Some(180.0);
        pr.callsign = Some(Callsign::new("AFR990").unwrap());
        pr.category = Some(AircraftCategory::Medium1);
        pr
    }

    #[test]
    fn roundtrip_position_preserves_all_fields() {
        let id = flight_id();
        let original = pr();
        let doc = position_to_document(&id, &original).unwrap();
        let parsed = document_to_position(&doc).unwrap();
        assert_eq!(parsed.icao24, original.icao24);
        assert!((parsed.latitude - original.latitude).abs() < 1e-9);
        assert!((parsed.longitude - original.longitude).abs() < 1e-9);
        assert_eq!(parsed.altitude_ft, original.altitude_ft);
        assert_eq!(parsed.ground_speed_kt, original.ground_speed_kt);
        assert_eq!(parsed.track_deg, original.track_deg);
        assert_eq!(parsed.callsign, original.callsign);
        assert_eq!(parsed.category, original.category);
        assert_eq!(parsed.observed_at, original.observed_at);
    }

    #[test]
    fn optional_fields_left_out_of_document() {
        let id = flight_id();
        let pr = PositionReport::new(Icao24::new("ABCDEF").unwrap(), 0.0, 0.0, t(0)).unwrap();
        let doc = position_to_document(&id, &pr).unwrap();
        assert!(!doc.contains_key("alt_ft"));
        assert!(!doc.contains_key("ground_speed_kt"));
        assert!(!doc.contains_key("track_deg"));
        assert!(!doc.contains_key("callsign"));
        assert!(!doc.contains_key("category"));
    }

    #[test]
    fn non_objectid_flight_id_is_rejected() {
        let bad = FlightId::new("zzzz");
        let err = position_to_document(&bad, &pr()).unwrap_err();
        assert!(matches!(err, CodecError::InvalidValue("flight_id", _)));
    }

    #[test]
    fn missing_lat_is_codec_error() {
        let doc = doc! {
            "icao24": "ABCDEF",
            "lon": 8.0_f64,
            "observed_at": bson::DateTime::now(),
        };
        let err = document_to_position(&doc).unwrap_err();
        assert!(matches!(err, CodecError::MissingField("lat")));
    }

    #[test]
    fn altitude_can_be_stored_as_i32_or_i64() {
        let mut doc = doc! {
            "icao24": "ABCDEF",
            "lat": 0.0_f64,
            "lon": 0.0_f64,
            "observed_at": bson::DateTime::now(),
            "alt_ft": 30_000_i32,
        };
        assert_eq!(
            document_to_position(&doc).unwrap().altitude_ft,
            Some(30_000)
        );
        doc.insert("alt_ft", 30_000_i64);
        assert_eq!(
            document_to_position(&doc).unwrap().altitude_ft,
            Some(30_000)
        );
    }
}
