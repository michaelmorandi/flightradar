<template>
  <div class="flight-log-view">
    <div class="filter-bar">
      <button
        type="button"
        class="filter-chip"
        @click="toggleMilitaryFilter"
        :class="{ 'active': militaryFilter }"
      >
        <i class="bi bi-shield-fill"></i>
        Military
      </button>
      <button
        type="button"
        class="filter-chip"
        @click="toggleLiveFilter"
        :class="{ 'active': liveFilter }"
      >
        <i class="bi bi-broadcast"></i>
        Live
      </button>
      <div class="refresh-indicator" v-if="loading && flights.length > 0">
        <div class="spinner-border spinner-border-sm" role="status">
          <span class="visually-hidden">Refreshing...</span>
        </div>
      </div>
    </div>

    <div v-if="filteredFlights.length === 0 && loading" class="loading-state">
      <div class="spinner-border" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
      <p>Loading flights...</p>
    </div>

    <div v-else-if="filteredFlights.length === 0 && !loading" class="empty-state">
      <i class="bi bi-inbox"></i>
      <p>No flights found</p>
    </div>

    <div v-else class="flight-list">
      <FlightlogEntry
        v-for="flight in filteredFlights"
        :key="flight.id"
        :flight="flight"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, computed } from 'vue';
import { differenceInMinutes } from 'date-fns';
import { Flight } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import FlightlogEntry from '@/components/flights/FlightlogEntry.vue';

const REFRESH_INTERVAL = 60000; // 1 minute in milliseconds
const LIVE_THRESHOLD_MINUTES = 5;

const flights = ref<Array<Flight>>([]);
const loading = ref(false);
const militaryFilter = ref(false);
const liveFilter = ref(false);

const apiService = getFlightApiService();

let intervalId: ReturnType<typeof setInterval> | null = null;

const isFlightLive = (flight: Flight): boolean => {
  const lastContact = new Date(flight.lstCntct);
  const minutesAgo = differenceInMinutes(new Date(), lastContact);
  // Live if last contact was within threshold and not in the future
  return minutesAgo >= 0 && minutesAgo < LIVE_THRESHOLD_MINUTES;
};

const filteredFlights = computed(() => {
  if (liveFilter.value) {
    // Show only live flights when live filter is on
    return flights.value.filter(f => isFlightLive(f));
  }
  // Show only past flights when live filter is off
  return flights.value.filter(f => !isFlightLive(f));
});

const loadData = async () => {
  loading.value = true;
  try {
    const limit = 100;
    // When military filter is active, pass true. Otherwise pass undefined to get all flights.
    const mil = militaryFilter.value ? true : undefined;
    flights.value = await apiService.getFlights(limit, mil);
  } catch (err) {
    console.error('Could not fetch flights:', err);
    flights.value = [];
  } finally {
    loading.value = false;
  }
};

const toggleMilitaryFilter = () => {
  militaryFilter.value = !militaryFilter.value;
  flights.value = [];
  loadData();
};

const toggleLiveFilter = () => {
  liveFilter.value = !liveFilter.value;
};

onMounted(() => {
  loadData();

  intervalId = setInterval(() => {
    loadData();
  }, REFRESH_INTERVAL);
});

onBeforeUnmount(() => {
  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
});
</script>

<style scoped>
.flight-log-view {
  max-width: 900px;
  margin: 0 auto;
  padding: 16px;
}

.filter-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
}

.filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid #e0e0e0;
  background: #fafafa;
  border-radius: 20px;
  cursor: pointer;
  font-size: 0.8rem;
  font-weight: 500;
  color: #666;
  transition: all 0.15s ease;
}

.filter-chip:hover {
  background: #f0f0f0;
  border-color: #ccc;
}

.filter-chip.active {
  background: #333;
  border-color: #333;
  color: #fff;
}

.filter-chip.active:hover {
  background: #444;
  border-color: #444;
}

.filter-chip i {
  font-size: 0.75rem;
}

.refresh-indicator {
  margin-left: auto;
  display: flex;
  align-items: center;
  color: #999;
}

.refresh-indicator .spinner-border-sm {
  width: 14px;
  height: 14px;
  border-width: 2px;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 64px 16px;
  color: #999;
}

.loading-state p {
  margin-top: 16px;
  font-size: 0.95rem;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 64px 16px;
  color: #ccc;
}

.empty-state i {
  font-size: 2.5rem;
  margin-bottom: 12px;
}

.empty-state p {
  font-size: 0.95rem;
  color: #999;
}

.flight-list {
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
  overflow: hidden;
}
</style>
