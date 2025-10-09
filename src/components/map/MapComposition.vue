<template>
  <MapComponent :apikey="props.apikey || ''" :lat="props.lat || ''" :lng="props.lng || ''" @map-initialized="onMapInitialized" />
</template>

<script setup lang="ts">
import { inject, onBeforeMount, onBeforeUnmount, ref, watch } from 'vue';
import { TerrestialPosition } from '@/model/backendModel';
import { FlightRadarService } from '@/services/flightRadarService';
import { useFlightStore, usePositionStore, useMapStore, useConnectionStore } from '@/stores';
import MapComponent from './MapComponent.vue';
import { MarkerManager } from './MarkerManager';
import { FlightPathManager } from './FlightPathManager';

const radarService = inject('frService') as FlightRadarService;

const flightStore = useFlightStore();
const positionStore = usePositionStore();
const mapStore = useMapStore();
const connectionStore = useConnectionStore();

const props = defineProps({
  apikey: String,
  lat: String,
  lng: String,
  aerialOverview: Boolean, // If enabled displays a view of aircaft in the air
  highlightedFlightId: String, //If set displays the flightpath of the selected flight (historical and live)
  peridicallyRefresh: Boolean,
});

const emit = defineEmits(['onMarkerClicked']);

/* eslint-disable */
let map: any;
let intervalId = ref<ReturnType<typeof setTimeout>>();
let markerManager: MarkerManager;
let flightPathManager: FlightPathManager;


onBeforeMount(async () => {
  mapStore.setApiKey(props.apikey || '');
  mapStore.updateConfig({
    aerialOverview: props.aerialOverview || false,
    periodicRefresh: props.peridicallyRefresh || false,
  });
});

// Watch for props changes
watch(
  () => props.highlightedFlightId,
  (newFlightId) => {
    if (flightPathManager) {
      flightPathManager.handlePropsFlightIdChange(newFlightId || null);
    }
  },
);

// Watch for store-based flight selection changes
watch(
  () => flightStore.selectedFlightId,
  (newFlightId, oldFlightId) => {
    if (flightPathManager) {
      flightPathManager.handleFlightSelectionChange(newFlightId || null, oldFlightId || null);
    }
  },
);

const onMapInitialized = ({ map: mapInstance }: { map: any; platform: any }) => {
  map = mapInstance;

  markerManager = new MarkerManager(map, positionStore);
  flightPathManager = new FlightPathManager(map, radarService, flightStore, mapStore, connectionStore, markerManager);

  markerManager.setOnMarkerClickCallback((flightId: string) => {
    flightPathManager.selectFlight(flightId);
    emit('onMarkerClicked', flightId);
  });

  connectionStore.registerPositionsConnection(radarService, (positions: Map<string, TerrestialPosition>) => {
    if (positions && markerManager) {
      markerManager.updateAircraftPositions(positions);
    }
  });

  updateData();

  if (props.peridicallyRefresh) {
    intervalId.value = setInterval(() => {
      // Periodic refresh is handled by position streams
      // This interval can be used for other periodic tasks if needed
    }, mapStore.config.refreshInterval);
  } else {
    if (intervalId.value) clearInterval(intervalId.value);
  }
};

const updateData = () => {
  if (mapStore.isAerialViewEnabled) {
    loadLivePositions();
  }
  // Flight path updates are now handled by position streams
};

onBeforeUnmount(async () => {
  if (intervalId.value) clearInterval(intervalId.value);

  if (flightPathManager) {
    flightPathManager.cleanup();
  }
  if (markerManager) {
    markerManager.clearAllMarkers();
  }

  connectionStore.disconnectAllConnections(radarService);

  mapStore.setInitialized(false);
});

const loadLivePositions = async () => {
  // Try to get positions from store first
  const storePositions = positionStore.getCurrentPositions;
  if (storePositions.size > 0 && markerManager) {
    markerManager.updateAircraftPositions(storePositions);
    return;
  } else { // Fallback to service if store is empty    
    const positions = radarService.getCurrentPositions();
    if (positions && positions.size > 0 && markerManager) {
      markerManager.updateAircraftPositions(positions);
    }
  }
};

const unselectFlight = () => {
  if (flightPathManager) {
    flightPathManager.unselectFlight();
  }
};

defineExpose({
  unselectFlight,
  flightStore,
  positionStore,
  mapStore,
  connectionStore,
  getMarkerManager: () => markerManager,
  getFlightPathManager: () => flightPathManager,
});
</script>

<style scoped></style>
