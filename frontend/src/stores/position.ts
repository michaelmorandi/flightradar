import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { TerrestialPosition } from '@/model/backendModel';
import { differenceInSeconds } from 'date-fns';

export const usePositionStore = defineStore('position', () => {
  // State
  const positions = ref<Map<string, TerrestialPosition>>(new Map());
  const flightPositions = ref<Map<string, TerrestialPosition[]>>(new Map());
  const lastUpdate = ref<Date | null>(null);
  const connectionStatus = ref<'connected' | 'disconnected' | 'connecting'>('disconnected');
  const staleThreshold = ref<number>(15); // seconds

  // Getters
  const getCurrentPositions = computed(() => positions.value);
  
  const getActivePositions = computed(() => {
    if (!lastUpdate.value) return new Map();
    
    const now = new Date();
    const activePositions = new Map<string, TerrestialPosition>();
    
    positions.value.forEach((position, flightId) => {
      // Consider positions active if they were updated within the stale threshold
      // Since TerrestialPosition doesn't have a time field, use lastUpdate
      const positionTime = lastUpdate.value || now;
      if (differenceInSeconds(now, positionTime) <= staleThreshold.value) {
        activePositions.set(flightId, position);
      }
    });
    
    return activePositions;
  });

  const getPositionById = computed(() => {
    return (flightId: string) => positions.value.get(flightId);
  });

  const getFlightHistory = computed(() => {
    return (flightId: string) => flightPositions.value.get(flightId) || [];
  });

  const getPositionCount = computed(() => positions.value.size);
  
  const getActivePositionCount = computed(() => getActivePositions.value.size);

  const isConnected = computed(() => connectionStatus.value === 'connected');

  // Actions
  const updatePositions = (newPositions: Map<string, TerrestialPosition>) => {
    // Merge new positions with existing ones
    newPositions.forEach((position, flightId) => {
      positions.value.set(flightId, position);
    });
    
    lastUpdate.value = new Date();
  };

  const updateSinglePosition = (flightId: string, position: TerrestialPosition) => {
    positions.value.set(flightId, position);
    lastUpdate.value = new Date();
  };

  const updateFlightHistory = (flightId: string, positions: TerrestialPosition[]) => {
    flightPositions.value.set(flightId, positions);
  };

  const addToFlightHistory = (flightId: string, position: TerrestialPosition) => {
    const existing = flightPositions.value.get(flightId) || [];
    const updated = [...existing, position];
    
    // Keep only the last 1000 positions to prevent memory issues
    if (updated.length > 1000) {
      updated.splice(0, updated.length - 1000);
    }
    
    flightPositions.value.set(flightId, updated);
  };

  const removePosition = (flightId: string) => {
    positions.value.delete(flightId);
  };

  const removeFlightHistory = (flightId: string) => {
    flightPositions.value.delete(flightId);
  };

  const purgeStalePositions = () => {
    if (!lastUpdate.value) return;
    
    const now = new Date();
    const staleFlights: string[] = [];
    
    positions.value.forEach((_, flightId) => {
      // Since TerrestialPosition doesn't have a time field, use lastUpdate
      const positionTime = lastUpdate.value || now;
      if (differenceInSeconds(now, positionTime) > staleThreshold.value) {
        staleFlights.push(flightId);
      }
    });
    
    staleFlights.forEach(flightId => {
      positions.value.delete(flightId);
    });
    
    return staleFlights.length;
  };

  const setConnectionStatus = (status: 'connected' | 'disconnected' | 'connecting') => {
    connectionStatus.value = status;
  };

  const updateStaleThreshold = (seconds: number) => {
    staleThreshold.value = seconds;
  };

  const clearAllPositions = () => {
    positions.value.clear();
    flightPositions.value.clear();
    lastUpdate.value = null;
  };

  const clearFlightData = (flightId: string) => {
    removePosition(flightId);
    removeFlightHistory(flightId);
  };

  // Statistics and debugging
  const getPositionStats = computed(() => {
    const total = positions.value.size;
    const active = getActivePositionCount.value;
    const stale = total - active;
    
    return {
      total,
      active,
      stale,
      lastUpdate: lastUpdate.value,
      connectionStatus: connectionStatus.value
    };
  });

  // Reset store state
  const $reset = () => {
    positions.value.clear();
    flightPositions.value.clear();
    lastUpdate.value = null;
    connectionStatus.value = 'disconnected';
    staleThreshold.value = 15;
  };

  return {
    // State
    positions,
    flightPositions,
    lastUpdate,
    connectionStatus,
    staleThreshold,
    
    // Getters
    getCurrentPositions,
    getActivePositions,
    getPositionById,
    getFlightHistory,
    getPositionCount,
    getActivePositionCount,
    isConnected,
    getPositionStats,
    
    // Actions
    updatePositions,
    updateSinglePosition,
    updateFlightHistory,
    addToFlightHistory,
    removePosition,
    removeFlightHistory,
    purgeStalePositions,
    setConnectionStatus,
    updateStaleThreshold,
    clearAllPositions,
    clearFlightData,
    $reset
  };
});