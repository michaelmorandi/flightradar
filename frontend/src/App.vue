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

    const liveCount = computed(() => aircraftStore.activeAircraftList.length);

    // Check if current route is a dashboard route
    const isDashboardRoute = computed(() => {
      return route.path.startsWith('/dashboard');
    });

    // Update browser tab title with live aircraft count
    watch(liveCount, (count) => {
      document.title = count > 0 ? `Flightradar (${count})` : 'Flightradar';
    }, { immediate: true });

    return { viewStore, isDashboardRoute };
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
  background: #fff;
  border-bottom: 1px solid #eee;
  padding: 0;
  position: sticky;
  top: 0;
  z-index: 100;
}

.nav-tabs {
  display: inline-flex;
  gap: 0;
  padding: 8px 12px;
}

.nav-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 20px;
  text-decoration: none;
  font-size: 0.85rem;
  font-weight: 500;
  color: #666;
  border-radius: 6px;
  transition: all 0.15s ease;
}

.nav-tab:hover {
  color: #333;
  background: #f5f5f5;
}

.nav-tab.active {
  color: #111;
  background: #f0f0f0;
}

.nav-tab i {
  font-size: 0.9rem;
}
</style>
