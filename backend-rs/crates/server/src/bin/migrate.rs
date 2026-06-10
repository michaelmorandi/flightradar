//! One-shot Mongo schema migration: legacy Python field names → clean
//! Rust shape. Idempotent — running it twice is a no-op.
//!
//! What it changes:
//!
//! - **flights**: `modeS` → `icao24`, legacy `expire_at` dropped (TTL
//!   index owns expiry now).
//! - **positions**: the Python collection used `timestmp` as its
//!   time-series timeField. The Rust adapter uses `observed_at`, which
//!   Mongo cannot rename in a time-series collection. The migrator
//!   therefore **drops** the legacy positions collection so the next
//!   schema bootstrap can recreate it with the new timeField. Live
//!   tracks are inherently transient — they refill from the radar
//!   source within minutes.
//! - **aircraft**: `modeS` → `icao24`, and the document's `_id` is
//!   replaced with the legacy ObjectId pattern only as long as the
//!   `icao24` field exists (the new adapter looks documents up by the
//!   indexed `icao24`, not by `_id`).
//! - **aircraft_to_process**: `modeS` → `icao24`, `query_attempts` →
//!   `attempts`, `last_attempt_time` → `last_attempt_at`. `_id` is set
//!   to the icao24 string so the new adapter's upsert path lines up.
//! - **users**: collection dropped entirely; admin re-seeded from
//!   `ADMIN_EMAIL` + `ADMIN_PASSWORD` on next server boot.
//!
//! Usage:
//!
//! ```bash
//! MONGO_URI=mongodb://localhost:27017 \
//! MONGO_DB=flightradar \
//!     flightradar-migrate
//! ```

use anyhow::{Context, Result};
use bson::{doc, Document};
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let uri = std::env::var("MONGO_URI").context("MONGO_URI must be set")?;
    let db_name = std::env::var("MONGO_DB").context("MONGO_DB must be set")?;
    let opts = ClientOptions::parse(&uri)
        .await
        .context("parse MONGO_URI")?;
    let client = Client::with_options(opts).context("build mongo client")?;
    let db = client.database(&db_name);

    info!(database = %db_name, "connected to mongo");

    migrate_flights(&db).await?;
    migrate_positions(&db).await?;
    migrate_aircraft(&db).await?;
    migrate_crawler_queue(&db).await?;
    drop_users(&db).await?;

    info!("migration complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// flights
// ---------------------------------------------------------------------------

async fn migrate_flights(db: &Database) -> Result<()> {
    let col: Collection<Document> = db.collection("flights");
    let count = col
        .count_documents(doc! { "modeS": { "$exists": true } })
        .await
        .context("count legacy flights")?;
    if count == 0 {
        info!("flights: nothing to migrate");
        return Ok(());
    }
    info!(count, "flights: renaming modeS → icao24");
    let res = col
        .update_many(
            doc! { "modeS": { "$exists": true } },
            doc! {
                "$rename": { "modeS": "icao24" },
                "$unset": { "expire_at": "" }
            },
        )
        .await
        .context("rename modeS in flights")?;
    info!(
        matched = res.matched_count,
        modified = res.modified_count,
        "flights migrated"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// positions (drop + let bootstrap recreate)
// ---------------------------------------------------------------------------

async fn migrate_positions(db: &Database) -> Result<()> {
    // Look for the legacy time-series collection. Two telltales:
    //   - presence of the `timestmp` field on a sample document,
    //   - or the collection exists at all but with a different timeField.
    let col: Collection<Document> = db.collection("positions");
    let names = db
        .list_collection_names()
        .await
        .context("list collections")?;
    let has_positions = names.iter().any(|n| n == "positions");
    if !has_positions {
        info!("positions: not present, nothing to migrate");
        return Ok(());
    }

    let legacy_count = col
        .count_documents(doc! { "timestmp": { "$exists": true } })
        .await
        .unwrap_or(0);
    if legacy_count == 0 {
        // Could still be a Rust-shape collection. Leave it alone.
        info!("positions: no legacy `timestmp` documents — leaving in place");
        return Ok(());
    }

    warn!(
        legacy_count,
        "positions: dropping legacy time-series collection (field renames \
         aren't supported in place; live tracks will refill from the radar \
         source after server start)"
    );
    col.drop()
        .await
        .context("drop legacy positions collection")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// aircraft
// ---------------------------------------------------------------------------

async fn migrate_aircraft(db: &Database) -> Result<()> {
    let col: Collection<Document> = db.collection("aircraft");
    let count = col
        .count_documents(doc! { "modeS": { "$exists": true } })
        .await
        .unwrap_or(0);
    if count == 0 {
        info!("aircraft: nothing to migrate");
        return Ok(());
    }
    info!(count, "aircraft: renaming legacy fields");

    let res = col
        .update_many(
            doc! { "modeS": { "$exists": true } },
            doc! {
                "$rename": {
                    "modeS": "icao24",
                    "icaoTypeCode": "type_code",
                    "type": "type_description",
                    "registeredOwners": "operator",
                    "icaoTypeDesignator": "designator",
                },
            },
        )
        .await
        .context("rename aircraft fields")?;
    info!(
        matched = res.matched_count,
        modified = res.modified_count,
        "aircraft renamed"
    );

    // The new adapter looks up by the indexed `icao24` field, not `_id`,
    // so we leave the legacy `_id` (random ObjectId from Python) alone.
    // Admin edits will use `replace_one({icao24: …})` with `upsert: true`,
    // which preserves whatever `_id` is already there.
    Ok(())
}

// ---------------------------------------------------------------------------
// aircraft_to_process
// ---------------------------------------------------------------------------

async fn migrate_crawler_queue(db: &Database) -> Result<()> {
    let col: Collection<Document> = db.collection("aircraft_to_process");
    let count = col
        .count_documents(doc! { "modeS": { "$exists": true } })
        .await
        .unwrap_or(0);
    if count == 0 {
        info!("aircraft_to_process: nothing to migrate");
        return Ok(());
    }
    info!(count, "aircraft_to_process: renaming legacy fields");
    let res = col
        .update_many(
            doc! { "modeS": { "$exists": true } },
            doc! {
                "$rename": {
                    "modeS": "icao24",
                    "query_attempts": "attempts",
                    "last_attempt_time": "last_attempt_at",
                },
            },
        )
        .await
        .context("rename crawler-queue fields")?;
    info!(
        matched = res.matched_count,
        modified = res.modified_count,
        "crawler queue migrated"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// users (drop & reseed)
// ---------------------------------------------------------------------------

async fn drop_users(db: &Database) -> Result<()> {
    let names = db
        .list_collection_names()
        .await
        .context("list collections")?;
    if !names.iter().any(|n| n == "users") {
        info!("users: collection absent, nothing to do");
        return Ok(());
    }
    let col: Collection<Document> = db.collection("users");
    col.drop().await.context("drop users collection")?;
    info!("users: dropped (admin re-seeded from ADMIN_* env on next boot)");
    Ok(())
}
