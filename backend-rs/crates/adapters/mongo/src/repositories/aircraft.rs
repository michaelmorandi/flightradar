use async_trait::async_trait;
use bson::{doc, Document};
use futures::stream::TryStreamExt;
use mongodb::options::ReplaceOptions;
use mongodb::{Collection, Database};

use flightradar_domain::ports::repositories::{AircraftRepository, RepoResult};
use flightradar_domain::{Aircraft, Icao24};

use crate::codec::aircraft::{aircraft_to_document, document_to_aircraft};
use crate::collections::AIRCRAFT;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoAircraftRepository {
    col: Collection<Document>,
}

impl MongoAircraftRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(AIRCRAFT),
        }
    }
}

#[async_trait]
impl AircraftRepository for MongoAircraftRepository {
    async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
        // Query by the indexed `icao24` field, not `_id` — this works
        // for both legacy ObjectId-keyed documents and new documents.
        let found = self
            .col
            .find_one(doc! { "icao24": icao24.as_str() })
            .await
            .map_err(map_mongo_error)?;
        match found {
            Some(d) => Ok(Some(document_to_aircraft(&d)?)),
            None => Ok(None),
        }
    }

    async fn find_many(&self, icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
        if icao24s.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<&str> = icao24s.iter().map(Icao24::as_str).collect();
        let cursor = self
            .col
            .find(doc! { "icao24": { "$in": ids } })
            .await
            .map_err(map_mongo_error)?;
        let docs: Vec<Document> = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(docs
            .iter()
            .map(document_to_aircraft)
            .collect::<Result<_, _>>()?)
    }

    async fn upsert(&self, aircraft: &Aircraft) -> RepoResult<()> {
        // Upsert keyed by the indexed `icao24` field. The document
        // body doesn't carry `_id`, so Mongo preserves whatever was
        // already there (or mints a fresh ObjectId on first insert).
        let doc = aircraft_to_document(aircraft);
        self.col
            .replace_one(doc! { "icao24": aircraft.icao24.as_str() }, doc)
            .with_options(ReplaceOptions::builder().upsert(true).build())
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }
}
