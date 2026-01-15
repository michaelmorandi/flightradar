/**
 * Map Renderer Composable
 *
 * Provides reactive marker management for the map.
 * Watches the aircraft store's mapView and efficiently updates
 * markers on the HERE Maps instance.
 */

import { watch, onUnmounted, type Ref, shallowRef } from 'vue';
import { useAircraftStore, useFlightHistoryStore, type MapAircraftView, type HistoryPosition } from '@/stores/aircraft';
import { getDataIngestionService } from '@/services/dataIngestionService';
import { MarkerManager } from '@/components/map/MarkerManager';
import { FlightPath } from '@/components/map/flightPath';
import { AircraftIcon } from '@/components/map/aircraftElements';
import type { TerrestialPosition } from '@/model/backendModel';

// Dummy position store for MarkerManager compatibility (it needs updatePositions method)
// We'll create a minimal adapter
interface PositionStoreAdapter {
  updatePositions: (positions: Map<string, TerrestialPosition>) => void;
  staleThreshold: number;
  purgeStalePositions: () => void;
}

/**
 * Create a map renderer for managing aircraft markers.
 *
 * @param map Ref to the HERE Maps instance
 * @param onMarkerClick Callback when a marker is clicked
 */
export function useMapRenderer(
  map: Ref<unknown>,
  onMarkerClick?: (flightId: string) => void
) {
  const aircraftStore = useAircraftStore();
  const historyStore = useFlightHistoryStore();
  const dataService = getDataIngestionService();

  // Track created markers for cleanup
  const markerManager = shallowRef<MarkerManager | null>(null);
  const selectedFlightPath = shallowRef<FlightPath | null>(null);
  const lastSelectedFlightId = shallowRef<string | null>(null);

  // Create a minimal position store adapter for MarkerManager
  const createPositionStoreAdapter = (): PositionStoreAdapter => ({
    updatePositions: () => {}, // No-op, we handle updates through the store
    staleThreshold: aircraftStore.staleThreshold / 1000, // Convert to seconds
    purgeStalePositions: () => {}, // No-op
  });

  /**
   * Initialize the marker manager when map is ready.
   */
  function initializeMarkerManager(mapInstance: unknown): MarkerManager {
    const adapter = createPositionStoreAdapter();
    const manager = new MarkerManager(mapInstance, adapter as any);

    if (onMarkerClick) {
      manager.setOnMarkerClickCallback(onMarkerClick);
    }

    markerManager.value = manager;
    return manager;
  }

  /**
   * Update markers based on the current mapView.
   * Efficiently diffs the current markers with the new view.
   */
  function updateMarkers(view: Map<string, MapAircraftView>) {
    if (!markerManager.value || !map.value) return;

    const manager = markerManager.value;
    const currentMarkers = manager.getAllMarkers();

    // Remove markers not in new view
    for (const [id] of currentMarkers) {
      if (!view.has(id)) {
        manager.removeMarker(id);
      }
    }

    // Add/update markers from view
    for (const [id, data] of view) {
      const coords = {
        lat: data.lat,
        lng: data.lng,
        heading: data.heading,
      };
      manager.updateMarker(id, coords, data.groundSpeed, data.callsign, data.category);
    }
  }

  /**
   * Handle flight selection.
   */
  function selectFlight(flightId: string | null) {
    if (!markerManager.value || !map.value) return;

    const manager = markerManager.value;
    const previousFlightId = lastSelectedFlightId.value;

    // Reset color of previously selected marker
    if (previousFlightId && previousFlightId !== flightId) {
      if (manager.hasMarker(previousFlightId)) {
        manager.setMarkerColor(previousFlightId, AircraftIcon.INACTIVE_COLOR);
      }

      // Clean up previous flight path
      if (selectedFlightPath.value) {
        selectedFlightPath.value.removeFlightPath();
        selectedFlightPath.value = null;
      }

      // Unsubscribe from previous flight
      if (dataService.isSubscribedToFlight(previousFlightId)) {
        dataService.unsubscribeFromFlight(previousFlightId);
      }
    }

    // Handle new selection
    if (flightId) {
      // Highlight new marker
      if (manager.hasMarker(flightId)) {
        manager.setMarkerColor(flightId, AircraftIcon.HIGHLIGHT_COLOR);
      }

      // Create flight path
      selectedFlightPath.value = new FlightPath(flightId, map.value as any);

      // Subscribe to flight history updates
      dataService.subscribeToFlight(flightId);

      // Update store selection
      aircraftStore.selectFlight(flightId);
    } else {
      aircraftStore.clearSelection();
    }

    lastSelectedFlightId.value = flightId;
  }

  /**
   * Unselect the current flight.
   */
  function unselectFlight() {
    selectFlight(null);
  }

  /**
   * Update the flight path with history positions.
   */
  function updateFlightPath(flightId: string, positions: readonly HistoryPosition[]) {
    if (!selectedFlightPath.value || selectedFlightPath.value.flightId !== flightId) {
      return;
    }

    // Convert HistoryPosition to TerrestialPosition for FlightPath
    const terrestialPositions: TerrestialPosition[] = positions.map(pos => ({
      icao: '', // Not used by FlightPath
      callsign: '',
      lat: pos.lat,
      lon: pos.lon,
      alt: pos.altitude,
      track: pos.track,
      gs: pos.groundSpeed,
    }));

    selectedFlightPath.value.updateFlightPath(terrestialPositions);
  }

  /**
   * Clean up all markers and flight paths.
   */
  function cleanup() {
    // Clean up markers
    if (markerManager.value) {
      markerManager.value.clearAllMarkers();
      markerManager.value = null;
    }

    // Clean up flight path
    if (selectedFlightPath.value) {
      selectedFlightPath.value.removeFlightPath();
      selectedFlightPath.value = null;
    }

    // Unsubscribe from flight history
    if (lastSelectedFlightId.value && dataService.isSubscribedToFlight(lastSelectedFlightId.value)) {
      dataService.unsubscribeFromFlight(lastSelectedFlightId.value);
    }

    lastSelectedFlightId.value = null;
  }

  // Watch the mapView for changes and update markers
  const stopMapViewWatch = watch(
    () => aircraftStore.mapView,
    (newView) => {
      updateMarkers(newView);
    },
    { immediate: false }
  );

  // Watch for selection changes
  const stopSelectionWatch = watch(
    () => aircraftStore.selectedFlightId,
    (newFlightId, oldFlightId) => {
      // Handle color changes for selection
      if (markerManager.value) {
        if (oldFlightId && oldFlightId !== newFlightId) {
          if (markerManager.value.hasMarker(oldFlightId)) {
            markerManager.value.setMarkerColor(oldFlightId, AircraftIcon.INACTIVE_COLOR);
          }
        }
        if (newFlightId && newFlightId !== oldFlightId) {
          if (markerManager.value.hasMarker(newFlightId)) {
            markerManager.value.setMarkerColor(newFlightId, AircraftIcon.HIGHLIGHT_COLOR);
          }
        }
      }
    }
  );

  // Watch flight history for updates to selected flight
  const stopHistoryWatch = watch(
    () => {
      const selectedId = lastSelectedFlightId.value;
      if (!selectedId) return null;
      return historyStore.histories.get(selectedId);
    },
    (positions) => {
      if (positions && lastSelectedFlightId.value) {
        updateFlightPath(lastSelectedFlightId.value, positions);
      }
    },
    { deep: false }
  );

  // Cleanup on unmount
  onUnmounted(() => {
    stopMapViewWatch();
    stopSelectionWatch();
    stopHistoryWatch();
    cleanup();
  });

  return {
    // State
    markerManager,
    selectedFlightPath,

    // Methods
    initializeMarkerManager,
    selectFlight,
    unselectFlight,
    cleanup,

    // For testing/debugging
    updateMarkers,
  };
}

/**
 * Type for the return value of useMapRenderer.
 */
export type MapRendererResult = ReturnType<typeof useMapRenderer>;
