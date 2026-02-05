<template>
  <span
    class="airport-code"
    @mouseenter="onHover"
    @mouseleave="showPopover = false"
  >
    {{ iata }}
    <span v-if="showPopover && airportInfo" class="airport-popover">
      <strong>{{ airportInfo.airport }}</strong>
      <span class="airport-country">{{ airportInfo.country_code }}</span>
    </span>
    <span v-if="showPopover && loading" class="airport-popover">
      Loading...
    </span>
  </span>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import type { AirportInfo } from '@/model/backendModel';
import { getFlightApiService } from '@/services/flightApiService';

const props = defineProps<{
  iata: string;
}>();

const apiService = getFlightApiService();

const airportInfo = ref<AirportInfo | null>(null);
const loading = ref(false);
const showPopover = ref(false);
const fetched = ref(false);

const onHover = async () => {
  showPopover.value = true;

  if (fetched.value) return;

  loading.value = true;
  fetched.value = true;

  try {
    airportInfo.value = await apiService.getAirportInfo(props.iata);
  } catch {
    airportInfo.value = null;
  } finally {
    loading.value = false;
  }
};
</script>

<style scoped>
.airport-code {
  position: relative;
  cursor: help;
  border-bottom: 1px dotted #888;
}

.airport-popover {
  position: absolute;
  z-index: 1000;
  bottom: 125%;
  left: 50%;
  transform: translateX(-50%);
  background-color: #333;
  color: #fff;
  padding: 6px 10px;
  border-radius: 4px;
  font-size: 0.8em;
  white-space: nowrap;
  pointer-events: none;
}

.airport-popover::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 50%;
  margin-left: -5px;
  border-width: 5px;
  border-style: solid;
  border-color: #333 transparent transparent transparent;
}

.airport-country {
  margin-left: 6px;
  opacity: 0.7;
  font-size: 0.9em;
}
</style>
