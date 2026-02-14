<template>
  <div class="airlines-view">
    <div v-if="loading" class="loading-state">
      <div class="spinner-border" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
      <p>Loading airlines...</p>
    </div>

    <div v-else-if="airlines.length === 0" class="empty-state">
      <i class="bi bi-building"></i>
      <p v-if="viewStore.searchQuery">No airlines found matching "{{ viewStore.searchQuery }}"</p>
      <p v-else>No airlines observed yet</p>
    </div>

    <div v-else class="airlines-list">
      <div class="airlines-count">{{ airlines.length }} airlines</div>
      <div
        v-for="airline in airlines"
        :key="airline.icaoCode"
        class="airline-row"
        @click="navigateToFlights(airline.icaoCode)"
      >
        <div class="airline-code">{{ airline.icaoCode }}</div>
        <div class="airline-info">
          <div class="airline-name">{{ airline.name }}</div>
          <div class="airline-meta">
            <span v-if="airline.country">{{ airline.country }}</span>
            <span v-if="airline.callsign" class="airline-callsign">{{ airline.callsign }}</span>
          </div>
        </div>
        <div class="airline-stats">
          <div class="stat-flights">{{ airline.flightCount }} flights</div>
          <div class="stat-aircraft">{{ airline.aircraftCount }} aircraft</div>
        </div>
        <div class="airline-arrow">
          <i class="bi bi-chevron-right"></i>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import type { AirlineWithStats } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import { useViewStore } from '@/stores/viewStore';

const apiService = getFlightApiService();
const viewStore = useViewStore();

const airlines = ref<AirlineWithStats[]>([]);
const loading = ref(false);
let searchTimeout: ReturnType<typeof setTimeout> | null = null;

const loadAirlines = async (query?: string) => {
  loading.value = true;
  try {
    const response = await apiService.getAirlines(query);
    airlines.value = response.airlines;
  } catch (err) {
    console.error('Could not fetch airlines:', err);
    airlines.value = [];
  } finally {
    loading.value = false;
  }
};

watch(() => viewStore.searchQuery, (query) => {
  if (searchTimeout) clearTimeout(searchTimeout);
  searchTimeout = setTimeout(() => {
    loadAirlines(query || undefined);
  }, 300);
});

const navigateToFlights = (icaoCode: string) => {
  // Switch to flight history view with airline filter
  viewStore.showLog();

  // Use a custom event to communicate with FlightLog
  window.dispatchEvent(new CustomEvent('filter-airline', { detail: icaoCode }));
};

onMounted(() => {
  loadAirlines();
});
</script>

<style scoped>
.airlines-view {
  max-width: 900px;
  margin: 0 auto;
  padding: 56px 16px 16px;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
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

.airlines-list {
  background: #fff;
  border-radius: 10px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
  overflow: hidden;
}

.airlines-count {
  padding: 10px 16px;
  font-size: 0.78rem;
  color: #6c757d;
  border-bottom: 1px solid #f0f0f0;
  text-align: left;
}

.airline-row {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid #f0f0f0;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.airline-row:last-child {
  border-bottom: none;
}

.airline-row:hover {
  background-color: #f8f9fa;
}

.airline-code {
  flex-shrink: 0;
  width: 52px;
  font-family: 'SF Mono', SFMono-Regular, ui-monospace, monospace;
  font-size: 0.82rem;
  font-weight: 600;
  color: #495057;
  background: #e9ecef;
  padding: 4px 8px;
  border-radius: 4px;
  text-align: center;
}

.airline-info {
  flex: 1;
  min-width: 0;
  margin-left: 14px;
  text-align: left;
}

.airline-name {
  font-weight: 500;
  font-size: 0.9rem;
  color: #212529;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.airline-meta {
  display: flex;
  gap: 8px;
  font-size: 0.75rem;
  color: #6c757d;
  margin-top: 1px;
}

.airline-callsign {
  font-style: italic;
}

.airline-stats {
  flex-shrink: 0;
  text-align: right;
  margin-right: 8px;
}

.stat-flights {
  font-size: 0.82rem;
  font-weight: 500;
  color: #212529;
}

.stat-aircraft {
  font-size: 0.72rem;
  color: #6c757d;
}

.airline-arrow {
  flex-shrink: 0;
  color: #adb5bd;
  font-size: 0.85rem;
}

/* Mobile responsive */
@media (max-width: 640px) {
  .airlines-view {
    padding: 8px;
    padding-bottom: 80px; /* Space for bottom navbar */
  }

  .airline-row {
    padding: 10px 12px;
  }

  .airline-code {
    width: 44px;
    font-size: 0.75rem;
    padding: 3px 6px;
  }

  .airline-info {
    margin-left: 10px;
  }

  .airline-name {
    font-size: 0.82rem;
  }

  .airline-meta {
    font-size: 0.7rem;
  }

  .airline-stats {
    margin-right: 4px;
  }

  .stat-flights {
    font-size: 0.75rem;
  }

  .stat-aircraft {
    font-size: 0.68rem;
  }
}
</style>
