#!/usr/bin/env python3

import argparse
import logging
import sys
from pathlib import Path

# Add the parent directory to the path to import from app
sys.path.append(str(Path(__file__).parent.parent.parent))

from app.config import Config
from app.data.database import init_mongodb

def update_aircraft_with_icao_designators(config: Config):
    """Update aircraft documents with ICAO type designators"""
    
    logger = logging.getLogger("AircraftIcaoUpdater")
    
    # Initialize MongoDB connection
    try:
        db = init_mongodb(config.MONGODB_URI, config.MONGODB_DB_NAME, config.DB_RETENTION_MIN)
        logger.info("Connected to MongoDB successfully")
    except Exception as e:
        logger.error(f"Failed to connect to MongoDB: {e}")
        return False
    
    aircraft_collection = db["aircraft"]
    designators_collection = db["icao_type_designators"]
    
    # Get all ICAO type designators for lookup
    try:
        designators = {}
        for doc in designators_collection.find({}, {"icaoTypeCode": 1, "icaoTypeDesignator": 1}):
            designators[doc["icaoTypeCode"]] = doc["icaoTypeDesignator"]
        
        logger.info(f"Loaded {len(designators)} ICAO type designators for lookup")
        
        if not designators:
            logger.warning("No ICAO type designators found in collection")
            return False
            
    except Exception as e:
        logger.error(f"Failed to load ICAO designators: {e}")
        return False
    
    # Process aircraft documents
    try:
        # Find aircraft that have icaoTypeCode but no icaoTypeDesignator
        query = {
            "icaoTypeCode": {"$exists": True, "$ne": None, "$ne": ""},
            "$or": [
                {"icaoTypeDesignator": {"$exists": False}},
                {"icaoTypeDesignator": None},
                {"icaoTypeDesignator": ""}
            ]
        }
        
        aircraft_cursor = aircraft_collection.find(query, {"_id": 1, "modeS": 1, "icaoTypeCode": 1})
        aircraft_list = list(aircraft_cursor)
        
        logger.info(f"Found {len(aircraft_list)} aircraft documents to update")
        
        updated_count = 0
        not_found_count = 0
        
        for aircraft in aircraft_list:
            icao_type_code = aircraft.get("icaoTypeCode")
            
            if icao_type_code in designators:
                # Update the aircraft document with the designator
                result = aircraft_collection.update_one(
                    {"_id": aircraft["_id"]},
                    {"$set": {"icaoTypeDesignator": designators[icao_type_code]}}
                )
                
                if result.modified_count > 0:
                    updated_count += 1
                    if updated_count % 100 == 0:
                        logger.info(f"Updated {updated_count} aircraft documents...")
            else:
                not_found_count += 1
                logger.debug(f"No designator found for ICAO type code: {icao_type_code}")
        
        logger.info(f"Update completed: {updated_count} aircraft updated, {not_found_count} designators not found")
        return True
        
    except Exception as e:
        logger.error(f"Failed to update aircraft documents: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Update aircraft documents with ICAO type designators")
    parser.add_argument("-v", "--verbose", action="store_true", help="Enable verbose logging")
    
    args = parser.parse_args()
    
    # Setup logging
    log_level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=log_level,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    logger = logging.getLogger("AircraftIcaoUpdater")
    
    # Load configuration
    try:
        config = Config()
        logger.info("Configuration loaded successfully")
    except Exception as e:
        logger.error(f"Failed to load configuration: {e}")
        sys.exit(1)
    
    # Update aircraft
    success = update_aircraft_with_icao_designators(config)
    
    if success:
        logger.info("Aircraft update completed successfully")
        sys.exit(0)
    else:
        logger.error("Aircraft update failed")
        sys.exit(1)

if __name__ == "__main__":
    main()