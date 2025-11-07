<template>
  <div :class="[{ 'offcanvas-start': !isMobile, 'offcanvas-bottom': isMobile }, 'offcanvas']" data-bs-backdrop="false" tabindex="-1" id="sidebar" ref="sidebar">
    <div class="offcanvas-header">
      <button type="button" class="btn-close" data-bs-dismiss="offcanvas" aria-label="Close"></button>
    </div>
    <div class="offcanvas-body">
      <FlightDetail :flightId="flightId" />
    </div>
  </div>
  <MapComposition
    v-bind:apikey="apiKey"
    lat="46.8"
    lng="7.15"
    :aerialOverview="false"
    :highlightedFlightId="flightId"
    :peridicallyRefresh="false"
    ref="mapComponent"
  />
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useRoute } from 'vue-router';
import { config } from '@/config';
import { Offcanvas } from 'bootstrap';
import MapComposition from '@/components/map/MapComposition.vue';
import FlightDetail from '@/components/flights/FlightDetail.vue';

const apiKey = config.hereApiKey;

//Reference to the sidebar HTML div
const sidebar = ref();

const flightId = ref();

const isMobile = ref();

onMounted(() => {
  isMobile.value = window.innerWidth < 768;
  const route = useRoute();
  flightId.value = route.params.flightId;

  const offcanvas = new Offcanvas(sidebar.value);
  offcanvas.toggle();
});
</script>
