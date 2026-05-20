use async_trait::async_trait;
use bson::{doc, Document};
use futures::stream::TryStreamExt;
use mongodb::options::{FindOptions, ReplaceOptions, UpdateOptions};
use mongodb::{Collection, Database};

use flightradar_domain::ports::repositories::{
    CrawlerQueueEntry, CrawlerQueueRepository, RepoResult,
};
use flightradar_domain::Icao24;

use crate::codec::crawler::document_to_queue_entry;
use crate::codec::flight::unix_ms;
use crate::collections::CRAWLER_QUEUE;
use crate::error::map_mongo_error;

#[derive(Debug, Clone)]
pub struct MongoCrawlerQueueRepository {
    col: Collection<Document>,
}

impl MongoCrawlerQueueRepository {
    pub fn new(db: &Database) -> Self {
        Self {
            col: db.collection(CRAWLER_QUEUE),
        }
    }
}

#[async_trait]
impl CrawlerQueueRepository for MongoCrawlerQueueRepository {
    async fn enqueue(&self, icao24: &Icao24) -> RepoResult<()> {
        let id = icao24.as_str();
        // Upsert-on-insert: only seed `attempts: 0` if the document is new.
        // Existing entries keep their attempts count.
        self.col
            .update_one(
                doc! { "_id": id },
                doc! { "$setOnInsert": { "icao24": id, "attempts": 0_i64 } },
            )
            .with_options(UpdateOptions::builder().upsert(true).build())
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }

    async fn next_batch(&self, batch_size: u32) -> RepoResult<Vec<CrawlerQueueEntry>> {
        let limit = i64::from(batch_size.max(1));
        let opts = FindOptions::builder()
            .sort(doc! { "last_attempt_at": 1, "attempts": 1 })
            .limit(Some(limit))
            .build();
        let cursor = self
            .col
            .find(Document::new())
            .with_options(opts)
            .await
            .map_err(map_mongo_error)?;
        let docs: Vec<Document> = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(docs
            .iter()
            .map(document_to_queue_entry)
            .collect::<Result<_, _>>()?)
    }

    async fn record_attempt(&self, icao24: &Icao24, success: bool) -> RepoResult<()> {
        if success {
            // Successful crawl → drop from the queue.
            self.col
                .delete_one(doc! { "_id": icao24.as_str() })
                .await
                .map_err(map_mongo_error)?;
            return Ok(());
        }

        let now = time::OffsetDateTime::now_utc();
        let upsert = doc! {
            "_id": icao24.as_str(),
            "icao24": icao24.as_str(),
            "attempts": 1_i64,
            "last_attempt_at": bson::DateTime::from_millis(unix_ms(now)),
        };
        // Try to bump attempts; fall back to insert if the document is gone.
        let res = self
            .col
            .update_one(
                doc! { "_id": icao24.as_str() },
                doc! {
                    "$inc": { "attempts": 1_i64 },
                    "$set": { "last_attempt_at": bson::DateTime::from_millis(unix_ms(now)) },
                },
            )
            .await
            .map_err(map_mongo_error)?;
        if res.matched_count == 0 {
            self.col
                .replace_one(doc! { "_id": icao24.as_str() }, upsert)
                .with_options(ReplaceOptions::builder().upsert(true).build())
                .await
                .map_err(map_mongo_error)?;
        }
        Ok(())
    }
}
