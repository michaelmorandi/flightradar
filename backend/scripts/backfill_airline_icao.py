#!/usr/bin/env python3
"""
Backfill airline_icao field on existing flight documents.

Reads callsigns from all flights that have a callsign but no airline_icao,
extracts the ICAO airline code, and updates the documents in bulk.

Usage:
    python -m scripts.backfill_airline_icao [--mongodb-uri URI] [--db-name NAME] [--dry-run]

Run from the backend/ directory.
"""

import argparse
import logging
import sys
from pathlib import Path

# Add parent directory to path so we can import app modules
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from pymongo import MongoClient, UpdateOne
from app.core.utils.callsign_util import extract_airline_icao

logging.basicConfig(level=logging.INFO, format='%(asctime)s %(levelname)s %(message)s')
logger = logging.getLogger(__name__)

BATCH_SIZE = 1000


def backfill(mongodb_uri: str, db_name: str, dry_run: bool = False):
    client = MongoClient(mongodb_uri)
    db = client[db_name]
    flights = db['flights']

    # Find flights with a callsign but no airline_icao
    query = {
        "callsign": {"$exists": True, "$ne": None, "$ne": ""},
        "$or": [
            {"airline_icao": {"$exists": False}},
            {"airline_icao": None}
        ]
    }

    total = flights.count_documents(query)
    logger.info(f"Found {total} flights to process")

    if total == 0:
        logger.info("Nothing to backfill")
        return

    processed = 0
    updated = 0
    skipped = 0
    bulk_ops = []

    cursor = flights.find(query, {"_id": 1, "callsign": 1})

    for doc in cursor:
        callsign = doc.get("callsign", "")
        airline_icao = extract_airline_icao(callsign)

        if airline_icao:
            bulk_ops.append(UpdateOne(
                {"_id": doc["_id"]},
                {"$set": {"airline_icao": airline_icao}}
            ))
            updated += 1
        else:
            skipped += 1

        processed += 1

        if len(bulk_ops) >= BATCH_SIZE:
            if not dry_run:
                flights.bulk_write(bulk_ops, ordered=False)
            logger.info(f"Processed {processed}/{total} flights ({updated} updated, {skipped} skipped)")
            bulk_ops = []

    # Final batch
    if bulk_ops:
        if not dry_run:
            flights.bulk_write(bulk_ops, ordered=False)

    mode = "[DRY RUN] " if dry_run else ""
    logger.info(f"{mode}Backfill complete: {processed} processed, {updated} updated, {skipped} skipped (no airline pattern)")

    client.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Backfill airline_icao field on flight documents')
    parser.add_argument('--mongodb-uri', default='mongodb://localhost:27017/', help='MongoDB connection URI')
    parser.add_argument('--db-name', default='flightradar', help='Database name')
    parser.add_argument('--dry-run', action='store_true', help='Preview changes without writing')
    args = parser.parse_args()

    backfill(args.mongodb_uri, args.db_name, args.dry_run)
