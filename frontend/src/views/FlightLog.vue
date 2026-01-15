<template>
  <div class="about">
    <div v-if="flights.length == 0 && loading" class="spinner-border" role="status">
      <span class="visually-hidden">Loading...</span>
    </div>
    Filter:
    <button type="button" class="btn btn-sm" @click="toggleBoolean" :class="{ 'btn-outline-secondary': !militaryFilter, 'btn-outline-dark': militaryFilter }">
      Military
    </button>
    <FlightlogEntry v-for="flight in flights" :key="flight.id" v-bind:flight="flight" class="mx-auto" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue';
import { Flight } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';
import FlightlogEntry from '@/components/flights/FlightlogEntry.vue';

const flights = ref<Array<Flight>>([]);
const loading = ref(false);

const apiService = getFlightApiService();

const intervalId = ref<ReturnType<typeof setTimeout>>();

const militaryFilter = ref<boolean>(false);

const loadData = async () => {
  loading.value = true;
  try {
    const limit = 100;
    const filter = militaryFilter.value ? 'mil' : undefined;
    flights.value = await apiService.getFlights(limit, filter);
  } catch (err) {
    console.error('Could not fetch recent flights:', err);
    flights.value = [];
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  loadData();

  intervalId.value = setInterval(() => {
    loadData();
  }, 5000);
});

onBeforeUnmount(() => {
  if (intervalId.value) clearInterval(intervalId.value);
});

const toggleBoolean = () => {
  militaryFilter.value = !militaryFilter.value;

  flights.value = [];
  loadData();
};
</script>
