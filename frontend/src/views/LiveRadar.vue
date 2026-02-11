<template>
  <div class="live-radar-container">
    <LiveAircraftList @aircraft-selected="showFlightDetails" />

    <div
      :class="[{ 'offcanvas-start': !isMobile, 'offcanvas-bottom': isMobile }, 'offcanvas', 'details-offcanvas']"
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
  </div>
</template>

<script setup lang="ts">
import FlightDetail from '@/components/flights/FlightDetail.vue';
import MapComposition from '@/components/map/MapComposition.vue';
import LiveAircraftList from '@/components/map/LiveAircraftList.vue';
import { config } from '@/config';
import { Offcanvas } from 'bootstrap';
import { ref, onMounted, watch } from 'vue';
import { useAircraftStore } from '@/stores/aircraft';
import { useMilitaryStore } from '@/stores/militaryStore';

//Reference to the sidebar HTML div
const sidebar = ref();

const mapComponent = ref();

const apiKey = config.hereApiKey;

const isMobile = ref();

const selectedFlight = ref<string>();

const aircraftStore = useAircraftStore();
const militaryStore = useMilitaryStore();

onMounted(() => {
  isMobile.value = window.innerWidth < 768;

  const sidebarElement = document.getElementById('sidebar');
  if (sidebarElement) {
    sidebarElement.addEventListener('hide.bs.offcanvas', () => {
      if (mapComponent.value) {
        mapComponent.value.unselectFlight();
      }
      aircraftStore.clearSelection();
    });
  }
});

// When military filter is activated, hide detail pane if selected aircraft is not military
watch(() => militaryStore.militaryOnly, (active) => {
  if (active && selectedFlight.value) {
    const ac = aircraftStore.getAircraftById(selectedFlight.value);
    if (ac && !militaryStore.isMilitary(ac.icao24)) {
      // Programmatically hide the offcanvas — this triggers the hide.bs.offcanvas
      // event which already handles unselectFlight + clearSelection
      const sidebarEl = document.getElementById('sidebar');
      if (sidebarEl) {
        const instance = Offcanvas.getInstance(sidebarEl);
        if (instance) {
          instance.hide();
        }
      }
    }
  }
});

const toggleSidebar = () => {
  const offcanvas = new Offcanvas(sidebar.value);
  offcanvas.toggle();
};

const showFlightDetails = (flightId: string) => {
  selectedFlight.value = flightId;
  aircraftStore.selectFlight(flightId);
  toggleSidebar();
};
</script>

<style scoped>
.live-radar-container {
  position: relative;
  width: 100%;
  height: 100%;
}

/* Ensure the details offcanvas appears above the aircraft list */
.details-offcanvas {
  z-index: 1050;
}

/* Desktop: offcanvas should be positioned from the left */
@media (min-width: 768px) {
  .details-offcanvas.offcanvas-start {
    width: 400px;
  }
}
</style>
