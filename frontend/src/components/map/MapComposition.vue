<template>
  <MapComponent :apikey="props.apikey || ''" :lat="props.lat || ''" :lng="props.lng || ''" @map-initialized="onMapInitialized" />
</template>

<script setup lang="ts">
import { onBeforeMount, onBeforeUnmount, ref, watch } from 'vue';
import { useAircraftStore, useFlightHistoryStore } from '@/stores/aircraft';
import { useMapStore } from '@/stores/map';
import { useLocationStore } from '@/stores/locationStore';
import { getDataIngestionService } from '@/services/dataIngestionService';
import MapComponent from './MapComponent.vue';
import { useMapRenderer } from '@/composables/useMapRenderer';

const aircraftStore = useAircraftStore();
const historyStore = useFlightHistoryStore();
const mapStore = useMapStore();
const locationStore = useLocationStore();
const dataService = getDataIngestionService();

declare let H: any;

const USER_LOCATION_DOT_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20">
  <circle cx="10" cy="10" r="9" fill="#4285F4" fill-opacity="0.25"/>
  <circle cx="10" cy="10" r="6" fill="#4285F4" stroke="white" stroke-width="2"/>
</svg>`;

let userLocationMarker: any = null;
let locationCentered = false;

function addOrUpdateUserLocationMarker(lat: number, lng: number) {
  if (!map.value) return;
  const coords = { lat, lng };
  if (userLocationMarker) {
    userLocationMarker.setGeometry(coords);
  } else {
    const icon = new H.map.DomIcon(USER_LOCATION_DOT_SVG, {
      anchor: { x: 10, y: 10 },
    });
    userLocationMarker = new H.map.DomMarker(coords, { icon, zIndex: 100 });
    map.value.addObject(userLocationMarker);
  }
}

function removeUserLocationMarker() {
  if (userLocationMarker && map.value) {
    map.value.removeObject(userLocationMarker);
    userLocationMarker = null;
  }
}

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

// Watch for user location changes and update the blue dot marker
watch(
  [() => locationStore.enabled, () => locationStore.lat, () => locationStore.lng],
  ([enabled, lat, lng]) => {
    if (enabled && lat !== null && lng !== null) {
      addOrUpdateUserLocationMarker(lat, lng);
      if (!locationCentered) {
        locationCentered = true;
        mapStore.panTo({ lat, lng }, 13);
      }
    } else {
      removeUserLocationMarker();
      locationCentered = false;
    }
  }
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

  // Apply user location marker if location is already enabled
  if (locationStore.enabled && locationStore.lat !== null && locationStore.lng !== null) {
    addOrUpdateUserLocationMarker(locationStore.lat, locationStore.lng);
    if (!locationCentered) {
      locationCentered = true;
      mapStore.panTo({ lat: locationStore.lat, lng: locationStore.lng }, 13);
    }
  }

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

  // Clean up user location marker
  removeUserLocationMarker();

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
