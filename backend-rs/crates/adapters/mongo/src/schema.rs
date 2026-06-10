//! Idempotent schema bootstrap. Creates collections (including time-series
//! `positions`) and indexes on first start; safe to call repeatedly.
//!
//! Tolerates legacy databases: an existing non-TTL index over the same key
//! pattern is left alone with a warning instead of conflicting on
//! `IndexOptionsConflict`. Operators who want to migrate to TTL semantics
//! drop the old index manually.

use std::time::Duration;

use bson::doc;
use mongodb::error::{Error as MongoError, ErrorKind};
use mongodb::options::{
    CreateCollectionOptions, IndexOptions, TimeseriesGranularity, TimeseriesOptions,
};
use mongodb::{Database, IndexModel};
use tracing::{debug, info, warn};

use crate::collections::{AIRCRAFT, CRAWLER_LOGS, CRAWLER_QUEUE, FLIGHTS, POSITIONS, USERS};
use crate::error::map_mongo_error;
use flightradar_domain::ports::repositories::RepositoryError;

#[derive(Debug, Clone, Copy)]
pub struct SchemaConfig {
    /// Retention for flights (drives TTL on `last_contact`). `None`
    /// keeps documents forever.
    pub flight_retention: Option<Duration>,
    /// Retention for the position time-series. `None` keeps forever.
    pub position_retention: Option<Duration>,
    /// Retention for crawler logs.
    pub crawler_log_retention: Option<Duration>,
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            flight_retention: Some(Duration::from_secs(60 * 60 * 24)), // 24h
            position_retention: Some(Duration::from_secs(60 * 60 * 24)), // 24h
            crawler_log_retention: Some(Duration::from_secs(60 * 60 * 24 * 7)), // 7d
        }
    }
}

pub async fn ensure_schema(db: &Database, config: SchemaConfig) -> Result<(), RepositoryError> {
    ensure_collection(db, FLIGHTS, None).await?;
    ensure_time_series(db, config.position_retention).await?;
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

async fn ensure_time_series(
    db: &Database,
    retention: Option<Duration>,
) -> Result<(), RepositoryError> {
    let ts = TimeseriesOptions::builder()
        .time_field("observed_at".to_string())
        .meta_field(Some("flight_id".to_string()))
        .granularity(Some(TimeseriesGranularity::Seconds))
        .build();
    // Time-series TTL is set at collection-create time as
    // `expireAfterSeconds`. It can't be changed after the fact — the
    // operator drops the collection to switch retention windows.
    let opts = CreateCollectionOptions::builder()
        .timeseries(Some(ts))
        .expire_after_seconds(retention)
        .build();
    ensure_collection(db, POSITIONS, Some(opts)).await
}

/// Create an index, tolerating conflicts with a pre-existing index on
/// the same key pattern. Anything else fails hard.
async fn create_or_tolerate_index(
    col: &mongodb::Collection<bson::Document>,
    model: IndexModel,
    name: &str,
) -> Result<(), RepositoryError> {
    match col.create_index(model).await {
        Ok(_) => Ok(()),
        Err(e) if is_index_conflict(&e) => {
            warn!(
                index = name,
                "existing index has different options — leaving in place"
            );
            Ok(())
        }
        Err(e) => Err(map_mongo_error(e)),
    }
}

fn is_index_conflict(err: &MongoError) -> bool {
    // Mongo returns IndexOptionsConflict (85) when the key pattern matches
    // but options (eg. TTL) differ, and IndexKeySpecsConflict (86) when
    // names differ. Either way the right thing is to leave the existing
    // index alone in an idempotent bootstrap.
    if let ErrorKind::Command(cmd) = &*err.kind {
        return cmd.code == 85 || cmd.code == 86;
    }
    let msg = err.to_string();
    msg.contains("IndexOptionsConflict") || msg.contains("IndexKeySpecsConflict")
}

async fn ensure_flight_indexes(
    db: &Database,
    retention: Option<Duration>,
) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(FLIGHTS);

    create_or_tolerate_index(
        &col,
        IndexModel::builder().keys(doc! { "icao24": 1 }).build(),
        "flights.icao24",
    )
    .await?;
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "last_contact": -1 })
            .build(),
        "flights.last_contact_desc",
    )
    .await?;
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "is_military": 1 })
            .build(),
        "flights.is_military",
    )
    .await?;
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "airline_icao": 1 })
            .build(),
        "flights.airline_icao",
    )
    .await?;

    if let Some(ttl) = retention {
        create_or_tolerate_index(
            &col,
            IndexModel::builder()
                .keys(doc! { "last_contact": 1 })
                .options(IndexOptions::builder().expire_after(ttl).build())
                .build(),
            "flights.last_contact_ttl",
        )
        .await?;
    }

    Ok(())
}

async fn ensure_position_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(POSITIONS);
    create_or_tolerate_index(
        &col,
        IndexModel::builder().keys(doc! { "flight_id": 1 }).build(),
        "positions.flight_id",
    )
    .await?;
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "flight_id": 1, "observed_at": 1 })
            .build(),
        "positions.flight_id_observed_at",
    )
    .await?;
    Ok(())
}

async fn ensure_aircraft_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(AIRCRAFT);
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "icao24": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build(),
        "aircraft.icao24",
    )
    .await
}

async fn ensure_crawler_indexes(
    db: &Database,
    log_retention: Option<Duration>,
) -> Result<(), RepositoryError> {
    let queue = db.collection::<bson::Document>(CRAWLER_QUEUE);
    create_or_tolerate_index(
        &queue,
        IndexModel::builder()
            .keys(doc! { "last_attempt_at": 1 })
            .build(),
        "queue.last_attempt_at",
    )
    .await?;
    create_or_tolerate_index(
        &queue,
        IndexModel::builder().keys(doc! { "attempts": 1 }).build(),
        "queue.attempts",
    )
    .await?;

    let logs = db.collection::<bson::Document>(CRAWLER_LOGS);
    create_or_tolerate_index(
        &logs,
        IndexModel::builder().keys(doc! { "icao24": 1 }).build(),
        "logs.icao24",
    )
    .await?;
    if let Some(ttl) = log_retention {
        create_or_tolerate_index(
            &logs,
            IndexModel::builder()
                .keys(doc! { "recorded_at": 1 })
                .options(IndexOptions::builder().expire_after(ttl).build())
                .build(),
            "logs.recorded_at_ttl",
        )
        .await?;
    }
    Ok(())
}

async fn ensure_user_indexes(db: &Database) -> Result<(), RepositoryError> {
    let col = db.collection::<bson::Document>(USERS);
    create_or_tolerate_index(
        &col,
        IndexModel::builder()
            .keys(doc! { "email": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build(),
        "users.email",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_set_retention_windows() {
        let cfg = SchemaConfig::default();
        assert!(cfg.flight_retention.is_some());
        assert!(cfg.position_retention.is_some());
        assert!(cfg.crawler_log_retention.is_some());
    }

    #[test]
    fn retention_can_be_disabled() {
        let cfg = SchemaConfig {
            flight_retention: None,
            position_retention: None,
            crawler_log_retention: None,
        };
        assert!(cfg.flight_retention.is_none());
        assert!(cfg.position_retention.is_none());
        assert!(cfg.crawler_log_retention.is_none());
    }
}
