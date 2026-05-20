use async_trait::async_trait;
use bson::{doc, Document};
use futures::stream::TryStreamExt;
use mongodb::options::FindOptions;
use mongodb::{Collection, Database};

use flightradar_domain::ports::repositories::{CrawlerLogEntry, CrawlerLogRepository, RepoResult};
use flightradar_domain::Icao24;

use crate::codec::crawler::{document_to_log_entry, log_entry_to_document};
use crate::collections::CRAWLER_LOGS;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoCrawlerLogRepository {
    col: Collection<Document>,
}

impl MongoCrawlerLogRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(CRAWLER_LOGS),
        }
    }
}

#[async_trait]
impl CrawlerLogRepository for MongoCrawlerLogRepository {
    async fn record(&self, entry: &CrawlerLogEntry) -> RepoResult<()> {
        let doc = log_entry_to_document(entry);
        self.col.insert_one(doc).await.map_err(map_mongo_error)?;
        Ok(())
    }

    async fn recent_for(&self, icao24: &Icao24, limit: u32) -> RepoResult<Vec<CrawlerLogEntry>> {
        let opts = FindOptions::builder()
            .sort(doc! { "recorded_at": -1 })
            .limit(Some(i64::from(limit.max(1))))
            .build();
        let cursor = self
            .col
            .find(doc! { "icao24": icao24.as_str() })
            .with_options(opts)
            .await
            .map_err(map_mongo_error)?;
        let docs: Vec<Document> = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(docs
            .iter()
            .map(document_to_log_entry)
            .collect::<Result<_, _>>()?)
    }
}
