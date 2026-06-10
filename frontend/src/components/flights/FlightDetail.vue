<template>
  <div class="container text-center">
    <h1 class="title">{{ flight ? flight.callsign : null }}</h1>
    <div class="row">
      <div class="col">
        <DetailField label="Silhouette" :imageUrl="silhouetteUrl(aircraft && aircraft.type_code ? aircraft.type_code : '')" :showGenericFallback="true" />
      </div>
      <div class="col" v-if="routeInfo">
        <div class="route-field">
          <div class="labelText">Route</div>
          <div class="valueText route-value">
            <AirportIataCode :iata="departureAirport!" /> ➞ <AirportIataCode :iata="arrivalAirport!" />
          </div>
        </div>
      </div>
    </div>
    <div class="row">
      <div class="col">
        <DetailField label="24 bit address" :text="flight && flight.icao24 ? flight.icao24.toUpperCase() : null" />
      </div>
      <div class="col">
        <DetailField label="Registraton" :text="aircraft ? aircraft.registration : null" />
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
        <DetailField :label="typeLabel" :text="aircraft ? aircraft.type_description : null" :tooltip="categoryTooltip" />
      </div>
    </div>
    <div class="row" v-if="aircraft">
      <div class="col">
        <div style="position: relative; height: 45px">
          <div class="operator-label">Operator</div>
          <img
            v-if="flight?.airline_icao && !airlineLogoError"
            :src="`https://raw.githubusercontent.com/Jxck-S/airline-logos/main/fr24_banners/${flight.airline_icao}.png`"
            :alt="flight.airline_icao"
            :title="aircraft.operator"
            class="airline-banner"
            @error="airlineLogoError = true"
          />
          <div v-else class="operator-value">{{ aircraft.operator }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import DetailField from '@/components/flights/DetailField.vue';
import AirportIataCode from '@/components/flights/AirportIataCode.vue';
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
const airlineLogoError = ref(false);

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

// Subscribe to live position updates for this aircraft. The Rust
// backend keys its single-aircraft SSE stream by ICAO24 rather than
// the Mongo flight ObjectId, so we pass the icao24 of the currently
// loaded flight — not the flight id from the URL.
const setupFlightSubscription = (icao24: string) => {
  if (currentFlightId === icao24) return;

  if (currentFlightId && dataService.isSubscribedToFlight(currentFlightId)) {
    dataService.unsubscribeFromFlight(currentFlightId);
  }

  currentFlightId = icao24;
  dataService.subscribeToFlight(icao24);
};

// Load flight and aircraft data
const loadFlightData = async (flightId: string) => {
  // Clear previous data
  flight.value = undefined;
  aircraft.value = undefined;
  routeInfo.value = null;
  airlineLogoError.value = false;

  try {
    // Fetch flight details
    const flightData = await apiService.getFlight(flightId);
    if (flightData) {
      flight.value = flightData;

      // Fetch route info if callsign is available
      if (flightData.callsign) {
        fetchRouteInfo(flightData.callsign);
      }

      // Fetch aircraft details - check cache first
      if (flightData.icao24) {
        // Check store cache first for optimal performance
        let cachedDetails = aircraftStore.getAircraftDetails(flightData.icao24);

        if (cachedDetails) {
          // Use cached data; map the internal store shape onto the
          // backend DTO shape that this component renders.
          aircraft.value = {
            icao24: cachedDetails.icao24,
            type_description: cachedDetails.type,
            type_code: cachedDetails.icaoType,
            registration: cachedDetails.registration,
            operator: cachedDetails.operator,
          };
        } else {
          // Not in cache - fetch from API
          const aircraftData = await apiService.getAircraft(flightData.icao24);
          if (aircraftData) {
            aircraft.value = aircraftData;

            // Cache the result in the store for future use
            aircraftStore.cacheAircraftDetails({
              icao24: flightData.icao24,
              type: aircraftData.type_description,
              icaoType: aircraftData.type_code,
              registration: aircraftData.registration,
              operator: aircraftData.operator,
            });
          }
        }
      }

      // Seed the history store with the persisted flight track so the
      // path renders immediately, then attach the live stream for
      // continued updates. Without this the path would start at "now"
      // because the SSE snapshot only carries the current position.
      if (flightData.icao24) {
        try {
          const positions = await apiService.getPositions(flightId);
          if (positions.length > 0) {
            const seeded = positions.map((p) => ({
              lat: p.lat,
              lon: p.lon,
              altitude: p.alt_ft,
              groundSpeed: p.ground_speed_kt,
              track: p.track_deg,
              timestamp: p.observed_at ? Date.parse(p.observed_at) : Date.now(),
            }));
            historyStore.setHistory(flightData.icao24, seeded);
          }
        } catch (err) {
          console.warn('Could not pre-load flight history:', err);
        }

        // Subscribe by ICAO24, not by flight id — see comment above.
        setupFlightSubscription(flightData.icao24);
      }
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
  return `Type (${aircraft.value?.type_code ? aircraft.value.type_code : 'Type'})`;
});

const categoryTooltip = computed(() => {
  let category;

  // First try to get category from live position data
  const liveState = currentAircraftState.value;
  if (liveState?.category !== undefined && liveState.category > 1) {
    category = mapProtobufCategoryToIcon(liveState.category);
  }

  // Fall back to determining category from aircraft type
  if ((!category || category === 'default') && (aircraft.value?.type_code || aircraft.value?.type_description)) {
    category = determineAircraftCategory(aircraft.value.type_code, aircraft.value.type_description);
  }

  if (category && category !== 'default') {
    const description = AIRCRAFT_CATEGORIES[category];
    return `${category}: ${description}`;
  }
  return undefined;
});

const departureAirport = computed(() => {
  if (routeInfo.value) {
    return routeInfo.value.split('-')[0];
  }
  return null;
});

const arrivalAirport = computed(() => {
  if (routeInfo.value) {
    const parts = routeInfo.value.split('-');
    return parts.length > 1 ? parts[1] : null;
  }
  return null;
});
</script>

<style scoped>
.title {
  font-size: 2em;
  text-align: left;
}

.route-field {
  position: relative;
  height: 45px;
}

.route-field .labelText {
  font-size: 0.7em;
  text-transform: uppercase;
  color: rgb(100, 100, 100);
  position: absolute;
  top: 0px;
  left: 0px;
}

.route-field .route-value {
  text-align: left;
  font-size: 1em;
  position: absolute;
  top: 15px;
  left: 0px;
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
  position: absolute;
  top: 0px;
  left: 0px;
}

.operator-label {
  font-size: 0.7em;
  text-transform: uppercase;
  color: rgb(100, 100, 100);
  position: absolute;
  top: 0px;
  left: 0px;
}

.operator-value {
  font-size: 1em;
  position: absolute;
  top: 15px;
  left: 0px;
}

.airline-banner {
  padding-top: 5px;
  height: 25px;
  object-fit: contain;
  opacity: 0.85;
  position: absolute;
  top: 15px;
  left: 0px;
}
</style>
