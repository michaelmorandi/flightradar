<template>
  <div
    :class="[{ 'offcanvas-start': !isMobile, 'offcanvas-bottom': isMobile }, 'offcanvas']"
    data-bs-backdrop="false"
    tabindex="-1"
    id="sidebar"
    aria-labelledby="offcanvasExampleLabel"
    ref="sidebar"
  >
    <div class="offcanvas-header">
      <button type="button" class="btn-close" data-bs-dismiss="offcanvas" aria-label="Close"></button>
    </div>
    <div class="offcanvas-body">
      <FlightDetail :flight-id="selectedFlight" />
    </div>
  </div>

  <MapComposition
    v-bind:apikey="apiKey"
    lat="46.8"
    lng="8.15"
    :aerialOverview="true"
    :peridicallyRefresh="true"
    @on-marker-clicked="showFlightDetails($event)"
    ref="mapComponent"
  />
</template>

<script setup lang="ts">
import FlightDetail from '@/components/flights/FlightDetail.vue';
import MapComposition from '@/components/map/MapComposition.vue';
import { config } from '@/config';
import { Offcanvas } from 'bootstrap';
import { ref, onMounted } from 'vue';

//Reference to the sidebar HTML div
const sidebar = ref();

const mapComponent = ref();

const apiKey = config.hereApiKey;

const isMobile = ref();

const selectedFlight = ref<string>();

onMounted(() => {
  isMobile.value = window.innerWidth < 768;

  const sidebarElement = document.getElementById('sidebar');
  if (sidebarElement) {
    sidebarElement.addEventListener('hide.bs.offcanvas', () => {
      if (mapComponent.value) {
        mapComponent.value.unselectFlight();
      }
    });
  }
});

const toggleSidebar = () => {
  const offcanvas = new Offcanvas(sidebar.value);
  offcanvas.toggle();
};

const showFlightDetails = (flightId: string) => {
  selectedFlight.value = flightId;
  toggleSidebar();
};
</script>
