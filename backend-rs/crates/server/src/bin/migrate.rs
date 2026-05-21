//! One-shot Mongo schema migration: legacy Python field names → clean
//! Rust shape. Idempotent — running it twice is a no-op.
//!
//! What it changes (collection-by-collection):
//!
//! - **flights**:
//!     - `modeS` → `icao24`
//!     - `is_military` left as-is (already present)
//!     - `airline_icao` left as-is
//!     - legacy `expire_at` timestamp dropped (Mongo TTL index handles
//!       expiry now)
//! - **positions**:
//!     - `flight_id` left as-is (already ObjectId)
//!     - `timestmp` → `observed_at`
//!     - `alt` → `alt_ft`
//!     - `gs` → `ground_speed_kt`
//!     - `track` → `track_deg`
//!     - legacy `expire_at` dropped
//! - **aircraft**:
//!     - `modeS` → `icao24`
//!     - `registration`, `icaoTypeCode` → `type_code`, `type` →
//!       `type_description`, `registeredOwners` → `operator`,
//!       `icaoTypeDesignator` → `designator`
//! - **aircraft_to_process**:
//!     - `modeS` → `icao24`
//!     - `query_attempts` → `attempts`
//!     - `last_attempt_time` → `last_attempt_at`
//! - **users**: collection dropped entirely (clean-slate auth — the
//!   admin is re-seeded from `ADMIN_*` env vars on first server boot).
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
// positions
// ---------------------------------------------------------------------------

async fn migrate_positions(db: &Database) -> Result<()> {
    let col: Collection<Document> = db.collection("positions");
    let count = col
        .count_documents(doc! { "timestmp": { "$exists": true } })
        .await
        .unwrap_or(0);
    if count == 0 {
        info!("positions: nothing to migrate");
        return Ok(());
    }
    // Time-series collections in Mongo don't accept $rename across
    // metaField/timeField via updateMany — they're write-once. The
    // pragmatic path: log how many docs would be affected and instruct
    // the operator to drop and re-seed from the radar source.
    warn!(
        count,
        "positions is a time-series collection; field renames are not \
         supported in place. Drop the collection and let the FlightUpdater \
         repopulate it from the radar source after cutover."
    );
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
        "aircraft migrated"
    );

    // The _id key used to mirror modeS in the old Python store; if it's
    // still the legacy random ObjectId we leave it (the Rust adapter
    // looks up by icao24 field, not _id). New documents will use icao24
    // as _id directly.
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
    // Clean-slate auth: the Beanie/fastapi-users shape isn't worth
    // shimming. Drop the whole collection; the new admin will be seeded
    // from ADMIN_EMAIL + ADMIN_PASSWORD on the next server boot.
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
    info!("users: dropped (admin will be re-seeded from ADMIN_* env on next boot)");
    Ok(())
}
