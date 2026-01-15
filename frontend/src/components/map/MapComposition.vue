<template>
  <MapComponent :apikey="props.apikey || ''" :lat="props.lat || ''" :lng="props.lng || ''" @map-initialized="onMapInitialized" />
</template>

<script setup lang="ts">
import { onBeforeMount, onBeforeUnmount, ref, watch } from 'vue';
import { useAircraftStore, useFlightHistoryStore } from '@/stores/aircraft';
import { useMapStore } from '@/stores/map';
import { getDataIngestionService } from '@/services/dataIngestionService';
import MapComponent from './MapComponent.vue';
import { useMapRenderer } from '@/composables/useMapRenderer';

const aircraftStore = useAircraftStore();
const historyStore = useFlightHistoryStore();
const mapStore = useMapStore();
const dataService = getDataIngestionService();

const props = defineProps({
  apikey: String,
  lat: String,
  lng: String,
  aerialOverview: Boolean, // If enabled displays a view of aircaft in the air
  highlightedFlightId: String, // If set displays the flightpath of the selected flight (historical and live)
  peridicallyRefresh: Boolean,
});

const emit = defineEmits(['onMarkerClicked']);

/* eslint-disable */
let map = ref<any>(null);
let intervalId = ref<ReturnType<typeof setTimeout>>();

// Initialize the map renderer composable
const mapRenderer = useMapRenderer(map, (flightId: string) => {
  // Handle marker click - select the flight and emit event
  mapRenderer.selectFlight(flightId);
  mapStore.selectMarker(flightId);
  mapStore.highlightFlight(flightId);
  emit('onMarkerClicked', flightId);
});

onBeforeMount(async () => {
  mapStore.setApiKey(props.apikey || '');
  mapStore.updateConfig({
    aerialOverview: props.aerialOverview || false,
    periodicRefresh: props.peridicallyRefresh || false,
  });
});

// Watch for props changes to highlightedFlightId
watch(
  () => props.highlightedFlightId,
  (newFlightId) => {
    if (newFlightId) {
      mapRenderer.selectFlight(newFlightId);
      mapStore.highlightFlight(newFlightId);
      aircraftStore.selectFlight(newFlightId);
    } else {
      mapRenderer.unselectFlight();
      mapStore.clearHighlight();
      aircraftStore.clearSelection();
    }
  },
);

// Watch for store-based flight selection changes (from other components)
watch(
  () => aircraftStore.selectedFlightId,
  (newFlightId, oldFlightId) => {
    // Only react if the change came from elsewhere (not from our selectFlight)
    if (newFlightId !== mapRenderer.selectedFlightPath.value?.flightId) {
      if (newFlightId) {
        mapRenderer.selectFlight(newFlightId);
      } else if (oldFlightId) {
        mapRenderer.unselectFlight();
      }
    }
  },
);

const onMapInitialized = ({ map: mapInstance }: { map: any; platform: any }) => {
  map.value = mapInstance;

  // Initialize the marker manager with the map instance
  mapRenderer.initializeMarkerManager(mapInstance);

  // Ensure data service is connected (should already be from main.ts)
  if (!dataService.isConnected()) {
    dataService.connect();
  }

  // Initial marker update from current mapView
  mapRenderer.updateMarkers(aircraftStore.mapView);

  if (props.peridicallyRefresh) {
    intervalId.value = setInterval(() => {
      // The map renderer watches the store automatically
      // This interval can be used for other periodic tasks if needed
    }, mapStore.config.refreshInterval);
  } else {
    if (intervalId.value) clearInterval(intervalId.value);
  }
};

onBeforeUnmount(async () => {
  if (intervalId.value) clearInterval(intervalId.value);

  // Clean up the map renderer
  mapRenderer.cleanup();

  mapStore.setInitialized(false);
});

const unselectFlight = () => {
  mapRenderer.unselectFlight();
  mapStore.clearHighlight();
  aircraftStore.clearSelection();
};

defineExpose({
  unselectFlight,
  aircraftStore,
  historyStore,
  mapStore,
  getMarkerManager: () => mapRenderer.markerManager.value,
});
</script>

<style scoped></style>
