/**
 * Aircraft Store
 *
 * Single unified Pinia store for all live aircraft data.
 * Provides a single source of truth for aircraft state with
 * pre-computed views for optimal rendering performance.
 */

import { defineStore } from 'pinia';
import { shallowRef, ref, computed, readonly } from 'vue';
import type {
  AircraftState,
  PositionUpdate,
  MapAircraftView,
  AircraftDetails,
  ConnectionStatus,
} from './types';
import {
  createAircraftState,
  mergeAircraftState,
  updateAircraftCallsign,
  updateAircraftCategory,
  buildMapView,
  filterStaleAircraft,
} from './helpers';

export const useAircraftStore = defineStore('aircraft', () => {
  // ═══════════════════════════════════════════════════════════
  // CORE STATE
  // ═══════════════════════════════════════════════════════════

  /**
   * Primary aircraft data store.
   * Uses shallowRef for performance - reactivity is triggered
   * only when the entire Map is replaced.
   */
  const aircraft = shallowRef<Map<string, AircraftState>>(new Map());

  /**
   * Cache for aircraft details (static data loaded on demand).
   */
  const aircraftDetailsCache = shallowRef<Map<string, AircraftDetails>>(new Map());

  // ═══════════════════════════════════════════════════════════
  // UI STATE
  // ═══════════════════════════════════════════════════════════

  /** Currently selected flight ID */
  const selectedFlightId = ref<string | null>(null);

  /** Currently highlighted flight ID (e.g., on hover) */
  const highlightedFlightId = ref<string | null>(null);

  // ═══════════════════════════════════════════════════════════
  // CONNECTION STATE
  // ═══════════════════════════════════════════════════════════

  /** SSE connection status */
  const connectionStatus = ref<ConnectionStatus>('disconnected');

  /** Last successful update timestamp */
  const lastUpdate = ref<number | null>(null);

  // ═══════════════════════════════════════════════════════════
  // CONFIGURATION
  // ═══════════════════════════════════════════════════════════

  /** Stale threshold in milliseconds (15 seconds) */
  const staleThreshold = 15_000;

  // ═══════════════════════════════════════════════════════════
  // COMPUTED VIEWS (Reactive, auto-updating)
  // ═══════════════════════════════════════════════════════════

  /**
   * Map-ready view of all active aircraft.
   * Pre-computed for optimal rendering performance.
   * Only includes aircraft with valid headings that are not stale.
   */
  const mapView = computed<Map<string, MapAircraftView>>(() => {
    const now = Date.now();
    return buildMapView(aircraft.value, now, staleThreshold);
  });

  /**
   * Count of currently active (non-stale) aircraft.
   */
  const activeCount = computed(() => mapView.value.size);

  /**
   * Total count of all tracked aircraft (including stale).
   */
  const totalCount = computed(() => aircraft.value.size);

  /**
   * Currently selected aircraft state (full details).
   */
  const selectedAircraft = computed(() =>
    selectedFlightId.value ? aircraft.value.get(selectedFlightId.value) ?? null : null
  );

  /**
   * Selected aircraft's static details (if loaded).
   */
  const selectedAircraftDetails = computed(() => {
    const ac = selectedAircraft.value;
    if (!ac) return null;
    return aircraftDetailsCache.value.get(ac.icao24) ?? null;
  });

  /**
   * Check if connection is active.
   */
  const isConnected = computed(() => connectionStatus.value === 'connected');

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Data Ingestion
  // ═══════════════════════════════════════════════════════════

  /**
   * Update positions from SSE stream.
   * This is the main entry point for position data.
   *
   * @param updates Map of flight ID to position update
   * @param isInitial If true, replaces all data (initial snapshot)
   */
  function updatePositions(updates: Map<string, PositionUpdate>, isInitial = false) {
    const now = Date.now();

    if (isInitial) {
      // Replace all data for initial snapshot
      const newMap = new Map<string, AircraftState>();
      for (const [id, update] of updates) {
        newMap.set(id, createAircraftState(id, update, now));
      }
      aircraft.value = newMap;
    } else {
      // Delta update - merge with existing data
      const newMap = new Map(aircraft.value);

      for (const [id, update] of updates) {
        const existing = newMap.get(id);

        if (existing) {
          newMap.set(id, mergeAircraftState(existing, update, now));
        } else {
          newMap.set(id, createAircraftState(id, update, now));
        }
      }

      // Trigger reactivity via shallowRef replacement
      aircraft.value = newMap;
    }

    lastUpdate.value = now;
  }

  /**
   * Update callsigns from SSE stream.
   *
   * @param callsigns Map of flight ID to callsign
   */
  function updateCallsigns(callsigns: Map<string, string>) {
    let hasChanges = false;
    const newMap = new Map(aircraft.value);

    for (const [id, callsign] of callsigns) {
      const existing = newMap.get(id);
      if (existing && existing.callsign !== callsign) {
        newMap.set(id, updateAircraftCallsign(existing, callsign));
        hasChanges = true;
      }
    }

    if (hasChanges) {
      aircraft.value = newMap;
    }
  }

  /**
   * Update categories from SSE stream.
   *
   * @param categories Map of flight ID to category number
   */
  function updateCategories(categories: Map<string, number>) {
    let hasChanges = false;
    const newMap = new Map(aircraft.value);

    for (const [id, category] of categories) {
      const existing = newMap.get(id);
      if (existing && existing.category !== category) {
        newMap.set(id, updateAircraftCategory(existing, category));
        hasChanges = true;
      }
    }

    if (hasChanges) {
      aircraft.value = newMap;
    }
  }

  /**
   * Cache aircraft details (static data).
   *
   * @param details Aircraft details to cache
   */
  function cacheAircraftDetails(details: AircraftDetails) {
    const newCache = new Map(aircraftDetailsCache.value);
    newCache.set(details.icao24, details);
    aircraftDetailsCache.value = newCache;

    // Also update the aircraft state if it exists
    const existing = findAircraftByIcao24(details.icao24);
    if (existing) {
      const newMap = new Map(aircraft.value);
      newMap.set(existing.id, {
        ...existing,
        aircraftType: details.type,
        icaoType: details.icaoType,
        registration: details.registration,
        operator: details.operator,
      });
      aircraft.value = newMap;
    }
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Connection Management
  // ═══════════════════════════════════════════════════════════

  /**
   * Set connection status.
   */
  function setConnectionStatus(status: ConnectionStatus) {
    connectionStatus.value = status;
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Queries
  // ═══════════════════════════════════════════════════════════

  /**
   * Get aircraft by flight ID.
   */
  function getAircraftById(id: string): AircraftState | undefined {
    return aircraft.value.get(id);
  }

  /**
   * Find aircraft by ICAO24 address.
   */
  function findAircraftByIcao24(icao24: string): AircraftState | undefined {
    for (const ac of aircraft.value.values()) {
      if (ac.icao24 === icao24) {
        return ac;
      }
    }
    return undefined;
  }

  /**
   * Get all active (non-stale) aircraft.
   */
  function getActiveAircraft(): AircraftState[] {
    const now = Date.now();
    const result: AircraftState[] = [];

    for (const ac of aircraft.value.values()) {
      if (now - ac.lastUpdate <= staleThreshold) {
        result.push(ac);
      }
    }

    return result;
  }

  /**
   * Get cached aircraft details by ICAO24.
   */
  function getAircraftDetails(icao24: string): AircraftDetails | undefined {
    return aircraftDetailsCache.value.get(icao24);
  }

  /**
   * Update an aircraft's icao24 field by flight ID.
   * This is needed because position stream doesn't include icao24.
   */
  function updateAircraftIcao24(flightId: string, icao24: string) {
    const existing = aircraft.value.get(flightId);
    if (existing && !existing.icao24) {
      const newMap = new Map(aircraft.value);
      newMap.set(flightId, {
        ...existing,
        icao24,
      });
      aircraft.value = newMap;
    }
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Selection & UI
  // ═══════════════════════════════════════════════════════════

  /**
   * Select a flight.
   */
  function selectFlight(id: string | null) {
    selectedFlightId.value = id;
  }

  /**
   * Highlight a flight (e.g., on hover).
   */
  function highlightFlight(id: string | null) {
    highlightedFlightId.value = id;
  }

  /**
   * Clear selection.
   */
  function clearSelection() {
    selectedFlightId.value = null;
  }

  /**
   * Clear highlight.
   */
  function clearHighlight() {
    highlightedFlightId.value = null;
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Cleanup
  // ═══════════════════════════════════════════════════════════

  /**
   * Remove stale aircraft from the store.
   * Called periodically to clean up old data.
   *
   * @returns Number of aircraft removed
   */
  function purgeStaleAircraft(): number {
    const now = Date.now();
    const filtered = filterStaleAircraft(aircraft.value, now, staleThreshold);

    const removed = aircraft.value.size - filtered.size;
    if (removed > 0) {
      aircraft.value = filtered;
    }

    return removed;
  }

  /**
   * Clear all aircraft data.
   */
  function clearAllAircraft() {
    aircraft.value = new Map();
    lastUpdate.value = null;
  }

  /**
   * Reset the entire store to initial state.
   */
  function $reset() {
    aircraft.value = new Map();
    aircraftDetailsCache.value = new Map();
    selectedFlightId.value = null;
    highlightedFlightId.value = null;
    connectionStatus.value = 'disconnected';
    lastUpdate.value = null;
  }

  // ═══════════════════════════════════════════════════════════
  // RETURN
  // ═══════════════════════════════════════════════════════════

  return {
    // State (readonly to prevent direct mutation)
    aircraft: readonly(aircraft),
    aircraftDetailsCache: readonly(aircraftDetailsCache),
    selectedFlightId: readonly(selectedFlightId),
    highlightedFlightId: readonly(highlightedFlightId),
    connectionStatus,
    lastUpdate: readonly(lastUpdate),
    staleThreshold,

    // Computed views
    mapView,
    activeCount,
    totalCount,
    selectedAircraft,
    selectedAircraftDetails,
    isConnected,

    // Actions - Data Ingestion
    updatePositions,
    updateCallsigns,
    updateCategories,
    cacheAircraftDetails,

    // Actions - Connection
    setConnectionStatus,

    // Actions - Queries
    getAircraftById,
    findAircraftByIcao24,
    getActiveAircraft,
    getAircraftDetails,
    updateAircraftIcao24,

    // Actions - UI
    selectFlight,
    highlightFlight,
    clearSelection,
    clearHighlight,

    // Actions - Cleanup
    purgeStaleAircraft,
    clearAllAircraft,
    $reset,
  };
});
