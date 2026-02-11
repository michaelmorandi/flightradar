<template>
  <!-- Dashboard routes use router-view -->
  <template v-if="isDashboardRoute">
    <router-view />
  </template>

  <!-- Main app with nav for live/log views -->
  <template v-else>
    <nav class="app-nav">
      <div class="nav-tabs">
        <a
          href="#"
          class="nav-tab"
          :class="{ active: viewStore.currentView === 'live' }"
          @click.prevent="viewStore.showLive()"
        >
          <i class="bi bi-radar"></i>
          <span>Live</span>
        </a>
        <a
          href="#"
          class="nav-tab"
          :class="{ active: viewStore.currentView === 'log' }"
          @click.prevent="viewStore.showLog()"
        >
          <i class="bi bi-card-list"></i>
          <span>Flight history</span>
        </a>
        <button
          class="mil-toggle"
          :class="{ active: militaryStore.militaryOnly }"
          @click="militaryStore.toggleFilter()"
        >
          <i class="bi bi-shield-fill"></i>
          <span>Military</span>
          <span v-if="militaryCount > 0" class="mil-count">{{ militaryCount }}</span>
        </button>
      </div>
    </nav>
    <div v-show="viewStore.currentView === 'live'">
      <LiveRadar />
    </div>
    <div v-show="viewStore.currentView === 'log'">
      <FlightLog />
    </div>
  </template>
</template>

<script>
import { defineComponent, watch, computed } from 'vue';
import { useRoute } from 'vue-router';
import { Tooltip } from 'bootstrap';
import { useViewStore } from './stores/viewStore';
import { useAircraftStore } from './stores/aircraft';
import { useMilitaryStore } from './stores/militaryStore';
import LiveRadar from './views/LiveRadar.vue';
import FlightLog from './views/FlightLog.vue';

export default defineComponent({
  name: 'App',

  components: {
    LiveRadar,
    FlightLog,
  },

  setup() {
    const route = useRoute();
    const viewStore = useViewStore();
    const aircraftStore = useAircraftStore();
    const militaryStore = useMilitaryStore();

    const liveCount = computed(() => aircraftStore.activeAircraftList.length);

    const militaryCount = computed(() =>
      aircraftStore.activeAircraftList.filter(ac => militaryStore.isMilitary(ac.icao24)).length
    );

    // Check if current route is a dashboard route
    const isDashboardRoute = computed(() => {
      return route.path.startsWith('/dashboard');
    });

    // Update browser tab title with live aircraft count
    watch(liveCount, (count) => {
      document.title = count > 0 ? `Flightradar (${count})` : 'Flightradar';
    }, { immediate: true });

    return { viewStore, militaryStore, militaryCount, isDashboardRoute };
  },

  mounted() {
    // Initialize Bootstrap tooltips
    new Tooltip(document.body, {
      selector: "[data-bs-toggle='tooltip']",
    });
  },
});
</script>

<style>
#app {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-align: center;
  color: #2c3e50;
}

.app-nav {
  position: fixed;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 500;
  background: rgba(255, 255, 255, 0.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-radius: 24px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  padding: 0;
}

.nav-tabs {
  display: inline-flex;
  align-items: center;
  gap: 0;
  padding: 6px 8px;
  border-bottom: none;
}

.nav-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 18px;
  text-decoration: none;
  font-size: 0.82rem;
  font-weight: 500;
  color: #555;
  border-radius: 18px;
  transition: all 0.15s ease;
}

.nav-tab:hover {
  color: #222;
  background: rgba(0, 0, 0, 0.06);
}

.nav-tab.active {
  color: #111;
  background: rgba(0, 0, 0, 0.09);
}

.nav-tab i {
  font-size: 0.85rem;
}

.mil-toggle {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  margin-left: 4px;
  border: 1px solid rgba(0, 0, 0, 0.12);
  background: transparent;
  border-radius: 16px;
  cursor: pointer;
  font-size: 0.78rem;
  font-weight: 500;
  color: #555;
  transition: all 0.15s ease;
}

.mil-toggle:hover {
  background: rgba(0, 0, 0, 0.06);
  border-color: rgba(0, 0, 0, 0.2);
}

.mil-toggle.active {
  background: rgba(0, 0, 0, 0.75);
  border-color: transparent;
  color: #fff;
}

.mil-toggle.active:hover {
  background: rgba(0, 0, 0, 0.82);
}

.mil-toggle i {
  font-size: 0.72rem;
}

.mil-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 9px;
  font-size: 0.68rem;
  font-weight: 600;
  background: rgba(0, 0, 0, 0.12);
  line-height: 1;
}

.mil-toggle.active .mil-count {
  background: rgba(255, 255, 255, 0.2);
}
</style>
