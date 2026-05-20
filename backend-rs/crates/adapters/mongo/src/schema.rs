//! Idempotent schema bootstrap. Creates collections (including time-series
//! `positions`) and indexes on first start; safe to call repeatedly.

use std::time::Duration;

use bson::doc;
use mongodb::options::{
    CreateCollectionOptions, IndexOptions, TimeseriesGranularity, TimeseriesOptions,
};
use mongodb::{Database, IndexModel};
use tracing::{debug, info};

use crate::collections::{AIRCRAFT, CRAWLER_LOGS, CRAWLER_QUEUE, FLIGHTS, POSITIONS, USERS};
use crate::error::map_mongo_error;
use flightradar_domain::ports::repositories::RepositoryError;

#[derive(Debug, Clone, Copy)]
pub struct SchemaConfig {
    /// Document retention for flights/positions. `None` disables the TTL.
    pub flight_retention: Option<Duration>,
    /// Retention for crawler logs.
    pub crawler_log_retention: Option<Duration>,
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            flight_retention: Some(Duration::from_secs(60 * 60 * 24)), // 24h
            crawler_log_retention: Some(Duration::from_secs(60 * 60 * 24 * 7)), // 7d
        }
    }
}

pub async fn ensure_schema(db: &Database, config: SchemaConfig) -> Result<(), RepositoryError> {
    ensure_collection(db, FLIGHTS, None).await?;
    ensure_time_series(db).await?;
    ensure_collection(db, AIRCRAFT, None).await?;
    ensure_collection(db, CRAWLER_QUEUE, None).await?;
    ensure_collection(db, CRAWLER_LOGS, None).await?;
    ensure_collection(db, USERS, None).await?;

    ensure_flight_indexes(db, config.flight_retention).await?;
    ensure_position_indexes(db).await?;
    ensure_aircraft_indexes(db).await?;
    ensure_crawler_indexes(db, config.crawler_log_retention).await?;
    ensure_user_indexes(db).await?;
    info!("mongo schema ensured");
    Ok(())
}

async fn ensure_collection(
    db: &Database,
    name: &str,
    opts: Option<CreateCollectionOptions>,
) -> Result<(), RepositoryError> {
    let existing = db.list_collection_names().await.map_err(map_mongo_error)?;
    if existing.iter().any(|n| n == name) {
        debug!(collection = name, "collection already present");
        return Ok(());
    }
    let mut req = db.create_collection(name);
    if let Some(o) = opts {
        req = req.with_options(o);
    }
    req.await.map_err(map_mongo_error)?;
    debug!(collection = name, "collection created");
    Ok(())
}

async fn ensure_time_series(db: &Database) -> Result<(), RepositoryError> {
    let ts = TimeseriesOptions::builder()
        .time_field("observed_at".to_string())
        .meta_field(Some("flight_id".to_string()))
        .granularity(Some(TimeseriesGranularity::Seconds))
        .build();
    let opts = CreateCollectionOptions::builder()
        .timeseries(Some(ts))
        .build();
    ensure_collection(db, POSITIONS, Some(opts)).await
}

async fn ensure_flight_indexes(
    db: &Database,
    retention: Option<Duration>,
) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(FLIGHTS);

    col.create_index(IndexModel::builder().keys(doc! { "icao24": 1 }).build())
        .await
        .map_err(map_mongo_error)?;
    col.create_index(
        IndexModel::builder()
            .keys(doc! { "last_contact": -1 })
            .build(),
    )
    .await
    .map_err(map_mongo_error)?;
    col.create_index(
        IndexModel::builder()
            .keys(doc! { "is_military": 1 })
            .build(),
    )
    .await
    .map_err(map_mongo_error)?;
    col.create_index(
        IndexModel::builder()
            .keys(doc! { "airline_icao": 1 })
            .build(),
    )
    .await
    .map_err(map_mongo_error)?;

    if let Some(ttl) = retention {
        col.create_index(
            IndexModel::builder()
                .keys(doc! { "last_contact": 1 })
                .options(IndexOptions::builder().expire_after(ttl).build())
                .build(),
        )
        .await
        .map_err(map_mongo_error)?;
    }

    Ok(())
}

async fn ensure_position_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(POSITIONS);
    col.create_index(IndexModel::builder().keys(doc! { "flight_id": 1 }).build())
        .await
        .map_err(map_mongo_error)?;
    col.create_index(
        IndexModel::builder()
            .keys(doc! { "flight_id": 1, "observed_at": 1 })
            .build(),
    )
    .await
    .map_err(map_mongo_error)?;
    Ok(())
}

async fn ensure_aircraft_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(AIRCRAFT);
    col.create_index(IndexModel::builder().keys(doc! { "icao24": 1 }).build())
        .await
        .map_err(map_mongo_error)?;
    Ok(())
}

async fn ensure_crawler_indexes(
    db: &Database,
    log_retention: Option<Duration>,
) -> Result<(), RepositoryError> {
    let queue = db.collection::<bson::Document>(CRAWLER_QUEUE);
    queue
        .create_index(
            IndexModel::builder()
                .keys(doc! { "last_attempt_at": 1 })
                .build(),
        )
        .await
        .map_err(map_mongo_error)?;
    queue
        .create_index(IndexModel::builder().keys(doc! { "attempts": 1 }).build())
        .await
        .map_err(map_mongo_error)?;

    let logs = db.collection::<bson::Document>(CRAWLER_LOGS);
    logs.create_index(IndexModel::builder().keys(doc! { "icao24": 1 }).build())
        .await
        .map_err(map_mongo_error)?;
    if let Some(ttl) = log_retention {
        logs.create_index(
            IndexModel::builder()
                .keys(doc! { "recorded_at": 1 })
                .options(IndexOptions::builder().expire_after(ttl).build())
                .build(),
        )
        .await
        .map_err(map_mongo_error)?;
    }
    Ok(())
}

async fn ensure_user_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(USERS);
    col.create_index(
        IndexModel::builder()
            .keys(doc! { "email": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build(),
    )
    .await
    .map_err(map_mongo_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_set_retention_windows() {
        let cfg = SchemaConfig::default();
        assert!(cfg.flight_retention.is_some());
        assert!(cfg.crawler_log_retention.is_some());
    }

    #[test]
    fn retention_can_be_disabled() {
        let cfg = SchemaConfig {
            flight_retention: None,
            crawler_log_retention: None,
        };
        assert!(cfg.flight_retention.is_none());
        assert!(cfg.crawler_log_retention.is_none());
    }
}
