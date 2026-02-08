/**
 * Aircraft Store Helper Functions
 *
 * Pure utility functions for aircraft data processing.
 * These functions are stateless and side-effect free.
 */

import type {
  AircraftState,
  PositionUpdate,
  MapAircraftView,
  RawPositionData,
  HistoryPosition,
} from './types';
import { mapProtobufCategoryToIcon, determineAircraftCategory, type AircraftCategory } from '@/utils/aircraftIcons';

/**
 * Create a new AircraftState from an initial position update.
 */
export function createAircraftState(
  id: string,
  update: PositionUpdate,
  now: number
): AircraftState {
  return {
    id,
    icao24: update.icao24,
    lat: update.lat,
    lon: update.lon,
    altitude: update.altitude,
    groundSpeed: update.groundSpeed,
    track: update.track,
    callsign: update.callsign,
    category: update.category,
    lastUpdate: now,
    firstSeen: now,
    _resolvedHeading: update.track,
  };
}

/**
 * Merge a position update into an existing aircraft state.
 * Preserves fields not present in the update (delta merge).
 */
export function mergeAircraftState(
  existing: AircraftState,
  update: PositionUpdate,
  now: number
): AircraftState {
  // Store previous position for heading calculation if position changed
  const positionChanged = existing.lat !== update.lat || existing.lon !== update.lon;

  const merged: AircraftState = {
    ...existing,
    lat: update.lat,
    lon: update.lon,
    altitude: update.altitude ?? existing.altitude,
    groundSpeed: update.groundSpeed ?? existing.groundSpeed,
    track: update.track ?? existing.track,
    callsign: update.callsign ?? existing.callsign,
    category: update.category ?? existing.category,
    lastUpdate: now,
  };

  // Update heading history if position changed
  if (positionChanged) {
    merged._previousLat = existing.lat;
    merged._previousLon = existing.lon;
  }

  // Resolve heading
  merged._resolvedHeading = resolveHeading(merged);

  return merged;
}

/**
 * Update callsign for an aircraft state.
 */
export function updateAircraftCallsign(
  existing: AircraftState,
  callsign: string
): AircraftState {
  if (existing.callsign === callsign) {
    return existing;
  }
  return {
    ...existing,
    callsign,
  };
}

/**
 * Update category for an aircraft state.
 */
export function updateAircraftCategory(
  existing: AircraftState,
  category: number
): AircraftState {
  if (existing.category === category) {
    return existing;
  }
  return {
    ...existing,
    category,
  };
}

/**
 * Resolve aircraft heading from available data sources.
 *
 * Priority:
 * 1. Track from ADS-B data (most reliable)
 * 2. Cached resolved heading (preserves last known heading)
 * 3. Calculate from position history
 *
 * Returns undefined if no valid heading can be determined.
 */
export function resolveHeading(aircraft: AircraftState): number | undefined {
  // 1. Use track from position data if available (most reliable)
  if (aircraft.track !== undefined && aircraft.track !== null) {
    return Math.round(aircraft.track);
  }

  // 2. Preserve existing resolved heading if available
  if (aircraft._resolvedHeading !== undefined) {
    return aircraft._resolvedHeading;
  }

  // 3. Calculate heading from position history if available
  if (
    aircraft._previousLat !== undefined &&
    aircraft._previousLon !== undefined &&
    (aircraft._previousLat !== aircraft.lat || aircraft._previousLon !== aircraft.lon)
  ) {
    return calculateHeading(
      aircraft._previousLat,
      aircraft._previousLon,
      aircraft.lat,
      aircraft.lon
    );
  }

  // No valid heading available
  return undefined;
}

/**
 * Calculate heading from two geographic coordinates.
 * Returns heading in degrees (0-360).
 */
export function calculateHeading(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number
): number {
  const lat1Rad = (lat1 * Math.PI) / 180;
  const lon1Rad = (lon1 * Math.PI) / 180;
  const lat2Rad = (lat2 * Math.PI) / 180;
  const lon2Rad = (lon2 * Math.PI) / 180;

  const y = Math.sin(lon2Rad - lon1Rad) * Math.cos(lat2Rad);
  const x =
    Math.cos(lat1Rad) * Math.sin(lat2Rad) -
    Math.sin(lat1Rad) * Math.cos(lat2Rad) * Math.cos(lon2Rad - lon1Rad);

  let heading = (Math.atan2(y, x) * 180) / Math.PI;
  heading = (heading + 360) % 360;

  return Math.round(heading);
}

