<template>
  <div class="live-aircraft-list" :class="{ 'list-collapsed': isListCollapsed }">
    <div class="search-bar">
      <i class="bi bi-search search-icon"></i>
      <input
        type="text"
        class="search-input"
        placeholder="Search..."
        v-model="searchQuery"
      />
      <button
        class="collapse-btn"
        @click="toggleListCollapse"
        :title="isListCollapsed ? 'Expand list' : 'Collapse list'"
      >
        <i :class="isListCollapsed ? 'bi bi-chevron-down' : 'bi bi-chevron-up'"></i>
      </button>
    </div>

    <div class="list-content" v-show="!isListCollapsed">
      <div
        v-for="aircraft in filteredAircraft"
        :key="aircraft.flightId"
        class="aircraft-item"
        :class="{ 'selected': aircraft.flightId === selectedFlightId }"
        @click="selectAircraft(aircraft.flightId)"
      >
        <div class="aircraft-silhouette">
          <img
            :src="getSilhouetteSrc(aircraft.icaoType)"
            :alt="aircraft.icaoType || 'Aircraft'"
            @error="onImageError"
          />
        </div>
        <div class="aircraft-info">
          <div class="callsign">
            {{ aircraft.callsign || aircraft.icao24 }}
          </div>
          <div class="operator" v-if="aircraft.operator">
            {{ aircraft.operator }}
          </div>
        </div>
      </div>

      <div v-if="filteredAircraft.length === 0" class="empty-state">
        <i class="bi bi-search"></i>
        <p>No aircraft found</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { useAircraftStore } from '@/stores/aircraft';
import { useMilitaryStore } from '@/stores/militaryStore';
import { getAircraftDetailsService } from '@/services/aircraftDetailsService';
import { getFlightApiService } from '@/services/flightApiService';
import { silhouetteUrl } from '@/components/aircraftIcon';

const aircraftStore = useAircraftStore();
const militaryStore = useMilitaryStore();
const aircraftDetailsService = getAircraftDetailsService();
const flightApiService = getFlightApiService();

const emit = defineEmits(['aircraftSelected']);

const searchQuery = ref('');
const isListCollapsed = ref(false);

const toggleListCollapse = () => {
  isListCollapsed.value = !isListCollapsed.value;
};

const getSilhouetteSrc = (icaoType?: string): string => {
  if (icaoType) {
    return silhouetteUrl(icaoType);
  }
  return '/silhouettes/generic.png';
};

const onImageError = (event: Event) => {
  const img = event.target as HTMLImageElement;
  if (!img.src.endsWith('/silhouettes/generic.png')) {
    img.src = '/silhouettes/generic.png';
  }
};

// Use pre-computed list from store for optimal reactivity and performance
const activeAircraft = computed(() => aircraftStore.activeAircraftList);

// Track which flight IDs we've already requested to avoid duplicate requests
const requestedFlightIds = new Set<string>();

// Fetch flight details to get icao24, then fetch aircraft details
const fetchFlightAndAircraftDetails = async (flightId: string) => {
  try {
    const flightData = await flightApiService.getFlight(flightId);
    if (flightData?.icao24) {
      // Update the aircraft's icao24 in the store
      aircraftStore.updateAircraftIcao24(flightId, flightData.icao24);
      // Now fetch aircraft details using the icao24
      await aircraftDetailsService.fetchAircraftDetails(flightData.icao24);
    }
  } catch (error) {
    console.error(`Error fetching details for flight ${flightId}:`, error);
  }
};

// Watch for aircraft changes and prefetch details
watch(
  activeAircraft,
  (aircraft) => {
    // Get flight IDs that need details fetched
    const flightIdsNeedingDetails = aircraft
      .filter(ac => ac.flightId && !requestedFlightIds.has(ac.flightId))
      .map(ac => ac.flightId);

    if (flightIdsNeedingDetails.length > 0) {
      console.log('[LiveAircraftList] Fetching details for flights:', flightIdsNeedingDetails.length);
      // Mark as requested before fetching
      flightIdsNeedingDetails.forEach(id => requestedFlightIds.add(id));
      // Fetch in parallel with limited concurrency
      const batchSize = 5;
      for (let i = 0; i < flightIdsNeedingDetails.length; i += batchSize) {
        const batch = flightIdsNeedingDetails.slice(i, i + batchSize);
        Promise.all(batch.map(id => fetchFlightAndAircraftDetails(id)));
      }
    }
  },
  { immediate: true }
);

const filteredAircraft = computed(() => {
  let list = activeAircraft.value;

  if (militaryStore.militaryOnly) {
    list = list.filter(ac => militaryStore.isMilitary(ac.icao24));
  }

  if (searchQuery.value.trim()) {
    const query = searchQuery.value.toLowerCase();
    list = list.filter((aircraft) => {
      const callsign = aircraft.callsign?.toLowerCase() || '';
      const icao = aircraft.icao24?.toLowerCase() || '';
      return callsign.includes(query) || icao.includes(query);
    });
  }

  return list;
});

const selectedFlightId = computed(() => aircraftStore.selectedFlightId);

const selectAircraft = (flightId: string) => {
  emit('aircraftSelected', flightId);
};
</script>

<style scoped>
.live-aircraft-list {
  position: fixed;
  left: 10px;
  top: 10px;
  bottom: 10px;
  width: 240px;
  background: rgba(255, 255, 255, 0.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  z-index: 400;
  transition: bottom 0.3s ease;
  overflow: hidden;
}

.live-aircraft-list.list-collapsed {
  bottom: auto;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
}

.search-icon {
  color: #999;
  font-size: 0.8rem;
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 14px;
  font-size: 0.8rem;
  outline: none;
  background: rgba(0, 0, 0, 0.03);
  transition: border-color 0.2s;
}

.search-input:focus {
  border-color: #0d6efd;
  background: #fff;
}

.collapse-btn {
  background: none;
  border: none;
  padding: 4px 6px;
  cursor: pointer;
  color: #999;
  font-size: 0.75rem;
  border-radius: 8px;
  flex-shrink: 0;
  transition: all 0.15s ease;
}

.collapse-btn:hover {
  background: rgba(0, 0, 0, 0.06);
  color: #333;
}

.list-content {
  flex: 1;
  overflow-y: auto;
  border-top: 1px solid rgba(0, 0, 0, 0.06);
}

.aircraft-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.aircraft-item:hover {
  background: rgba(0, 0, 0, 0.04);
}

.aircraft-item.selected {
  background: rgba(13, 110, 253, 0.08);
  border-left: 3px solid #0d6efd;
}

.aircraft-silhouette {
  width: 40px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 10px;
  flex-shrink: 0;
}

.aircraft-silhouette img {
  max-width: 40px;
  max-height: 28px;
  object-fit: contain;
}

.aircraft-info {
  flex: 1;
  min-width: 0;
  text-align: left;
}

.callsign {
  font-size: 0.75rem;
  font-weight: 600;
  color: #333;
  margin-bottom: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.operator {
  font-size: 0.75rem;
  color: #666;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 16px;
  color: #ccc;
}

.empty-state i {
  font-size: 2rem;
  margin-bottom: 8px;
}

.empty-state p {
  font-size: 0.875rem;
  color: #999;
  margin: 0;
}

/* Mobile styles */
@media (max-width: 767px) {
  .live-aircraft-list {
    width: calc(100% - 20px);
    left: 10px;
    right: 10px;
    top: auto;
    bottom: 10px;
    max-height: 60vh;
  }

  .list-content {
    max-height: calc(60vh - 60px);
  }
}
</style>
