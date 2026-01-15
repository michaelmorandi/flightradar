/**
 * Flight History Store
 *
 * Separate Pinia store for flight path history data.
 * Manages position history for selected flights, used for
 * rendering flight paths on the map.
 */

import { defineStore } from 'pinia';
import { shallowRef, ref, computed, readonly } from 'vue';
import type { HistoryPosition } from './types';

/** Maximum number of positions to keep per flight */
const MAX_HISTORY_POSITIONS = 1000;

export const useFlightHistoryStore = defineStore('flightHistory', () => {
  // ═══════════════════════════════════════════════════════════
  // STATE
  // ═══════════════════════════════════════════════════════════

  /**
   * Flight history data keyed by flight ID.
   * Uses shallowRef for performance - reactivity triggers
   * only on Map replacement.
   */
  const histories = shallowRef<Map<string, HistoryPosition[]>>(new Map());

  /**
   * Active subscriptions - flight IDs currently being tracked.
   * These flights have active SSE connections for path updates.
   */
  const activeSubscriptions = ref<Set<string>>(new Set());

  /**
   * Loading state per flight ID.
   */
  const loading = ref<Map<string, boolean>>(new Map());

  // ═══════════════════════════════════════════════════════════
  // COMPUTED
  // ═══════════════════════════════════════════════════════════

  /**
   * Check if a flight is currently subscribed for history updates.
   */
  const isSubscribed = computed(() => {
    return (flightId: string) => activeSubscriptions.value.has(flightId);
  });

  /**
   * Get the number of active subscriptions.
   */
  const subscriptionCount = computed(() => activeSubscriptions.value.size);

  /**
   * Check if a flight's history is loading.
   */
  const isLoading = computed(() => {
    return (flightId: string) => loading.value.get(flightId) ?? false;
  });

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Data Management
  // ═══════════════════════════════════════════════════════════

  /**
   * Set the complete history for a flight (initial load).
   */
  function setHistory(flightId: string, positions: HistoryPosition[]) {
    const newMap = new Map(histories.value);

    // Cap at max positions, keeping most recent
    const capped = positions.length > MAX_HISTORY_POSITIONS
      ? positions.slice(-MAX_HISTORY_POSITIONS)
      : positions;

    newMap.set(flightId, capped);
    histories.value = newMap;
  }

  /**
   * Add positions to a flight's history (incremental updates).
   */
  function addPositions(flightId: string, positions: HistoryPosition[]) {
    const newMap = new Map(histories.value);
    const existing = newMap.get(flightId) || [];

    // Combine and cap at max positions
    const combined = [...existing, ...positions];
    const capped = combined.length > MAX_HISTORY_POSITIONS
      ? combined.slice(-MAX_HISTORY_POSITIONS)
      : combined;

    newMap.set(flightId, capped);
    histories.value = newMap;
  }

  /**
   * Add a single position to a flight's history.
   */
  function addPosition(flightId: string, position: HistoryPosition) {
    addPositions(flightId, [position]);
  }

  /**
   * Get history for a flight.
   */
  function getHistory(flightId: string): HistoryPosition[] {
    return histories.value.get(flightId) || [];
  }

  /**
   * Check if we have history for a flight.
   */
  function hasHistory(flightId: string): boolean {
    const history = histories.value.get(flightId);
    return history !== undefined && history.length > 0;
  }

  /**
   * Get the most recent position for a flight.
   */
  function getLatestPosition(flightId: string): HistoryPosition | undefined {
    const history = histories.value.get(flightId);
    if (!history || history.length === 0) {
      return undefined;
    }
    return history[history.length - 1];
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Subscription Management
  // ═══════════════════════════════════════════════════════════

  /**
   * Mark a flight as subscribed (SSE connection active).
   * Note: The actual SSE connection is managed by DataIngestionService.
   */
  function subscribe(flightId: string) {
    activeSubscriptions.value.add(flightId);
    // Trigger reactivity
    activeSubscriptions.value = new Set(activeSubscriptions.value);
  }

  /**
   * Mark a flight as unsubscribed.
   */
  function unsubscribe(flightId: string) {
    activeSubscriptions.value.delete(flightId);
    // Trigger reactivity
    activeSubscriptions.value = new Set(activeSubscriptions.value);
  }

  /**
   * Set loading state for a flight.
   */
  function setLoading(flightId: string, isLoading: boolean) {
    const newMap = new Map(loading.value);
    if (isLoading) {
      newMap.set(flightId, true);
    } else {
      newMap.delete(flightId);
    }
    loading.value = newMap;
  }

  // ═══════════════════════════════════════════════════════════
  // ACTIONS - Cleanup
  // ═══════════════════════════════════════════════════════════

  /**
   * Clear history for a specific flight.
   */
  function clearHistory(flightId: string) {
    const newMap = new Map(histories.value);
    newMap.delete(flightId);
    histories.value = newMap;
  }

  /**
   * Clear all history and subscriptions.
   */
  function clearAll() {
    histories.value = new Map();
    activeSubscriptions.value = new Set();
    loading.value = new Map();
  }

  /**
   * Clean up a flight - unsubscribe and optionally clear history.
   */
  function cleanupFlight(flightId: string, clearHistoryData = false) {
    unsubscribe(flightId);
    setLoading(flightId, false);
    if (clearHistoryData) {
      clearHistory(flightId);
    }
  }

  /**
   * Reset the store to initial state.
   */
  function $reset() {
    histories.value = new Map();
    activeSubscriptions.value = new Set();
    loading.value = new Map();
  }

  // ═══════════════════════════════════════════════════════════
  // RETURN
  // ═══════════════════════════════════════════════════════════

  return {
    // State (readonly)
    histories: readonly(histories),
    activeSubscriptions: readonly(activeSubscriptions),
    loading: readonly(loading),

    // Computed
    isSubscribed,
    subscriptionCount,
    isLoading,

    // Actions - Data
    setHistory,
    addPositions,
    addPosition,
    getHistory,
    hasHistory,
    getLatestPosition,

    // Actions - Subscriptions
    subscribe,
    unsubscribe,
    setLoading,

    // Actions - Cleanup
    clearHistory,
    clearAll,
    cleanupFlight,
    $reset,
  };
});
