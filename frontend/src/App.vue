<template>
  <nav class="app-nav">
    <div class="nav-tabs">
      <router-link to="/" class="nav-tab">
        <i class="bi bi-radar"></i>
        <span>Live</span>
      </router-link>
      <router-link to="/flightlog" class="nav-tab">
        <i class="bi bi-card-list"></i>
        <span>Log</span>
      </router-link>
    </div>
  </nav>
  <router-view />
</template>

<script>
import { defineComponent } from 'vue';
import { Tooltip } from 'bootstrap';

export default defineComponent({
  name: 'App',

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

.nav-tab.router-link-exact-active {
  color: #111;
  background: #f0f0f0;
}

.nav-tab i {
  font-size: 0.9rem;
}
</style>
