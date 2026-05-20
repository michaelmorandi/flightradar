//! Flight ↔ BSON.

use bson::{doc, oid::ObjectId, Document};
use time::OffsetDateTime;

use flightradar_domain::{AirlineIcao, Callsign, Flight, FlightId, Icao24};

use crate::error::CodecError;

pub fn flight_to_document(flight: &Flight) -> Result<Document, CodecError> {
    let id = ObjectId::parse_str(flight.id.as_str()).unwrap_or_else(|_| ObjectId::new()); // accept domain-generated IDs by minting an ObjectId

    let mut doc = doc! {
        "_id": id,
        "icao24": flight.icao24.as_str(),
        "is_military": flight.is_military,
        "first_contact": bson::DateTime::from_millis(
            unix_ms(flight.first_contact)
        ),
        "last_contact": bson::DateTime::from_millis(
            unix_ms(flight.last_contact)
        ),
    };
    if let Some(cs) = &flight.callsign {
        doc.insert("callsign", cs.as_str());
    }
    if let Some(al) = &flight.airline_icao {
        doc.insert("airline_icao", al.as_str());
    }
    Ok(doc)
}

pub fn document_to_flight(doc: &Document) -> Result<Flight, CodecError> {
    let id = doc
        .get_object_id("_id")
        .map_err(|_| CodecError::MissingField("_id"))?;
    let icao24_raw = doc
        .get_str("icao24")
        .map_err(|_| CodecError::MissingField("icao24"))?;
    let icao24 =
        Icao24::new(icao24_raw).map_err(|e| CodecError::InvalidValue("icao24", e.to_string()))?;
    let is_military = doc.get_bool("is_military").unwrap_or(false);

    let first_contact = read_datetime(doc, "first_contact")?;
    let last_contact = read_datetime(doc, "last_contact")?;

    let callsign = read_opt_str(doc, "callsign")?
        .map(|s| Callsign::new(&s))
        .transpose()
        .map_err(|e| CodecError::InvalidValue("callsign", e.to_string()))?;

    let airline_icao = read_opt_str(doc, "airline_icao")?
        .map(|s| AirlineIcao::new(&s))
        .transpose()
        .map_err(|e| CodecError::InvalidValue("airline_icao", e.to_string()))?;

    Ok(Flight {
        id: FlightId::new(id.to_hex()),
        icao24,
        callsign,
        airline_icao,
        is_military,
        first_contact,
        last_contact,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn unix_ms(dt: OffsetDateTime) -> i64 {
    let secs = dt.unix_timestamp();
    let nanos = i64::from(dt.nanosecond());
    secs.saturating_mul(1000) + nanos / 1_000_000
}

pub(crate) fn read_datetime(
    doc: &Document,
    field: &'static str,
) -> Result<OffsetDateTime, CodecError> {
    let bson_dt = doc
        .get_datetime(field)
        .map_err(|_| CodecError::MissingField(field))?;
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(bson_dt.timestamp_millis()) * 1_000_000)
        .map_err(|e| CodecError::InvalidValue(field, e.to_string()))
}

pub(crate) fn read_opt_str(doc: &Document, field: &str) -> Result<Option<String>, CodecError> {
    match doc.get(field) {
        None | Some(bson::Bson::Null) => Ok(None),
        Some(bson::Bson::String(s)) => Ok(Some(s.clone())),
        Some(_) => {
            // Field exists but isn't a string — surface as a typed error.
            let owned: &'static str = match field {
                "icao24" => "icao24",
                "callsign" => "callsign",
                "airline_icao" => "airline_icao",
                "registration" => "registration",
                "type_code" => "type_code",
                "type_description" => "type_description",
                "operator" => "operator",
                "designator" => "designator",
                "source" => "source",
                "email" => "email",
                "display_name" => "display_name",
                "password_hash" => "password_hash",
                _ => "string field",
            };
            Err(CodecError::TypeMismatch(owned))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    fn flight() -> Flight {
        Flight {
            id: FlightId::new(ObjectId::new().to_hex()),
            icao24: Icao24::new("ABCDEF").unwrap(),
            callsign: Some(Callsign::new("AFR990").unwrap()),
            airline_icao: Some(AirlineIcao::new("AFR").unwrap()),
            is_military: false,
            first_contact: t(0),
            last_contact: t(60),
        }
    }

    #[test]
    fn roundtrip_flight_preserves_all_fields() {
        let original = flight();
        let doc = flight_to_document(&original).unwrap();
        let parsed = document_to_flight(&doc).unwrap();
        assert_eq!(parsed.icao24, original.icao24);
        assert_eq!(parsed.callsign, original.callsign);
        assert_eq!(parsed.airline_icao, original.airline_icao);
        assert_eq!(parsed.is_military, original.is_military);
        assert_eq!(parsed.first_contact, original.first_contact);
        assert_eq!(parsed.last_contact, original.last_contact);
    }

    #[test]
    fn callsign_and_airline_icao_optional_when_missing() {
        let mut f = flight();
        f.callsign = None;
        f.airline_icao = None;
        let doc = flight_to_document(&f).unwrap();
        assert!(!doc.contains_key("callsign"));
        assert!(!doc.contains_key("airline_icao"));
        let parsed = document_to_flight(&doc).unwrap();
        assert!(parsed.callsign.is_none());
        assert!(parsed.airline_icao.is_none());
    }

    #[test]
    fn missing_required_field_returns_codec_error() {
        let doc = doc! { "_id": ObjectId::new(), "icao24": "ABCDEF" };
        let err = document_to_flight(&doc).unwrap_err();
        assert!(matches!(err, CodecError::MissingField(_)));
    }

    #[test]
    fn invalid_icao24_in_document_returns_invalid_value() {
        let now = bson::DateTime::now();
        let doc = doc! {
            "_id": ObjectId::new(),
            "icao24": "NOT-HEX",
            "is_military": false,
            "first_contact": now,
            "last_contact": now,
        };
        let err = document_to_flight(&doc).unwrap_err();
        assert!(matches!(err, CodecError::InvalidValue("icao24", _)));
    }

    #[test]
    fn non_objectid_flight_id_falls_back_to_new_oid() {
        let mut f = flight();
        f.id = FlightId::new("not-an-objectid");
        let doc = flight_to_document(&f).unwrap();
        // _id should still be a valid ObjectId
        assert!(doc.get_object_id("_id").is_ok());
    }

    #[test]
    fn read_opt_str_returns_none_on_missing_and_null() {
        let doc = doc! { "x": bson::Bson::Null };
        assert_eq!(read_opt_str(&doc, "x").unwrap(), None);
        assert_eq!(read_opt_str(&doc, "y").unwrap(), None);
    }

    #[test]
    fn read_opt_str_rejects_wrong_type() {
        let doc = doc! { "callsign": 42_i32 };
        let err = read_opt_str(&doc, "callsign").unwrap_err();
        assert!(matches!(err, CodecError::TypeMismatch("callsign")));
    }

    #[test]
    fn unix_ms_handles_fractional_seconds() {
        let dt = OffsetDateTime::from_unix_timestamp_nanos(1_700_000_000_500_000_000).unwrap();
        // 500ms past the epoch second
        assert_eq!(unix_ms(dt), 1_700_000_000_500);
    }
}
