<template>
  <div class="container text-center">
    <h1 class="title">{{ flight ? flight.cls : null }}</h1>
    <div class="row">
      <div class="col">
        <DetailField label="Silhouette" :imageUrl="silhouetteUrl(aircraft && aircraft.icaoType ? aircraft.icaoType : '')" :showGenericFallback="true" />
      </div>
      <div class="col" v-if="routeInfo">
        <DetailField label="Route" :text="formattedRoute" />
      </div>
    </div>
    <div class="row">
      <div class="col">
        <DetailField label="24 bit address" :text="flight && flight.icao24 ? flight.icao24.toUpperCase() : null" />
      </div>
      <div class="col">
        <DetailField label="Registraton" :text="aircraft ? aircraft.reg : null" />
      </div>
    </div>
    <div class="row" v-if="currentAltitude || currentGroundSpeed">
      <div class="col">
        <DetailField label="Current Altitude" :text="currentAltitude" />
      </div>
      <div class="col">
        <DetailField label="Ground Speed" :text="currentGroundSpeed" />
      </div>
    </div>
    <div class="row" v-if="aircraft">
      <div class="col">
        <DetailField :label="typeLabel" :text="aircraft ? aircraft.type : null" :tooltip="categoryTooltip" />
      </div>
    </div>
    <div class="row" v-if="aircraft">
      <div class="col">
        <DetailField label="Operator" :text="aircraft ? aircraft.op : null" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import DetailField from '@/components/flights/DetailField.vue';
import { Flight, Aircraft } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import { getDataIngestionService } from '@/services/dataIngestionService';
import { useAircraftStore, useFlightHistoryStore } from '@/stores/aircraft';
import { computed, watch, ref, onMounted, onBeforeUnmount } from 'vue';
import { silhouetteUrl } from '@/components/aircraftIcon';
import { mapProtobufCategoryToIcon, AIRCRAFT_CATEGORIES, determineAircraftCategory } from '@/utils/aircraftIcons';

const props = defineProps({
  flightId: String,
});

const flight = ref<Flight>();
const aircraft = ref<Aircraft>();
const routeInfo = ref<string | null>(null);

const apiService = getFlightApiService();
const dataService = getDataIngestionService();
const aircraftStore = useAircraftStore();
const historyStore = useFlightHistoryStore();

// Track active subscription
let currentFlightId: string | null = null;

// Get current position from the aircraft store (reactive)
const currentAircraftState = computed(() => {
  if (!props.flightId) return null;
  return aircraftStore.getAircraftById(props.flightId);
});

// Fetch route information
const fetchRouteInfo = async (callsign: string) => {
  try {
    routeInfo.value = await apiService.getFlightRoute(callsign);
  } catch (error) {
    console.error('Error fetching route information:', error);
    routeInfo.value = null;
  }
};

// Subscribe to flight history for position updates
const setupFlightSubscription = (flightId: string) => {
  if (currentFlightId === flightId) return;

  // Unsubscribe from previous flight
  if (currentFlightId && dataService.isSubscribedToFlight(currentFlightId)) {
    dataService.unsubscribeFromFlight(currentFlightId);
  }

  currentFlightId = flightId;

  // Subscribe to the new flight for history updates
  dataService.subscribeToFlight(flightId);
};

// Load flight and aircraft data
const loadFlightData = async (flightId: string) => {
  // Clear previous data
  flight.value = undefined;
  aircraft.value = undefined;
  routeInfo.value = null;

  try {
    // Fetch flight details
    const flightData = await apiService.getFlight(flightId);
    if (flightData) {
      flight.value = flightData;

      // Fetch route info if callsign is available
      if (flightData.cls) {
        fetchRouteInfo(flightData.cls);
      }

      // Fetch aircraft details - check cache first
      if (flightData.icao24) {
        // Check store cache first for optimal performance
        let cachedDetails = aircraftStore.getAircraftDetails(flightData.icao24);

        if (cachedDetails) {
          // Use cached data
          aircraft.value = {
            icao24: cachedDetails.icao24,
            type: cachedDetails.type,
            icaoType: cachedDetails.icaoType,
            reg: cachedDetails.registration,
            op: cachedDetails.operator,
          };
        } else {
          // Not in cache - fetch from API
          const aircraftData = await apiService.getAircraft(flightData.icao24);
          if (aircraftData) {
            aircraft.value = aircraftData;

            // Cache the result in the store for future use
            aircraftStore.cacheAircraftDetails({
              icao24: flightData.icao24,
              type: aircraftData.type,
              icaoType: aircraftData.icaoType,
              registration: aircraftData.reg,
              operator: aircraftData.op,
            });
          }
        }
      }

      // Subscribe to flight position updates
      setupFlightSubscription(flightId);
    }
  } catch (error) {
    console.error('Error loading flight data:', error);
  }
};

