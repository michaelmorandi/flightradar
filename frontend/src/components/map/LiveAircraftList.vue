<template>
  <div class="live-aircraft-list" :class="{ 'collapsed': isMobileCollapsed, 'sidebar-collapsed': isSidebarCollapsed }">
    <!-- Collapse toggle button (visible when sidebar is collapsed) -->
    <button
      v-if="isSidebarCollapsed"
      class="sidebar-expand-btn"
      @click="toggleSidebarCollapse"
      title="Show aircraft list"
    >
      <i class="bi bi-chevron-right"></i>
    </button>

    <!-- Main sidebar content -->
    <template v-if="!isSidebarCollapsed">
      <div class="list-header">
        <h3 class="list-title">
          <i class="bi bi-radar"></i>
          Live Aircraft
        </h3>
        <button
          class="mobile-toggle"
          @click="toggleMobileCollapse"
          v-if="isMobile"
        >
          <i :class="isMobileCollapsed ? 'bi bi-chevron-up' : 'bi bi-chevron-down'"></i>
        </button>
        <span class="aircraft-count">{{ activeAircraft.length }}</span>
        <button
          class="sidebar-collapse-btn"
          @click="toggleSidebarCollapse"
          title="Hide aircraft list"
        >
          <i class="bi bi-chevron-left"></i>
        </button>
      </div>

      <div class="search-bar" v-if="!isMobileCollapsed">
        <input
          type="text"
          class="search-input"
          placeholder="Search by callsign or ICAO..."
          v-model="searchQuery"
        />
        <i class="bi bi-search search-icon"></i>
      </div>

      <div class="list-content" v-if="!isMobileCollapsed">
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
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import { useAircraftStore } from '@/stores/aircraft';
import { getAircraftDetailsService } from '@/services/aircraftDetailsService';
import { getFlightApiService } from '@/services/flightApiService';
import { silhouetteUrl } from '@/components/aircraftIcon';

const aircraftStore = useAircraftStore();
const aircraftDetailsService = getAircraftDetailsService();
const flightApiService = getFlightApiService();

const emit = defineEmits(['aircraftSelected']);

const searchQuery = ref('');
const isMobile = ref(false);
const isMobileCollapsed = ref(false);
const isSidebarCollapsed = ref(false);

const checkMobile = () => {
  isMobile.value = window.innerWidth < 768;
  if (isMobile.value) {
    isMobileCollapsed.value = true;
  }
};

const toggleSidebarCollapse = () => {
  isSidebarCollapsed.value = !isSidebarCollapsed.value;
};

onMounted(() => {
  checkMobile();
  window.addEventListener('resize', checkMobile);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', checkMobile);
});

const toggleMobileCollapse = () => {
  isMobileCollapsed.value = !isMobileCollapsed.value;
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

const activeAircraft = computed(() => {
  const now = Date.now();
  const staleThreshold = 15000;

  // Access the details cache - need to use getAircraftDetails for proper reactivity
  const allAircraft = Array.from(aircraftStore.aircraft.values())
    .filter(aircraft => {
      const timeSinceUpdate = now - aircraft.lastUpdate;
      return timeSinceUpdate < staleThreshold;
    })
    .map(aircraft => {
      const details = aircraftStore.getAircraftDetails(aircraft.icao24);

      return {
        flightId: aircraft.id,
        icao24: aircraft.icao24,
        callsign: aircraft.callsign || '',
        lat: aircraft.lat,
        lon: aircraft.lon,
        alt: aircraft.altitude,
        gs: aircraft.groundSpeed,
        track: aircraft.track,
        operator: aircraft.operator || details?.operator,
        aircraftType: aircraft.aircraftType || details?.type,
        icaoType: aircraft.icaoType || details?.icaoType,
        category: aircraft.category,
        firstSeen: aircraft.firstSeen,
      };
    })
    .sort((a, b) => b.firstSeen - a.firstSeen);

  return allAircraft;
});

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
  if (!searchQuery.value.trim()) {
    return activeAircraft.value;
  }

  const query = searchQuery.value.toLowerCase();
  return activeAircraft.value.filter((aircraft) => {
    const callsign = aircraft.callsign?.toLowerCase() || '';
    const icao = aircraft.icao24?.toLowerCase() || '';
    return callsign.includes(query) || icao.includes(query);
  });
});

const selectedFlightId = computed(() => aircraftStore.selectedFlightId);

const selectAircraft = (flightId: string) => {
  emit('aircraftSelected', flightId);
};
</script>

<style scoped>
.live-aircraft-list {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 240px;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  z-index: 400;
  transition: all 0.3s ease;
}

.live-aircraft-list.sidebar-collapsed {
  width: auto;
  background: transparent;
  backdrop-filter: none;
  box-shadow: none;
}

.sidebar-collapse-btn {
  background: none;
  border: none;
  padding: 4px 8px;
  cursor: pointer;
  color: #666;
  font-size: 1rem;
  margin-left: 8px;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.sidebar-collapse-btn:hover {
  background: #f0f0f0;
  color: #333;
}

.sidebar-expand-btn {
  position: absolute;
  top: 0;
  left: 0;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(10px);
  border: none;
  padding: 12px 8px;
  cursor: pointer;
  color: #666;
  font-size: 1rem;
  border-radius: 0 0 8px 0;
  box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.1);
  transition: all 0.2s;
}

.sidebar-expand-btn:hover {
  background: #fff;
  color: #333;
  padding-right: 12px;
}

.live-aircraft-list.collapsed {
  height: auto;
  bottom: auto;
  width: 100%;
  left: 0;
  right: 0;
  top: auto;
  bottom: 0;
}

.list-header {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-bottom: 1px solid #e0e0e0;
  background: #fff;
}

.list-title {
  font-size: 1rem;
  font-weight: 600;
  margin: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  color: #333;
  flex: 1;
}

.list-title i {
  color: #0d6efd;
}

.mobile-toggle {
  background: none;
  border: none;
  padding: 4px;
  cursor: pointer;
  color: #666;
  font-size: 1.2rem;
}

.aircraft-count {
  background: #0d6efd;
  color: white;
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 0.75rem;
  font-weight: 600;
  margin-left: 8px;
}

.search-bar {
  position: relative;
  padding: 8px 12px;
  border-bottom: 1px solid #e0e0e0;
  background: #fff;
}

.search-input {
  width: 100%;
  padding: 8px 12px 8px 36px;
  border: 1px solid #e0e0e0;
  border-radius: 20px;
  font-size: 0.875rem;
  outline: none;
  transition: border-color 0.2s;
}

.search-input:focus {
  border-color: #0d6efd;
}

.search-icon {
  position: absolute;
  left: 24px;
  top: 50%;
  transform: translateY(-50%);
  color: #999;
  pointer-events: none;
}

.list-content {
  flex: 1;
  overflow-y: auto;
  background: #fafafa;
}

.aircraft-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #e0e0e0;
  cursor: pointer;
  transition: background-color 0.15s ease;
  background: #fff;
}

.aircraft-item:hover {
  background: #f8f9fa;
}

.aircraft-item.selected {
  background: #e7f3ff;
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
    width: 100%;
    left: 0;
    right: 0;
    top: auto;
    bottom: 0;
    max-height: 60vh;
  }

  .live-aircraft-list.collapsed {
    max-height: 60px;
  }

  .list-content {
    max-height: calc(60vh - 120px);
  }
}
</style>
