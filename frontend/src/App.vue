<template>
  <!-- Dashboard routes use router-view -->
  <template v-if="isDashboardRoute">
    <router-view />
  </template>

  <!-- Main app with nav for live/log views -->
  <template v-else>
    <nav class="app-nav">
      <div class="nav-tabs">
        <div class="view-dropdown" ref="dropdownRef">
          <button class="view-toggle" @click="dropdownOpen = !dropdownOpen">
            <i :class="viewIcon"></i>
            <span>{{ viewLabel }}</span>
            <i class="bi bi-chevron-down chevron" :class="{ open: dropdownOpen }"></i>
          </button>
          <div class="view-menu" v-show="dropdownOpen">
            <a
              href="#"
              class="view-menu-item"
              :class="{ active: viewStore.currentView === 'live' }"
              @click.prevent="selectView('live')"
            >
              <i class="bi bi-radar"></i>
              <span>Live</span>
            </a>
            <a
              href="#"
              class="view-menu-item"
              :class="{ active: viewStore.currentView === 'log' }"
              @click.prevent="selectView('log')"
            >
              <i class="bi bi-card-list"></i>
              <span>Flight history</span>
            </a>
            <a
              href="#"
              class="view-menu-item"
              :class="{ active: viewStore.currentView === 'airlines' }"
              @click.prevent="selectView('airlines')"
            >
              <i class="bi bi-building"></i>
              <span>Airlines</span>
            </a>
          </div>
        </div>
        <button
          class="mil-toggle"
          :class="{ active: militaryStore.militaryOnly }"
          :disabled="viewStore.currentView === 'airlines'"
          @click="militaryStore.toggleFilter()"
          title="Military filter"
        >
          <i class="bi bi-shield-fill"></i>
          <span v-if="militaryCount > 0" class="mil-count">{{ militaryCount }}</span>
        </button>
        <button
          v-if="viewStore.currentView === 'live'"
          class="list-toggle"
          :class="{ active: viewStore.listVisible }"
          @click="viewStore.listVisible = !viewStore.listVisible"
          title="Toggle flight list"
        >
          <i class="bi bi-list-ul"></i>
        </button>
        <div class="nav-search" :class="{ collapsed: viewStore.currentView === 'live' && !viewStore.listVisible }">
          <i class="bi bi-search search-icon"></i>
          <input
            type="text"
            class="search-input"
            placeholder="Search..."
            v-model="viewStore.searchQuery"
            :tabindex="viewStore.currentView === 'live' && !viewStore.listVisible ? -1 : 0"
          />
        </div>
        <button
          v-if="viewStore.currentView === 'log'"
          class="list-toggle"
          @click="flightLogRef?.loadData()"
          :disabled="flightLogRef?.loading"
        >
          <i class="bi bi-arrow-clockwise" :class="{ spinning: flightLogRef?.loading }"></i>
        </button>
      </div>
    </nav>
    <div v-show="viewStore.currentView === 'live'">
      <LiveRadar />
    </div>
    <div v-show="viewStore.currentView === 'log'">
      <FlightLog ref="flightLogRef" />
    </div>
    <div v-show="viewStore.currentView === 'airlines'">
      <Airlines />
    </div>
  </template>
</template>

<script lang="ts">
import { defineComponent, watch, computed, ref, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { Tooltip } from 'bootstrap';
import { useViewStore } from './stores/viewStore';
import { useAircraftStore } from './stores/aircraft';
import { useMilitaryStore } from './stores/militaryStore';
import LiveRadar from './views/LiveRadar.vue';
import FlightLog from './views/FlightLog.vue';
import Airlines from './views/Airlines.vue';

export default defineComponent({
  name: 'App',

  components: {
    LiveRadar,
    FlightLog,
    Airlines,
  },

  setup() {
    const route = useRoute();
    const viewStore = useViewStore();
    const aircraftStore = useAircraftStore();
    const militaryStore = useMilitaryStore();
    const flightLogRef = ref<InstanceType<typeof FlightLog> | null>(null);
    const dropdownRef = ref<HTMLElement | null>(null);
    const dropdownOpen = ref(false);
    const isMobile = ref(window.innerWidth < 768);

    const viewLabels: Record<string, string> = {
      live: 'Live',
      log: 'Flight history',
      airlines: 'Airlines',
    };
    const viewIcons: Record<string, string> = {
      live: 'bi bi-radar',
      log: 'bi bi-card-list',
      airlines: 'bi bi-building',
    };
    const viewLabel = computed(() => viewLabels[viewStore.currentView] || 'Live');
    const viewIcon = computed(() => viewIcons[viewStore.currentView] || 'bi bi-radar');

    const selectView = (view: 'live' | 'log' | 'airlines') => {
      viewStore.setView(view);
      dropdownOpen.value = false;
    };

    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.value && !dropdownRef.value.contains(e.target as Node)) {
        dropdownOpen.value = false;
      }
    };

    const liveCount = computed(() => aircraftStore.activeAircraftList.length);

    const militaryCount = computed(() =>
      aircraftStore.activeAircraftList.filter(ac => militaryStore.isMilitary(ac.icao24)).length
    );

    const isDashboardRoute = computed(() => {
      return route.path.startsWith('/dashboard');
    });

    watch(liveCount, (count) => {
      document.title = count > 0 ? `Flightradar (${count})` : 'Flightradar';
    }, { immediate: true });

    // Listen for filter events from Airlines view
    const handleAirlineFilter = (event: Event) => {
      const icaoCode = (event as CustomEvent).detail;
      viewStore.showLog();
      // Use nextTick to ensure FlightLog is visible before applying filter
      setTimeout(() => {
        flightLogRef.value?.setFilter('airline', icaoCode);
      }, 50);
    };

    onMounted(() => {
      window.addEventListener('filter-airline', handleAirlineFilter);
      document.addEventListener('click', handleClickOutside);
    });

    onUnmounted(() => {
      window.removeEventListener('filter-airline', handleAirlineFilter);
      document.removeEventListener('click', handleClickOutside);
    });

    return {
      viewStore, militaryStore, militaryCount, isDashboardRoute, flightLogRef,
      dropdownRef, dropdownOpen, viewLabel, viewIcon, selectView, isMobile,
    };
  },

  mounted() {
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
  gap: 6px;
  padding: 6px 8px;
  border-bottom: none;
}

.view-dropdown {
  position: relative;
}

.view-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border: none;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 18px;
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 500;
  color: #333;
  transition: all 0.15s ease;
}