watch(
  () => props.flightId,
  (newValue, _oldValue) => {
    if (newValue) {
      loadFlightData(newValue);
    } else {
      // Clear data
      flight.value = undefined;
      aircraft.value = undefined;
      routeInfo.value = null;

      // Unsubscribe from current flight
      if (currentFlightId && dataService.isSubscribedToFlight(currentFlightId)) {
        dataService.unsubscribeFromFlight(currentFlightId);
        currentFlightId = null;
      }
    }
  },
);

onMounted(() => {
  if (props.flightId) {
    loadFlightData(props.flightId);
  }
});

onBeforeUnmount(() => {
  // Unsubscribe from flight position updates
  if (currentFlightId && dataService.isSubscribedToFlight(currentFlightId)) {
    dataService.unsubscribeFromFlight(currentFlightId);
    currentFlightId = null;
  }
});

const currentAltitude = computed(() => {
  // First check live data from aircraft store
  const liveState = currentAircraftState.value;
  if (liveState?.altitude !== undefined && liveState.altitude >= 0) {
    return `${liveState.altitude.toLocaleString()} ft`;
  }

  // Fallback to history if available
  if (props.flightId) {
    const history = historyStore.getHistory(props.flightId);
    if (history.length > 0) {
      const latest = history[history.length - 1];
      if (latest.altitude !== undefined && latest.altitude >= 0) {
        return `${latest.altitude.toLocaleString()} ft`;
      }
    }
  }

  return undefined;
});

const currentGroundSpeed = computed(() => {
  // First check live data from aircraft store
  const liveState = currentAircraftState.value;
  if (liveState?.groundSpeed !== undefined && liveState.groundSpeed >= 0) {
    return `${Math.round(liveState.groundSpeed)} kts`;
  }

  // Fallback to history if available
  if (props.flightId) {
    const history = historyStore.getHistory(props.flightId);
    if (history.length > 0) {
      const latest = history[history.length - 1];
      if (latest.groundSpeed !== undefined && latest.groundSpeed >= 0) {
        return `${Math.round(latest.groundSpeed)} kts`;
      }
    }
  }

  return undefined;
});

const typeLabel = computed(() => {
  return `Type (${aircraft.value?.icaoType ? aircraft.value.icaoType : 'Type'})`;
});

const categoryTooltip = computed(() => {
  let category;

  // First try to get category from live position data
  const liveState = currentAircraftState.value;
  if (liveState?.category !== undefined && liveState.category > 1) {
    category = mapProtobufCategoryToIcon(liveState.category);
  }

  // Fall back to determining category from aircraft type
  if ((!category || category === 'default') && (aircraft.value?.icaoType || aircraft.value?.type)) {
    category = determineAircraftCategory(aircraft.value.icaoType, aircraft.value.type);
  }

  if (category && category !== 'default') {
    const description = AIRCRAFT_CATEGORIES[category];
    return `${category}: ${description}`;
  }
  return undefined;
});

const formattedRoute = computed(() => {
  if (routeInfo.value) {
    return routeInfo.value.replace(/-/g, '➞');
  }
  return null;
});
</script>

<style scoped>
.title {
  font-size: 2em;
  text-align: left;
}

.categoryText {
  font-size: 0.6em;
  text-transform: uppercase;
  color: rgb(100, 100, 100);
  position: absolute;
  top: 0px;
  left: 0px;
  border: 1px solid;
}

.valueText {
  font-size: 0.6em;
  text-transform: uppercase;
  color: rgb(100, 100, 100);
  position: absolute;
  top: 0px;
  left: 0px;
  border: 1px solid;
}
</style>
