<template>
  <div class="flight-log-view">
    <div class="filter-bar">
      <div class="filter-row" v-if="hasActiveFilters || totalFlights > 0">
        <div class="active-filters" v-if="hasActiveFilters">
          <span
            v-if="filters.icao24"
            class="filter-chip active"
            @click="removeFilter('icao24')"
          >
            <i class="bi bi-airplane"></i>
            {{ filters.icao24.toUpperCase() }}
            <i class="bi bi-x"></i>
          </span>
          <span
            v-if="filters.airline"
            class="filter-chip active"
            @click="removeFilter('airline')"
          >
            <i class="bi bi-building"></i>
            {{ airlineFilterLabel }}
            <i class="bi bi-x"></i>
          </span>
        </div>

        <div class="pagination-info" v-if="totalFlights > 0">
          {{ totalFlights }} {{ hasActiveFilters || viewStore.searchQuery ? 'matching' : 'past' }} flights
        </div>
      </div>
    </div>

    <!-- Context Card -->
    <div v-if="contextCard" class="context-card">
      <!-- Aircraft context -->
      <template v-if="filters.icao24 && aircraftContext">
        <div class="context-icon">
          <img
            v-if="aircraftContext.icaoType"
            :src="silhouetteUrl(aircraftContext.icaoType)"
            class="context-silhouette"
            @error="($event.target as HTMLImageElement).src = '/silhouettes/generic.png'"
          />
          <i v-else class="bi bi-airplane"></i>
        </div>
        <div class="context-info">
          <div class="context-title">
            {{ aircraftContext.type || 'Unknown Aircraft' }}
            <span v-if="aircraftContext.reg" class="context-reg">{{ aircraftContext.reg }}</span>
          </div>
          <div class="context-meta">
            <span>ICAO24: {{ filters.icao24.toUpperCase() }}</span>
            <span v-if="aircraftContext.op">{{ aircraftContext.op }}</span>
          </div>
        </div>
      </template>

      <!-- Airline context -->
      <template v-else-if="filters.airline && airlineContext">
        <div class="context-icon">
          <i class="bi bi-building"></i>
        </div>
        <div class="context-info">
          <div class="context-title">{{ airlineContext.name }}</div>
          <div class="context-meta">
            <span>ICAO: {{ airlineContext.icaoCode }}</span>
            <span v-if="airlineContext.country">{{ airlineContext.country }}</span>
            <span v-if="airlineContext.flightCount">{{ airlineContext.flightCount }} flights</span>
            <span v-if="airlineContext.aircraftCount">{{ airlineContext.aircraftCount }} aircraft</span>
          </div>
        </div>
      </template>
    </div>

    <div v-if="flights.length === 0 && loading" class="loading-state">
      <div class="spinner-border" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
      <p>Loading past flights...</p>
    </div>

    <div v-else-if="flights.length === 0 && !loading" class="empty-state">
      <i class="bi bi-inbox"></i>
      <p v-if="hasActiveFilters">No flights found for this filter</p>
      <p v-else>No past flights found</p>
    </div>

    <div v-else class="flight-list">
      <FlightlogEntry
        v-for="flight in flights"
        :key="flight.id"
        :flight="flight"
        :single-aircraft="!!filters.icao24"
        @filter-airline="onFilterAirline"
        @filter-aircraft="onFilterAircraft"
      />
    </div>

    <div v-if="totalPages > 1" class="pagination-controls">
      <button
        class="pagination-btn"
        @click="goToPage(1)"
        :disabled="currentPage === 1 || loading"
      >
        <i class="bi bi-chevron-double-left"></i>
      </button>
      <button
        class="pagination-btn"
        @click="goToPage(currentPage - 1)"
        :disabled="currentPage === 1 || loading"
      >
        <i class="bi bi-chevron-left"></i>
      </button>
      <span class="page-info">
        Page {{ currentPage }} of {{ totalPages }}
      </span>
      <button
        class="pagination-btn"
        @click="goToPage(currentPage + 1)"
        :disabled="currentPage === totalPages || loading"
      >
        <i class="bi bi-chevron-right"></i>
      </button>
      <button
        class="pagination-btn"
        @click="goToPage(totalPages)"
        :disabled="currentPage === totalPages || loading"
      >
        <i class="bi bi-chevron-double-right"></i>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch, computed, reactive } from 'vue';
import type { Flight, Aircraft, FlightFilters, AirlineDetail } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import { useMilitaryStore } from '@/stores/militaryStore';
import { useViewStore } from '@/stores/viewStore';
import { silhouetteUrl } from '@/components/aircraftIcon';
import FlightlogEntry from '@/components/flights/FlightlogEntry.vue';

const PAGE_SIZE = 50;

const militaryStore = useMilitaryStore();
const viewStore = useViewStore();
const apiService = getFlightApiService();

const flights = ref<Array<Flight>>([]);
const loading = ref(false);
const currentPage = ref(1);
const totalFlights = ref(0);
const totalPages = ref(0);

const filters = reactive<FlightFilters>({});
let searchTimeout: ReturnType<typeof setTimeout> | null = null;
const aircraftContext = ref<Aircraft | null>(null);
const airlineContext = ref<AirlineDetail | null>(null);

const hasActiveFilters = computed(() => !!filters.icao24 || !!filters.airline);
const contextCard = computed(() => {
  return (filters.icao24 && aircraftContext.value) || (filters.airline && airlineContext.value);
});

const airlineFilterLabel = computed(() => {
  if (airlineContext.value) {
    return `${filters.airline} - ${airlineContext.value.name}`;
  }
  return filters.airline || '';
});

