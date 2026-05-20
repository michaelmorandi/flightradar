use async_trait::async_trait;
use bson::{doc, oid::ObjectId, Document};
use futures::stream::TryStreamExt;
use mongodb::options::{FindOneOptions, FindOptions, ReplaceOptions};
use mongodb::{Collection, Database};

use flightradar_domain::ports::repositories::{
    FlightFilter, FlightRepository, Page, PageRequest, RepoResult, RepositoryError,
};
use flightradar_domain::{Flight, FlightId, Icao24};

use crate::codec::flight::{document_to_flight, flight_to_document};
use crate::collections::FLIGHTS;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoFlightRepository {
    col: Collection<Document>,
}

impl MongoFlightRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(FLIGHTS),
        }
    }

    fn build_filter(filter: &FlightFilter) -> Document {
        let mut q = Document::new();
        if let Some(icao) = &filter.icao24 {
            q.insert("icao24", icao.as_str());
        }
        if let Some(al) = &filter.airline {
            q.insert("airline_icao", al.as_str());
        }
        if filter.military_only {
            q.insert("is_military", true);
        }
        if let Some(since) = filter.exclude_live_since {
            q.insert(
                "last_contact",
                doc! { "$lt": bson::DateTime::from_millis(
                    crate::codec::flight::unix_ms(since)
                ) },
            );
        }
        if let Some(text) = filter
            .free_text
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            // Free-text matches on icao24 or callsign, case-insensitive.
            let escaped = regex::escape(text);
            q.insert(
                "$or",
                bson::Bson::Array(vec![
                    doc! { "icao24": { "$regex": &escaped, "$options": "i" } }.into(),
                    doc! { "callsign": { "$regex": &escaped, "$options": "i" } }.into(),
                ]),
            );
        }
        q
    }
}

#[async_trait]
impl FlightRepository for MongoFlightRepository {
    async fn upsert(&self, flight: &Flight) -> RepoResult<()> {
        let doc = flight_to_document(flight)?;
        let id = doc
            .get("_id")
            .cloned()
            .ok_or(RepositoryError::Conflict("flight missing _id".into()))?;
        self.col
            .replace_one(doc! { "_id": id }, doc)
            .with_options(ReplaceOptions::builder().upsert(true).build())
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }

    async fn find_by_id(&self, id: &FlightId) -> RepoResult<Flight> {
        let oid = ObjectId::parse_str(id.as_str()).map_err(|_| RepositoryError::NotFound)?;
        let doc = self
            .col
            .find_one(doc! { "_id": oid })
            .await
            .map_err(map_mongo_error)?
            .ok_or(RepositoryError::NotFound)?;
        Ok(document_to_flight(&doc)?)
    }

    async fn find_open_for_icao24(&self, icao24: &Icao24) -> RepoResult<Option<Flight>> {
        let opts = FindOneOptions::builder()
            .sort(doc! { "last_contact": -1 })
            .build();
        let found = self
            .col
            .find_one(doc! { "icao24": icao24.as_str() })
            .with_options(opts)
            .await
            .map_err(map_mongo_error)?;
        match found {
            Some(doc) => Ok(Some(document_to_flight(&doc)?)),
            None => Ok(None),
        }
    }

    async fn list(&self, filter: &FlightFilter, page: PageRequest) -> RepoResult<Page<Flight>> {
        let q = Self::build_filter(filter);
        let total = self
            .col
            .count_documents(q.clone())
            .await
            .map_err(map_mongo_error)?;
        let page_size = u64::from(page.page_size.max(1));
        let skip = u64::from(page.page.saturating_sub(1)) * page_size;
        let limit = i64::try_from(page_size).unwrap_or(i64::MAX);
        let opts = FindOptions::builder()
            .sort(doc! { "last_contact": -1 })
            .skip(Some(skip))
            .limit(Some(limit))
            .build();
        let cursor = self
            .col
            .find(q)
            .with_options(opts)
            .await
            .map_err(map_mongo_error)?;
        let docs: Vec<Document> = cursor.try_collect().await.map_err(map_mongo_error)?;
        let items: Vec<Flight> = docs
            .iter()
            .map(document_to_flight)
            .collect::<Result<_, _>>()?;
        Ok(Page {
            items,
            total,
            page: page.page,
            page_size: page.page_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_filter_with_icao24_only() {
        let filter = FlightFilter {
            icao24: Some(Icao24::new("ABCDEF").unwrap()),
            ..Default::default()
        };
        let q = MongoFlightRepository::build_filter(&filter);
        assert_eq!(q.get_str("icao24").unwrap(), "ABCDEF");
        assert!(!q.contains_key("airline_icao"));
        assert!(!q.contains_key("is_military"));
        assert!(!q.contains_key("$or"));
    }

    #[test]
    fn build_filter_military_only_sets_flag() {
        let filter = FlightFilter {
            military_only: true,
            ..Default::default()
        };
        let q = MongoFlightRepository::build_filter(&filter);
        assert!(q.get_bool("is_military").unwrap());
    }

    #[test]
    fn build_filter_empty_free_text_is_ignored() {
        let filter = FlightFilter {
            free_text: Some("   ".into()),
            ..Default::default()
        };
        let q = MongoFlightRepository::build_filter(&filter);
        assert!(!q.contains_key("$or"));
    }

    #[test]
    fn build_filter_escapes_regex_metacharacters() {
        let filter = FlightFilter {
            free_text: Some(".*hi*".into()),
            ..Default::default()
        };
        let q = MongoFlightRepository::build_filter(&filter);
        let arr = q.get_array("$or").unwrap();
        assert_eq!(arr.len(), 2);
        // First entry: icao24 regex with escaped pattern.
        let icao_clause = arr[0].as_document().unwrap();
        let regex_doc = icao_clause.get_document("icao24").unwrap();
        let pat = regex_doc.get_str("$regex").unwrap();
        assert!(pat.contains(r"\."));
        assert!(pat.contains(r"\*"));
    }
}
