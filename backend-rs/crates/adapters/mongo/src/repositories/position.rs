use async_trait::async_trait;
use bson::{doc, oid::ObjectId, Document};
use futures::stream::TryStreamExt;
use mongodb::options::{FindOptions, InsertManyOptions};
use mongodb::{Collection, Database};
use tracing::warn;

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

    /// Best-effort batch insert.
    ///
    /// Entries that fail to encode are logged and skipped — they don't
    /// poison the rest of the tick. The remaining documents are inserted
    /// with `ordered: false`, so a single rejected document doesn't
    /// abort the batch either. Returns `Ok(())` iff at least one entry
    /// was either skipped cleanly or accepted by Mongo.
    async fn append_batch(&self, entries: &[(FlightId, PositionReport)]) -> RepoResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut docs: Vec<Document> = Vec::with_capacity(entries.len());
        for (id, pr) in entries {
            match position_to_document(id, pr) {
                Ok(d) => docs.push(d),
                Err(err) => {
                    warn!(error = %err, flight_id = %id.as_str(), "skipping unencodable position");
                }
            }
        }
        if docs.is_empty() {
            return Ok(());
        }
        let opts = InsertManyOptions::builder().ordered(false).build();
        if let Err(err) = self.col.insert_many(docs).with_options(opts).await {
            // bulk write errors are common (eg. a single duplicate timestamp).
            // Surface them as a warning, not a hard failure, so the next tick
            // can keep going.
            warn!(error = %err, "position batch insert had failures");
        }
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
