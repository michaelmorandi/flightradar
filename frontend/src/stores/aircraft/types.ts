/**
 * Aircraft Data Layer Types
 *
 * Core type definitions for the unified aircraft data store.
 * These types support the single source of truth architecture where
 * all aircraft data is consolidated and pre-computed for optimal rendering.
 */

import type { AircraftCategory } from '@/utils/aircraftIcons';

/**
 * Core aircraft state - single source of truth for all aircraft data.
 * Consolidates position, identity, and type information that may arrive
 * from different SSE events at different times.
 */
export interface AircraftState {
  /** Flight ID (MongoDB ObjectId from backend) */
  id: string;

  /** 24-bit ICAO transponder address */
  icao24: string;

  // ═══════════════════════════════════════════════════════════
  // Position data (always present after first update)
  // ═══════════════════════════════════════════════════════════

  /** Latitude in decimal degrees */
  lat: number;

  /** Longitude in decimal degrees */
  lon: number;

  /** Altitude in feet (optional) */
  altitude?: number;

  /** Ground speed in knots (optional) */
  groundSpeed?: number;

  /** Raw track/heading from ADS-B in degrees (optional) */
  track?: number;

  // ═══════════════════════════════════════════════════════════
  // Identity data (may arrive in separate SSE messages)
  // ═══════════════════════════════════════════════════════════

  /** Aircraft callsign (optional, may arrive later) */
  callsign?: string;

  // ═══════════════════════════════════════════════════════════
  // Category/Type data (may arrive in separate SSE messages)
  // ═══════════════════════════════════════════════════════════

  /** ADS-B category enum value (optional) */
  category?: number;

  // ═══════════════════════════════════════════════════════════
  // Static aircraft info (fetched on demand when selected)
  // ═══════════════════════════════════════════════════════════

  /** Full aircraft type name */
  aircraftType?: string;

  /** ICAO aircraft type designator (e.g., "B738") */
  icaoType?: string;

  /** Aircraft registration (tail number) */
  registration?: string;

  /** Operator/airline name */
  operator?: string;

  // ═══════════════════════════════════════════════════════════
  // Timestamps
  // ═══════════════════════════════════════════════════════════

  /** Last position update timestamp in milliseconds */
  lastUpdate: number;

  /** First detection timestamp in milliseconds */
  firstSeen: number;

  // ═══════════════════════════════════════════════════════════
  // Computed/cached values (for heading resolution)
  // ═══════════════════════════════════════════════════════════

  /**
   * Resolved heading for display (cached).
   * Computed from track if available, or calculated from position history.
   */
  _resolvedHeading?: number;

  /** Previous latitude for heading calculation */
  _previousLat?: number;

  /** Previous longitude for heading calculation */
  _previousLon?: number;
}

/**
 * Map-optimized view of aircraft data.
 * Pre-computed for rendering performance - contains only the data
 * needed for map marker display.
 */
export interface MapAircraftView {
  /** Flight ID */
  id: string;

  /** 24-bit ICAO transponder address */
  icao24: string;

  /** Latitude */
  lat: number;

  /** Longitude (using HERE Maps naming convention) */
  lng: number;

  /** Heading in degrees (always defined - required for render) */
  heading: number;

  /** Icon category for aircraft type visualization */
  iconCategory: AircraftCategory;

  /** Callsign for tooltip display (optional) */
  callsign?: string;

  /** Ground speed for tooltip display (optional) */
  groundSpeed?: number;

  /** ADS-B category value (for marker updates) */
  category?: number;
}

/**
 * List-optimized view of aircraft data.
 * Pre-computed for list display - includes all data needed for the
 * aircraft list component without requiring additional queries.
 */
export interface ListAircraftView {
  /** Flight ID */
  flightId: string;

  /** 24-bit ICAO address */
  icao24: string;

  /** Callsign */
  callsign?: string;

  /** Latitude */
  lat: number;

  /** Longitude */
  lon: number;

  /** Altitude in feet */
  altitude?: number;

  /** Ground speed in knots */
  groundSpeed?: number;

  /** Track/heading */
  track?: number;

  /** Operator name (from state or cache) */
  operator?: string;

  /** Full aircraft type name */
  aircraftType?: string;

  /** ICAO aircraft type designator */
  icaoType?: string;

  /** ADS-B category */
  category?: number;

  /** First seen timestamp */
  firstSeen: number;
}

/**
 * Position data for flight history/path rendering.
 */
export interface HistoryPosition {
  /** Latitude */
  lat: number;

  /** Longitude */
  lon: number;

  /** Altitude in feet (optional) */
  altitude?: number;

  /** Timestamp when this position was recorded */
  timestamp: number;

  /** Ground speed at this position (optional) */
  groundSpeed?: number;

  /** Track/heading at this position (optional) */
  track?: number;
}

/**
 * Position update from SSE stream.
 * May be a full update or a delta update with partial fields.
 */
export interface PositionUpdate {
  /** 24-bit ICAO address */
  icao24: string;

  /** Latitude */
  lat: number;

  /** Longitude */
  lon: number;

  /** Altitude (optional) */
  altitude?: number;

  /** Ground speed (optional) */
  groundSpeed?: number;

  /** Track/heading (optional) */
  track?: number;

  /** Callsign (optional, sometimes included in position updates) */
  callsign?: string;

  /** Category (optional, sometimes included in position updates) */
  category?: number;
}

/**
 * Static aircraft information from the aircraft database.
 * Loaded on-demand when an aircraft is selected.
 */
export interface AircraftDetails {
  /** 24-bit ICAO address */
  icao24: string;

  /** Full aircraft type name */
  type?: string;

  /** ICAO aircraft type designator */
  icaoType?: string;

  /** Aircraft registration */
  registration?: string;

  /** Operator name */
  operator?: string;
}

/**
 * Connection status for SSE streams.
 */
export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

/**
 * SSE message types from the backend.
 */
export type SSEMessageType = 'initial' | 'update';

/**
 * Raw positions data from SSE stream.
 */
export interface SSEPositionsMessage {
  type: SSEMessageType;
  positions: Record<string, RawPositionData>;
}

/**
 * Raw position data as received from backend.
 * Uses backend field names (lat, lon, alt, gs, track, callsign, cat).
 */
export interface RawPositionData {
  icao: string;
  lat: number;
  lon: number;
  alt?: number;
  gs?: number;
  track?: number;
  callsign?: string;
  cat?: number;
}

/**
 * Callsigns update from SSE stream.
 */
export interface SSECallsignsMessage {
  callsigns: Record<string, string>;
}

/**
 * Categories update from SSE stream.
 */
export interface SSECategoriesMessage {
  categories: Record<string, number>;
}

/**
 * Flight position stream message types.
 */
export interface SSEFlightPositionMessage {
  type: SSEMessageType;
  positions?: Record<string, RawPositionData[]>;
  // For update messages, position data is at the root level
  lat?: number;
  lon?: number;
  alt?: number;
  gs?: number;
  track?: number;
}
