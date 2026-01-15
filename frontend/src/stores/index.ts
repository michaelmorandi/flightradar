// Centralized exports for all Pinia stores

// New unified aircraft data stores
export { useAircraftStore, useFlightHistoryStore } from './aircraft';
export type {
  AircraftState,
  MapAircraftView,
  HistoryPosition,
  PositionUpdate,
  AircraftDetails,
  ConnectionStatus,
} from './aircraft';

// Map store (retained - manages map UI state)
export { useMapStore } from './map';
export type { MapCenter, MapViewport, MapConfiguration } from './map';