.view-toggle:hover {
  background: rgba(0, 0, 0, 0.1);
}

.view-toggle i {
  font-size: 0.85rem;
}

.chevron {
  font-size: 0.6rem !important;
  transition: transform 0.2s ease;
}

.chevron.open {
  transform: rotate(180deg);
}

.view-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  min-width: 160px;
  background: rgba(255, 255, 255, 0.95);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-radius: 12px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  padding: 4px;
  z-index: 600;
}

.view-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  text-decoration: none;
  font-size: 0.82rem;
  font-weight: 500;
  color: #555;
  border-radius: 8px;
  transition: all 0.15s ease;
}

.view-menu-item:hover {
  color: #222;
  background: rgba(0, 0, 0, 0.06);
}

.view-menu-item.active {
  color: #111;
  background: rgba(0, 0, 0, 0.09);
}

.view-menu-item i {
  font-size: 0.85rem;
}

.nav-search {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 4px;
  max-width: 200px;
  overflow: hidden;
  opacity: 1;
  transition: max-width 0.3s ease, opacity 0.2s ease, padding 0.3s ease;
}

.nav-search.collapsed {
  max-width: 0;
  opacity: 0;
  padding: 0;
  pointer-events: none;
}

.nav-search .search-icon {
  color: #999;
  font-size: 0.75rem;
}

.nav-search .search-input {
  width: 140px;
  padding: 6px 10px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 14px;
  font-size: 0.8rem;
  outline: none;
  background: rgba(0, 0, 0, 0.03);
  transition: border-color 0.2s, width 0.2s;
}

.nav-search .search-input:focus {
  border-color: #0d6efd;
  background: #fff;
  width: 180px;
}

.list-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid rgba(0, 0, 0, 0.12);
  background: transparent;
  border-radius: 50%;
  cursor: pointer;
  color: #555;
  font-size: 0.85rem;
  transition: all 0.15s ease;
}

.list-toggle:hover {
  background: rgba(0, 0, 0, 0.06);
}

.list-toggle.active {
  background: rgba(0, 0, 0, 0.09);
  border-color: rgba(0, 0, 0, 0.2);
  color: #111;
}

.list-toggle:disabled {
  opacity: 0.5;
  cursor: default;
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.mil-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid rgba(0, 0, 0, 0.12);
  background: transparent;
  border-radius: 50%;
  cursor: pointer;
  font-size: 0.85rem;
  color: #555;
  transition: all 0.15s ease;
}

.mil-toggle:has(.mil-count) {
  width: auto;
  padding: 0 10px;
  border-radius: 16px;
}

.mil-toggle:disabled {
  opacity: 0.35;
  cursor: default;
  pointer-events: none;
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

/* Mobile responsive navbar */
@media (max-width: 640px) {
  .app-nav {
    top: auto;
    bottom: 0;
    left: 0;
    right: 0;
    transform: none;
    border-radius: 0;
    box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.1);
    padding: 0;
    padding-bottom: env(safe-area-inset-bottom, 0);
  }

  .nav-tabs {
    display: flex;
    width: 100%;
    justify-content: center;
    gap: 4px;
    padding: 6px 8px;
  }

  .view-menu {
    bottom: calc(100% + 6px);
    top: auto;
  }

  .view-toggle {
    padding: 6px 10px;
    font-size: 0.75rem;
  }

  .nav-search .search-input {
    width: 100px;
    font-size: 0.75rem;
  }

  .nav-search .search-input:focus {
    width: 120px;
  }

  .mil-toggle {
    padding: 4px 8px;
    font-size: 0.72rem;
    border: none;
  }

  .mil-toggle i {
    font-size: 0.8rem;
  }

  .mil-count {
    font-size: 0.6rem;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
  }
}
</style>
