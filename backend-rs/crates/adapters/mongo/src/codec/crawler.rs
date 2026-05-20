//! Crawler queue / log ↔ BSON.

use bson::{doc, Document};

use flightradar_domain::ports::repositories::{CrawlerLogEntry, CrawlerQueueEntry};
use flightradar_domain::Icao24;

use super::flight::{read_datetime, unix_ms};
use crate::error::CodecError;

// ---------------------------------------------------------------------------
// Crawler queue
// ---------------------------------------------------------------------------

pub fn queue_entry_to_document(entry: &CrawlerQueueEntry) -> Document {
    let mut doc = doc! {
        "_id": entry.icao24.as_str(),
        "icao24": entry.icao24.as_str(),
        "attempts": i64::from(entry.attempts),
    };
    if let Some(t) = entry.last_attempt_at {
        doc.insert("last_attempt_at", bson::DateTime::from_millis(unix_ms(t)));
    }
    doc
}

pub fn document_to_queue_entry(doc: &Document) -> Result<CrawlerQueueEntry, CodecError> {
    let icao24_raw = doc
        .get_str("icao24")
        .map_err(|_| CodecError::MissingField("icao24"))?;
    let icao24 =
        Icao24::new(icao24_raw).map_err(|e| CodecError::InvalidValue("icao24", e.to_string()))?;
    let attempts = doc.get_i64("attempts").unwrap_or(0);
    let attempts = u32::try_from(attempts.max(0)).unwrap_or(0);
    let last_attempt_at = match doc.get_datetime("last_attempt_at") {
        Ok(_) => Some(read_datetime(doc, "last_attempt_at")?),
        Err(_) => None,
    };
    Ok(CrawlerQueueEntry {
        icao24,
        attempts,
        last_attempt_at,
    })
}

// ---------------------------------------------------------------------------
// Crawler log
// ---------------------------------------------------------------------------

pub fn log_entry_to_document(entry: &CrawlerLogEntry) -> Document {
    doc! {
        "icao24": entry.icao24.as_str(),
        "source": &entry.source,
        "success": entry.success,
        "recorded_at": bson::DateTime::from_millis(unix_ms(entry.recorded_at)),
    }
}

pub fn document_to_log_entry(doc: &Document) -> Result<CrawlerLogEntry, CodecError> {
    let icao24_raw = doc
        .get_str("icao24")
        .map_err(|_| CodecError::MissingField("icao24"))?;
    let icao24 =
        Icao24::new(icao24_raw).map_err(|e| CodecError::InvalidValue("icao24", e.to_string()))?;
    let source = doc
        .get_str("source")
        .map_err(|_| CodecError::MissingField("source"))?
        .to_owned();
    let success = doc
        .get_bool("success")
        .map_err(|_| CodecError::MissingField("success"))?;
    let recorded_at = read_datetime(doc, "recorded_at")?;
    Ok(CrawlerLogEntry {
        icao24,
        source,
        success,
        recorded_at,
    })
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    fn icao() -> Icao24 {
        Icao24::new("ABCDEF").unwrap()
    }

    #[test]
    fn queue_entry_roundtrip_full() {
        let entry = CrawlerQueueEntry {
            icao24: icao(),
            attempts: 3,
            last_attempt_at: Some(t(60)),
        };
        let doc = queue_entry_to_document(&entry);
        let parsed = document_to_queue_entry(&doc).unwrap();
        assert_eq!(parsed.icao24, entry.icao24);
        assert_eq!(parsed.attempts, entry.attempts);
        assert_eq!(parsed.last_attempt_at, entry.last_attempt_at);
    }

    #[test]
    fn queue_entry_without_last_attempt_omits_field() {
        let entry = CrawlerQueueEntry {
            icao24: icao(),
            attempts: 0,
            last_attempt_at: None,
        };
        let doc = queue_entry_to_document(&entry);
        assert!(!doc.contains_key("last_attempt_at"));
    }

    #[test]
    fn log_entry_roundtrip() {
        let entry = CrawlerLogEntry {
            icao24: icao(),
            source: "nighthawk".into(),
            success: true,
            recorded_at: t(10),
        };
        let doc = log_entry_to_document(&entry);
        let parsed = document_to_log_entry(&doc).unwrap();
        assert_eq!(parsed.icao24, entry.icao24);
        assert_eq!(parsed.source, entry.source);
        assert_eq!(parsed.success, entry.success);
        assert_eq!(parsed.recorded_at, entry.recorded_at);
    }

    #[test]
    fn log_entry_missing_source_is_codec_error() {
        let doc = doc! {
            "icao24": "ABCDEF",
            "success": true,
            "recorded_at": bson::DateTime::now(),
        };
        let err = document_to_log_entry(&doc).unwrap_err();
        assert!(matches!(err, CodecError::MissingField("source")));
    }

    #[test]
    fn queue_entry_id_is_icao24() {
        let entry = CrawlerQueueEntry {
            icao24: icao(),
            attempts: 1,
            last_attempt_at: None,
        };
        assert_eq!(
            queue_entry_to_document(&entry).get_str("_id").unwrap(),
            "ABCDEF"
        );
    }

    #[test]
    fn negative_attempts_clamped_to_zero() {
        let doc = doc! { "icao24": "ABCDEF", "attempts": -5_i64 };
        assert_eq!(document_to_queue_entry(&doc).unwrap().attempts, 0);
    }
}