const loadData = async () => {
  loading.value = true;
  try {
    const mil = militaryStore.militaryOnly ? true : undefined;
    const activeFilters: FlightFilters = {};
    if (filters.icao24) activeFilters.icao24 = filters.icao24;
    if (filters.airline) activeFilters.airline = filters.airline;
    if (viewStore.searchQuery.trim()) activeFilters.q = viewStore.searchQuery.trim();

    const response = await apiService.getFlights(
      PAGE_SIZE, mil, currentPage.value, true, activeFilters
    );
    flights.value = response.flights;
    totalFlights.value = response.total;
    totalPages.value = response.totalPages;
    currentPage.value = response.page;
  } catch (err) {
    console.error('Could not fetch flights:', err);
    flights.value = [];
    totalFlights.value = 0;
    totalPages.value = 0;
  } finally {
    loading.value = false;
  }
};

watch(() => viewStore.searchQuery, () => {
  if (searchTimeout) clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => {
    currentPage.value = 1;
    loadData();
  }, 300);
});

const loadContextData = async () => {
  aircraftContext.value = null;
  airlineContext.value = null;

  if (filters.icao24) {
    try {
      aircraftContext.value = await apiService.getAircraft(filters.icao24);
    } catch { /* ignore */ }
  }

  if (filters.airline) {
    try {
      airlineContext.value = await apiService.getAirlineDetail(filters.airline);
    } catch { /* ignore */ }
  }
};

const applyFilter = (key: keyof FlightFilters, value: string) => {
  filters[key] = value;
  currentPage.value = 1;
  flights.value = [];
  loadContextData();
  loadData();
};

const removeFilter = (key: keyof FlightFilters) => {
  filters[key] = undefined;
  currentPage.value = 1;
  flights.value = [];
  if (key === 'icao24') aircraftContext.value = null;
  if (key === 'airline') airlineContext.value = null;
  loadData();
};

const onFilterAirline = (airlineIcao: string) => {
  filters.icao24 = undefined;
  aircraftContext.value = null;
  applyFilter('airline', airlineIcao);
};

const onFilterAircraft = (icao24: string) => {
  filters.airline = undefined;
  airlineContext.value = null;
  applyFilter('icao24', icao24);
};

watch(() => militaryStore.militaryOnly, () => {
  currentPage.value = 1;
  flights.value = [];
  loadData();
});

const goToPage = (page: number) => {
  if (page >= 1 && page <= totalPages.value && page !== currentPage.value) {
    currentPage.value = page;
    loadData();
  }
};

// Public method for external filter application (from parent components)
const setFilter = (key: keyof FlightFilters, value: string) => {
  applyFilter(key, value);
};

defineExpose({ setFilter, loadData, loading });

onMounted(() => {
  loadData();
});
</script>

<style scoped>
.flight-log-view {
  max-width: 900px;
  margin: 0 auto;
  padding: 56px 16px 16px;
}

.filter-bar {
  margin-bottom: 16px;
}

.filter-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.active-filters {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
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

.filter-chip:hover:not(:disabled) {
  background: #f0f0f0;
  border-color: #ccc;
}

.filter-chip:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.filter-chip.active {
  background: #333;
  border-color: #333;
  color: #fff;
}

.filter-chip.active:hover:not(:disabled) {
  background: #555;
  border-color: #555;
}

.filter-chip.active .bi-x {
  margin-left: 2px;
  font-size: 0.85rem;
}

.filter-chip i {
  font-size: 0.75rem;
}

.pagination-info {
  margin-left: auto;
  font-size: 0.8rem;
  color: #666;
  padding: 6px 12px;
  white-space: nowrap;
}

.context-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  margin-bottom: 16px;
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 10px;
}

.context-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 80px;
  height: 44px;
  background: #fff;
  border-radius: 8px;
  border: 1px solid #dee2e6;
}

.context-silhouette {
  max-width: 72px;
  max-height: 36px;
  object-fit: contain;
}

.context-icon i {
  font-size: 1.1rem;
  color: #495057;
}

.context-info {
  flex: 1;
  min-width: 0;
}

.context-title {
  font-weight: 600;
  font-size: 0.92rem;
  color: #212529;
  text-align: left;
}

.context-reg {
  font-weight: 400;
  color: #6c757d;
  margin-left: 8px;
}

.context-meta {
  display: flex;
  gap: 12px;
  font-size: 0.78rem;
  color: #6c757d;
  margin-top: 2px;
  flex-wrap: wrap;
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

.pagination-controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: 16px;
  padding: 16px;
}

.pagination-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: 1px solid #e0e0e0;
  background: #fff;
  border-radius: 6px;
  cursor: pointer;
  color: #495057;
  transition: all 0.15s ease;
}

.pagination-btn:hover:not(:disabled) {
  background: #f8f9fa;
  border-color: #adb5bd;
}

.pagination-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.page-info {
  margin: 0 8px;
  font-size: 0.9rem;
  color: #495057;
  font-weight: 500;
}

/* Mobile responsive */
@media (max-width: 640px) {
  .flight-log-view {
    padding: 8px;
    padding-bottom: 80px; /* Space for bottom navbar */
  }

  .pagination-info {
    padding: 4px 8px;
    font-size: 0.75rem;
  }

  .context-card {
    padding: 10px 12px;
    gap: 10px;
  }

  .context-icon {
    width: 64px;
    height: 36px;
  }

  .context-title {
    font-size: 0.85rem;
  }

  .context-meta {
    font-size: 0.72rem;
    gap: 8px;
  }
}
</style>
