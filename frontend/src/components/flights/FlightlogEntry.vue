<template>
  <div class="flLogEntry">
    <div class="entry-content" :class="{ 'clickable': hasPositions || isLive }" @click="toggleMap">
      <div
        v-if="!singleAircraft"
        class="silhouette"
        data-bs-toggle="tooltip"
        data-bs-placement="left"
        data-bs-html="true"
        :data-bs-title="operatorTooltip"
        @click.stop="onAircraftClick"
      >
        <img
          :src="silhouetteSrc"
          @error="onImageError"
        />
      </div>
      <div class="callsign" v-if="flight.cls">
        <span
          class="badge text-bg-secondary callsign-badge"
          :class="{ 'has-airline': !!flight.airlineIcao }"
          @click.stop="onCallsignClick"
          :title="flight.airlineIcao ? `Show all ${flight.airlineIcao} flights` : undefined"
        >{{ flight.cls }}</span>
      </div>
      <div v-if="singleAircraft" class="flight-time">{{ flightTimestamp }}</div>
      <div v-if="!singleAircraft" class="aircraftType">{{ aircaftTypeTruncated }}</div>
      <div
        v-if="!singleAircraft"
        class="operator"
        :class="{ 'operator-clickable': !!flight.airlineIcao }"
        @click.stop="onOperatorClick"
        :title="flight.airlineIcao ? `Show all ${flight.airlineIcao} flights` : undefined"
      >{{ aircaftOperatorTruncated }}</div>
      <button
        v-if="hasPositions"
        class="map-toggle-btn"
        @click.stop="toggleMap"
        :class="{ 'active': showMap, 'live': isLive }"
        :aria-label="isLive ? 'View on live map' : 'Toggle map'"
      >
        <i :class="isLive ? 'bi bi-broadcast' : 'bi bi-geo-alt'"></i>
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
  singleAircraft: { type: Boolean, default: false },
});

const emit = defineEmits<{
  (e: 'filter-airline', airlineIcao: string): void;
  (e: 'filter-aircraft', icao24: string): void;
}>();

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
  let tooltipContent = `<strong>ICAO24: </strong> ${props.flight?.icao24?.toUpperCase()}<br/>`;
  if (aircraft.value.reg) tooltipContent += `<strong>Registration:</strong> ${aircraft.value.reg}<br/>`;
  tooltipContent += `<small style="color:#6ea8fe">Click icon to show all flights</small>`;
  if (!isLive.value) {
    tooltipContent += `<br/>${timestampTooltip.value}`;
  }
  return tooltipContent;
});

const getTimestampString = (timestamp: Date): string => {
  const now = new Date();
  const todayMidnight = startOfDay(now);
  const hoursSinceTodayMidnight = differenceInHours(timestamp, todayMidnight);
  let timestmpStr = '';

  if (hoursSinceTodayMidnight >= 0) {
    const minutes = differenceInMinutes(now, timestamp);

    if (isLive.value) {
      timestmpStr = minutes <= 0 ? 'tracking' : `${minutes} minutes ago`;
    } else {
      timestmpStr = `Today, ${format(timestamp, 'HH:mm')}`;
    }
  } else if (hoursSinceTodayMidnight >= -24) {
    timestmpStr = `Yesterday, ${format(timestamp, 'HH:mm')}`;
  } else {
    timestmpStr = format(timestamp, 'd.M.yyyy HH:mm');
  }

  return timestmpStr;
};

const timestampTooltip = computed(() => {
  const lastContact = new Date(props.flight.lstCntct);
  const lastContactStr: string = getTimestampString(lastContact);
  return `<i class="bi bi-radar"></i> ${lastContactStr}`;
});

const flightTimestamp = computed(() => {
  return getTimestampString(new Date(props.flight.firstCntct));
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
    aircraftStore.selectFlight(props.flight.id);
    viewStore.showLive();
  } else if (hasPositions.value) {
    showMap.value = !showMap.value;
  }
};

const onCallsignClick = () => {
  if (props.flight.airlineIcao) {
    emit('filter-airline', props.flight.airlineIcao);
  }
};

const onOperatorClick = () => {
  if (props.flight.airlineIcao) {
    emit('filter-airline', props.flight.airlineIcao);
  }
};

const onAircraftClick = () => {
  emit('filter-aircraft', props.flight.icao24);
};
</script>

<style scoped>
.flLogEntry {
  font-size: 0.95em;
  border-bottom: solid 1px #e0e0e0;
  transition: background-color 0.15s ease;
}

.flLogEntry:hover {
  background-color: #f8f9fa;
}

.entry-content {
  min-height: 45px;
  display: flex;
  align-items: center;
  padding: 8px;
  gap: 8px;
}

.entry-content.clickable {
  cursor: pointer;
}

.silhouette {
  flex-shrink: 0;
  width: 90px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.silhouette img {
  max-width: 100%;
  height: auto;
}

.silhouette:hover img {
  filter: brightness(0.7);
}

.callsign {
  flex-shrink: 0;
  width: 80px;
}

.callsign-badge.has-airline {
  cursor: pointer;
  transition: all 0.15s ease;
}

.callsign-badge.has-airline:hover {
  background-color: #495057 !important;
  transform: scale(1.05);
}

.flight-time {
  flex: 1;
  min-width: 0;
  color: #6c757d;
  font-size: 0.85rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aircraftType {
  flex: 1;
  min-width: 0;
  color: #212529;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.operator {
  flex: 1;
  min-width: 0;
  color: #6c757d;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: right;
}

.operator-clickable {
  cursor: pointer;
  transition: color 0.15s ease;
}

.operator-clickable:hover {
  color: #0d6efd;
}

.map-toggle-btn {
  flex-shrink: 0;
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

/* Mobile responsive */
@media (max-width: 640px) {
  .entry-content {
    gap: 6px;
    padding: 8px 6px;
  }

  .silhouette {
    width: 68px;
  }

  .callsign {
    width: auto;
    flex-shrink: 0;
  }

  .callsign .badge {
    font-size: 0.72rem;
    padding: 3px 6px;
  }

  .aircraftType {
    display: none;
  }

  .operator {
    flex: 1;
    font-size: 0.82rem;
  }

  .map-toggle-btn {
    padding: 5px 8px;
    font-size: 0.82rem;
  }
}
</style>
