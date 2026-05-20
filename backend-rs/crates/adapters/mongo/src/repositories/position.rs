use async_trait::async_trait;
use bson::{doc, oid::ObjectId, Document};
use futures::stream::TryStreamExt;
use mongodb::options::FindOptions;
use mongodb::{Collection, Database};

use flightradar_domain::ports::repositories::{PositionRepository, RepoResult, RepositoryError};
use flightradar_domain::{FlightId, PositionReport};

use crate::codec::position::{document_to_position, position_to_document};
use crate::collections::POSITIONS;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoPositionRepository {
    col: Collection<Document>,
}

impl MongoPositionRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(POSITIONS),
        }
    }
}

#[async_trait]
impl PositionRepository for MongoPositionRepository {
    async fn append(&self, flight_id: &FlightId, pr: &PositionReport) -> RepoResult<()> {
        let doc = position_to_document(flight_id, pr)?;
        self.col.insert_one(doc).await.map_err(map_mongo_error)?;
        Ok(())
    }

    async fn append_batch(&self, entries: &[(FlightId, PositionReport)]) -> RepoResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let docs: Vec<Document> = entries
            .iter()
            .map(|(id, pr)| position_to_document(id, pr))
            .collect::<Result<_, _>>()?;
        self.col.insert_many(docs).await.map_err(map_mongo_error)?;
        Ok(())
    }

    async fn history(&self, flight_id: &FlightId) -> RepoResult<Vec<PositionReport>> {
        let oid = ObjectId::parse_str(flight_id.as_str()).map_err(|_| RepositoryError::NotFound)?;
        let opts = FindOptions::builder()
            .sort(doc! { "observed_at": 1 })
            .build();
        let cursor = self
            .col
            .find(doc! { "flight_id": oid })
            .with_options(opts)
            .await
            .map_err(map_mongo_error)?;
        let docs: Vec<Document> = cursor.try_collect().await.map_err(map_mongo_error)?;
        let positions: Vec<PositionReport> = docs
            .iter()
            .map(document_to_position)
            .collect::<Result<_, _>>()?;
        Ok(positions)
    }
}

// No further unit-testable surface here: every method is a thin wrapper
// around the (already-tested) codec + a Mongo call. Integration tests
// covering this live in `tests/integration_*.rs` (gated on a live Mongo).
