<template>
  <div class="flLogEntry">
    <div class="entry-content" @click="toggleMap">
      <div class="silhouette" data-bs-toggle="tooltip" data-bs-placement="left" data-bs-html="true" :data-bs-title="operatorTooltip">
        <img
          :src="silhouetteSrc"
          height="20px"
          @error="onImageError"
        />
      </div>
      <div class="callsign" v-if="flight.cls">
        <span class="badge text-bg-secondary">{{ flight.cls }}</span>
      </div>
      <div class="aircraftType">{{ aircaftTypeTruncated }}</div>
      <div class="operator">{{ aircaftOperatorTruncated }}</div>
      <button
        v-if="hasPositions"
        class="map-toggle-btn"
        @click.stop="toggleMap"
        :class="{ 'active': showMap, 'live': isLive }"
        :aria-label="isLive ? 'View on live map' : 'Toggle map'"
      >
        <i :class="isLive ? 'bi bi-broadcast' : 'bi bi-map'"></i>
      </button>
    </div>
    <FlightLogMiniMap
      :flight-id="flight.id"
      :visible="showMap"
      @close="showMap = false"
    />
  </div>
</template>

<script setup lang="ts">
import { Aircraft, Flight } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import { silhouetteUrl } from '@/components/aircraftIcon';
import { computed, onMounted, ref, PropType } from 'vue';
import { truncate } from '@/utils/string';
import { differenceInMinutes, differenceInHours, startOfDay, format } from 'date-fns';
import { useAircraftStore } from '@/stores/aircraft';
import { useViewStore } from '@/stores/viewStore';
import FlightLogMiniMap from './FlightLogMiniMap.vue';

const props = defineProps({
  flight: { type: Object as PropType<Flight>, required: true },
});

const apiService = getFlightApiService();
const aircraftStore = useAircraftStore();
const viewStore = useViewStore();

const GENERIC_SILHOUETTE = '/silhouettes/generic.png';

const aircraft = ref<Aircraft>({ icao24: '' });
const showMap = ref(false);
const imageError = ref(false);

const silhouetteSrc = computed(() => {
  if (imageError.value || !aircraft.value.icaoType) {
    return GENERIC_SILHOUETTE;
  }
  return silhouetteUrl(aircraft.value.icaoType);
});

const onImageError = () => {
  imageError.value = true;
};

onMounted(async () => {
  try {
    const ac = await apiService.getAircraft(props.flight.icao24);
    if (ac) {
      aircraft.value = ac;
    }
  } catch (err) {
    // Aircraft details not available
  }
});

const operatorTooltip = computed(() => {
  let tooltipContent = `<strong>ICAO 24-bit: </strong> ${props.flight?.icao24?.toUpperCase()}<br/>`;
  if (aircraft.value.reg) tooltipContent += `<strong>Registration:</strong> ${aircraft.value.reg}<br/>`;
  // Don't show timestamp for live/tracking aircraft
  if (!isLive.value) {
    tooltipContent += `${timestampTooltip.value}`;
  }
  return tooltipContent;
});

const getTimestampString = (timestamp: Date): string => {
  const now = new Date();
  const todayMidnight = startOfDay(now);
  const hoursSinceTodayMidnight = differenceInHours(timestamp, todayMidnight);
  let timestmpStr = '';

  if (hoursSinceTodayMidnight >= 0) {
    // Today
    const minutes = differenceInMinutes(now, timestamp);

    if (isLive.value) {
      timestmpStr = minutes <= 0 ? 'tracking' : `${minutes} minutes ago`;
    } else {
      timestmpStr = `Today, ${format(timestamp, 'HH:mm')}`;
    }
  } else if (hoursSinceTodayMidnight >= -24) {
    // Yesterday
    timestmpStr = `Yesterday, ${format(timestamp, 'HH:mm')}`;
  } else {
    // Older
    timestmpStr = format(timestamp, 'd.M.yyyy HH:mm');
  }

  return timestmpStr;
};

const timestampTooltip = computed(() => {
  const lastContact = new Date(props.flight.lstCntct);
  const lastContactStr: string = getTimestampString(lastContact);
  return `<i class="bi bi-radar"></i> ${lastContactStr}`;
});

const isLive = computed(() => {
  const lastContact = new Date(props.flight.lstCntct);
  return differenceInMinutes(new Date(), lastContact) < 5;
});

const hasPositions = computed(() => {
  return props.flight.positionCount !== undefined && props.flight.positionCount > 0;
});

const aircaftOperatorTruncated = computed(() => {
  return truncate(aircraft.value.op, 28);
});

const aircaftTypeTruncated = computed(() => {
  return truncate(aircraft.value.type, 37);
});

const toggleMap = () => {
  if (isLive.value) {
    // For live aircraft, switch to live view and select the flight
    aircraftStore.selectFlight(props.flight.id);
    viewStore.showLive();
  } else {
    // For past flights, toggle the inline mini map
    showMap.value = !showMap.value;
  }
};
</script>

<style scoped>
.flLogEntry {
  font-size: 0.95em;
  border-bottom: solid 1px #e0e0e0;
  max-width: 800px;
  transition: background-color 0.15s ease;
}

.flLogEntry:hover {
  background-color: #f8f9fa;
}

.entry-content {
  height: 45px;
  position: relative;
  display: flex;
  align-items: center;
  padding: 8px 0;
  cursor: pointer;
}

.silhouette {
  position: absolute;
  left: 8px;
  display: flex;
  align-items: center;
}

.callsign {
  position: absolute;
  left: 100px;
}

.aircraftType {
  position: absolute;
  left: 200px;
  color: #212529;
  font-weight: 500;
}

.operator {
  position: absolute;
  left: 540px;
  color: #6c757d;
}

.map-toggle-btn {
  position: absolute;
  right: 8px;
  background: none;
  border: 1px solid #dee2e6;
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  color: #495057;
  transition: all 0.15s ease;
  font-size: 0.9rem;
}

.map-toggle-btn:hover {
  background: #e9ecef;
  border-color: #adb5bd;
  color: #212529;
}

.map-toggle-btn.active {
  background: #0d6efd;
  border-color: #0d6efd;
  color: #fff;
}

.map-toggle-btn.active:hover {
  background: #0b5ed7;
  border-color: #0b5ed7;
}

.map-toggle-btn.live {
  background: #f0fdf4;
  border-color: #86efac;
  color: #16a34a;
}

.map-toggle-btn.live:hover {
  background: #dcfce7;
  border-color: #4ade80;
}
</style>
