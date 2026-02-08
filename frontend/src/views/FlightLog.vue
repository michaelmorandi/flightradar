<template>
  <div class="flight-log-view">
    <div class="filter-bar">
      <button
        type="button"
        class="filter-chip refresh-button"
        @click="loadData"
        :disabled="loading"
      >
        <i class="bi bi-arrow-clockwise" :class="{ 'spinning': loading }"></i>
        Refresh
      </button>
      <div class="pagination-info" v-if="totalFlights > 0">
        {{ totalFlights }} past flights
      </div>
    </div>

    <div v-if="flights.length === 0 && loading" class="loading-state">
      <div class="spinner-border" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
      <p>Loading past flights...</p>
    </div>

    <div v-else-if="flights.length === 0 && !loading" class="empty-state">
      <i class="bi bi-inbox"></i>
      <p>No past flights found</p>
    </div>

    <div v-else class="flight-list">
      <FlightlogEntry
        v-for="flight in flights"
        :key="flight.id"
        :flight="flight"
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
import { onMounted, ref, watch } from 'vue';
import { Flight } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import { useMilitaryStore } from '@/stores/militaryStore';
import FlightlogEntry from '@/components/flights/FlightlogEntry.vue';

const PAGE_SIZE = 50;

const militaryStore = useMilitaryStore();

const flights = ref<Array<Flight>>([]);
const loading = ref(false);
const currentPage = ref(1);
const totalFlights = ref(0);
const totalPages = ref(0);

const apiService = getFlightApiService();

const loadData = async () => {
  loading.value = true;
  try {
    // Pass undefined when filter is off to get all flights (mil and non-mil)
    // excludeLive=true filters out flights with last contact within 5 minutes
    const mil = militaryStore.militaryOnly ? true : undefined;
    const response = await apiService.getFlights(PAGE_SIZE, mil, currentPage.value, true);
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

onMounted(() => {
  loadData();
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
  background: #444;
  border-color: #444;
}

.filter-chip i {
  font-size: 0.75rem;
}

.filter-chip.refresh-button i.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.pagination-info {
  margin-left: auto;
  font-size: 0.8rem;
  color: #666;
  padding: 6px 12px;
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
</style>
