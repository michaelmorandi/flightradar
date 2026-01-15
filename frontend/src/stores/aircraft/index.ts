/**
 * Aircraft Store Module
 *
 * Exports for the unified aircraft data layer.
 */

// Stores
export { useAircraftStore } from './aircraftStore';
export { useFlightHistoryStore } from './flightHistoryStore';

// Types
export type {
  AircraftState,
  MapAircraftView,
  HistoryPosition,
  PositionUpdate,
  AircraftDetails,
  ConnectionStatus,
  SSEMessageType,
  SSEPositionsMessage,
  SSECallsignsMessage,
  SSECategoriesMessage,
  SSEFlightPositionMessage,
  RawPositionData,
} from './types';

// Helpers (for use by services and composables)
export {
  createAircraftState,
  mergeAircraftState,
  resolveHeading,
  calculateHeading,
  mapCategoryToIcon,
  toMapView,
  parsePositionData,
  parseHistoryPosition,
  isStale,
  filterStaleAircraft,
  buildMapView,
} from './helpers';
