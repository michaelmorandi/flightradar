//! Aircraft ↔ BSON. The Aircraft `_id` is the ICAO24 string — naturally
//! unique, no synthetic key needed.

use bson::{doc, Document};

use flightradar_domain::{Aircraft, AircraftSource, Icao24};

use super::flight::read_opt_str;
use crate::error::CodecError;

pub fn aircraft_to_document(ac: &Aircraft) -> Document {
    let mut doc = doc! {
        "_id": ac.icao24.as_str(),
        "icao24": ac.icao24.as_str(),
    };
    if let Some(reg) = &ac.registration {
        doc.insert("registration", reg);
    }
    if let Some(tc) = &ac.type_code {
        doc.insert("type_code", tc);
    }
    if let Some(td) = &ac.type_description {
        doc.insert("type_description", td);
    }
    if let Some(op) = &ac.operator {
        doc.insert("operator", op);
    }
    if let Some(d) = &ac.designator {
        doc.insert("designator", d);
    }
    if let Some(s) = &ac.source {
        doc.insert("source", s.as_str());
    }
    doc
}

pub fn document_to_aircraft(doc: &Document) -> Result<Aircraft, CodecError> {
    let icao24_raw = doc
        .get_str("icao24")
        .map_err(|_| CodecError::MissingField("icao24"))?;
    let icao24 =
        Icao24::new(icao24_raw).map_err(|e| CodecError::InvalidValue("icao24", e.to_string()))?;

    let mut ac = Aircraft::new(icao24);
    ac.registration = read_opt_str(doc, "registration")?;
    ac.type_code = read_opt_str(doc, "type_code")?;
    ac.type_description = read_opt_str(doc, "type_description")?;
    ac.operator = read_opt_str(doc, "operator")?;
    ac.designator = read_opt_str(doc, "designator")?;
    ac.source = read_opt_str(doc, "source")?.map(AircraftSource::new);
    Ok(ac)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_aircraft() -> Aircraft {
        let mut ac = Aircraft::new(Icao24::new("ABCDEF").unwrap());
        ac.registration = Some("HB-JCS".into());
        ac.type_code = Some("A320".into());
        ac.type_description = Some("Airbus A320".into());
        ac.operator = Some("Swiss".into());
        ac.designator = Some("SWR".into());
        ac.source = Some(AircraftSource::new("nighthawk"));
        ac
    }

    #[test]
    fn roundtrip_full_aircraft() {
        let original = full_aircraft();
        let doc = aircraft_to_document(&original);
        let parsed = document_to_aircraft(&doc).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn empty_optional_fields_skipped_in_document() {
        let ac = Aircraft::new(Icao24::new("ABCDEF").unwrap());
        let doc = aircraft_to_document(&ac);
        assert!(!doc.contains_key("registration"));
        assert!(!doc.contains_key("operator"));
    }

    #[test]
    fn id_field_is_icao24() {
        let ac = full_aircraft();
        let doc = aircraft_to_document(&ac);
        assert_eq!(doc.get_str("_id").unwrap(), "ABCDEF");
    }

    #[test]
    fn missing_icao24_returns_codec_error() {
        let doc = doc! { "_id": "ABCDEF" };
        let err = document_to_aircraft(&doc).unwrap_err();
        assert!(matches!(err, CodecError::MissingField("icao24")));
    }

    #[test]
    fn invalid_icao24_returns_invalid_value() {
        let doc = doc! { "_id": "x", "icao24": "BAD" };
        let err = document_to_aircraft(&doc).unwrap_err();
        assert!(matches!(err, CodecError::InvalidValue("icao24", _)));
    }
}