/**
 * Map ADS-B category to icon category with fallback to ICAO type.
 *
 * Priority:
 * 1. gRPC ADS-B category (if valid - not 0/UNKNOWN or 1/NO_INFO)
 * 2. ICAO type designator from aircraft database (fallback)
 * 3. Aircraft type description from database (fallback)
 * 4. Default icon
 */
export function mapCategoryToIcon(
  category?: number,
  icaoType?: string,
  aircraftType?: string
): AircraftCategory {
  // First try gRPC category (values > 1 are valid categories)
  if (category !== undefined && category > 1) {
    return mapProtobufCategoryToIcon(category);
  }

  // Fallback to ICAO type or aircraft type from database
  if (icaoType || aircraftType) {
    return determineAircraftCategory(aircraftType, icaoType);
  }

  return 'default';
}

/**
 * Convert AircraftState to MapAircraftView for rendering.
 * Returns null if the aircraft cannot be displayed (e.g., no valid heading).
 *
 * Icon category is determined with priority:
 * 1. gRPC ADS-B category (if valid)
 * 2. ICAO type designator from aircraft database
 * 3. Default icon
 */
export function toMapView(aircraft: AircraftState): MapAircraftView | null {
  const heading = resolveHeading(aircraft);

  // Cannot display aircraft without valid heading
  if (heading === undefined) {
    return null;
  }

  return {
    id: aircraft.id,
    icao24: aircraft.icao24,
    lat: aircraft.lat,
    lng: aircraft.lon, // HERE Maps uses 'lng'
    heading,
    iconCategory: mapCategoryToIcon(aircraft.category, aircraft.icaoType, aircraft.aircraftType),
    callsign: aircraft.callsign,
    groundSpeed: aircraft.groundSpeed,
    category: aircraft.category,
  };
}

/**
 * Parse raw position data from backend format to PositionUpdate.
 */
export function parsePositionData(
  id: string,
  raw: RawPositionData
): PositionUpdate {
  return {
    icao24: raw.icao,
    lat: raw.lat,
    lon: raw.lon,
    altitude: raw.alt,
    groundSpeed: raw.gs,
    track: raw.track !== undefined ? Math.round(raw.track) : undefined,
    callsign: raw.callsign,
    category: raw.cat,
  };
}

/**
 * Parse raw position data to HistoryPosition for flight path.
 */
export function parseHistoryPosition(
  raw: RawPositionData,
  timestamp?: number
): HistoryPosition {
  return {
    lat: raw.lat,
    lon: raw.lon,
    altitude: raw.alt,
    timestamp: timestamp ?? Date.now(),
    groundSpeed: raw.gs,
    track: raw.track,
  };
}

/**
 * Check if an aircraft is stale (hasn't been updated recently).
 */
export function isStale(aircraft: AircraftState, now: number, thresholdMs: number): boolean {
  return now - aircraft.lastUpdate > thresholdMs;
}

/**
 * Filter stale aircraft from a map.
 */
export function filterStaleAircraft(
  aircraft: Map<string, AircraftState>,
  now: number,
  thresholdMs: number
): Map<string, AircraftState> {
  const result = new Map<string, AircraftState>();

  for (const [id, ac] of aircraft) {
    if (!isStale(ac, now, thresholdMs)) {
      result.set(id, ac);
    }
  }

  return result;
}

/**
 * Build map view from aircraft data.
 * Filters out stale aircraft and those without valid headings.
 */
export function buildMapView(
  aircraft: Map<string, AircraftState>,
  now: number,
  staleThresholdMs: number
): Map<string, MapAircraftView> {
  const result = new Map<string, MapAircraftView>();

  for (const [id, ac] of aircraft) {
    // Skip stale aircraft
    if (isStale(ac, now, staleThresholdMs)) {
      continue;
    }

    // Convert to map view
    const view = toMapView(ac);
    if (view !== null) {
      result.set(id, view);
    }
  }

  return result;
}
