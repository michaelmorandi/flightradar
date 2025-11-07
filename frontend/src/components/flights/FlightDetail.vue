<template>
  <div class="container text-center">
    <h1 class="title">{{ flight ? flight.cls : null }}</h1>
    <div class="row">
      <div class="col">
        <DetailField label="Silhouette" :imageUrl="silhouetteUrl(aircraft && aircraft.icaoType ? aircraft.icaoType : '')" />
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
        <DetailField :label="typeLabel" :text="aircraft ? aircraft.type : null" />       
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
import { Flight, Aircraft, TerrestialPosition } from '@/model/backendModel';
import { FlightRadarService } from '@/services/flightRadarService';
import { computed, inject, watch, ref, onMounted, onBeforeUnmount } from 'vue';
import { silhouetteUrl } from '@/components/aircraftIcon';

const props = defineProps({
  flightId: String,
});

const flight = ref<Flight>();
const aircraft = ref<Aircraft>();
const currentPosition = ref<TerrestialPosition | null>(null);
const routeInfo = ref<string | null>(null);
let positionUpdateInterval: number | null = null;
// Track active position stream
let currentFlightId: string | null = null;

const frService = inject('frService') as FlightRadarService;

// Fetch route information using service
const fetchRouteInfo = (callsign: string) => {
  frService.getFlightRoute(callsign).subscribe({
    next: (route) => {
      routeInfo.value = route;
    },
    error: (error) => {
      console.error('Error fetching route information:', error);
      routeInfo.value = null;
    },
  });
};

// Set up position stream for updates
const setupPositionStream = (flightId: string) => {
  frService.registerFlightPositionsCallback(flightId, (positions: Array<TerrestialPosition>) => {
    // Only update if this is still the selected flight
    if (props.flightId === flightId && positions && positions.length > 0) {
      // Always use the most recent position from the array
      const latestPosition = positions[positions.length - 1];
      if (latestPosition && latestPosition.alt !== undefined) {
        currentPosition.value = latestPosition;
      }
    }
  });
};

const updateCurrentPosition = () => {
  if (!props.flightId) return;

  const requestedFlightId = props.flightId;

  try {
    // First check if we have a live position from the main position stream
    const livePosition = frService.getCurrentPosition(requestedFlightId);

    // Only update if this is still the selected flight
    if (props.flightId === requestedFlightId && livePosition && livePosition.alt !== undefined) {
      currentPosition.value = livePosition;
      return;
    }

    // Fallback: try to get historical positions if live data doesn't have altitude
    frService.getPositions(requestedFlightId).subscribe({
      next: (positions) => {
        // Only update if this is still the selected flight
        if (props.flightId === requestedFlightId && positions && positions.length > 0) {
          const latestPosition = positions[positions.length - 1];

          if (latestPosition && latestPosition.alt !== undefined) {
            currentPosition.value = latestPosition;
          }
        }
      },
      error: (error) => {
        console.error('Error fetching current position:', error);
      },
    });
  } catch (error) {
    console.error('Error fetching current position:', error);
  }
};

watch(
  () => props.flightId,
  (newValue, _oldValue) => {
    // Clear current position when changing flights
    currentPosition.value = null;
    routeInfo.value = null;

    // Disconnect any existing position stream
    if (currentFlightId) {
      frService.disconnectFlightPositions(currentFlightId);
      currentFlightId = null;
    }

    if (newValue) {
      // Reset references
      flight.value = undefined;
      aircraft.value = undefined;

      // Fetch new flight data using observables
      frService.getFlight(newValue).subscribe({
        next: (flightData) => {
          flight.value = flightData;

          // Fetch route information if callsign is available
          if (flightData && flightData.cls) {
            fetchRouteInfo(flightData.cls);
          }

          if (flightData && flightData.icao24) {
            // Once we have the flight, get the aircraft data
            frService.getAircraft(flightData.icao24).subscribe({
              next: (aircraftData) => {
                aircraft.value = aircraftData;

                // Set up position stream for updates
                currentFlightId = newValue;
                setupPositionStream(newValue);

                // Get the current position (might be available immediately)
                updateCurrentPosition();
              },
              error: (err) => {
                console.error(err);
                aircraft.value = undefined;
              },
            });
          }
        },
        error: (err) => {
          console.error(err);
          flight.value = undefined;
        },
      });
    }
  },
);

onMounted(() => {
  // Get initial position data and set up position stream if needed
  if (props.flightId) {
    updateCurrentPosition();
    if (!currentFlightId) {
      currentFlightId = props.flightId;
      setupPositionStream(props.flightId);
    }
  }

  // Maintain the interval as a fallback, but make it less frequent
  positionUpdateInterval = window.setInterval(() => {
    if (props.flightId && !currentPosition.value) {
      updateCurrentPosition();
    }
  }, 5000); // Fallback check every 5 seconds if no position is available
});

onBeforeUnmount(() => {
  // Clean up interval
  if (positionUpdateInterval) {
    window.clearInterval(positionUpdateInterval);
    positionUpdateInterval = null;
  }

  // Disconnect position stream
  if (currentFlightId) {
    frService.disconnectFlightPositions(currentFlightId);
    currentFlightId = null;
  }
});

const currentAltitude = computed(() => {
  if (currentPosition?.value?.alt !== undefined && currentPosition.value.alt !== null && currentPosition.value.alt >= 0) {
    return `${currentPosition.value?.alt.toLocaleString()} ft`;
  }

  return undefined;
});

const currentGroundSpeed = computed(() => {
  if (currentPosition?.value?.gs !== undefined && currentPosition.value.gs !== null && currentPosition.value.gs >= 0) {
    return `${currentPosition.value.gs} kts`;
  }

  return undefined;
});

const typeLabel = computed(() => {
  return `Type (${aircraft.value?.icaoType ? aircraft.value.icaoType : 'Type'})`;
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
