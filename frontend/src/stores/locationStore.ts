import { defineStore } from 'pinia';
import { ref, computed } from 'vue';

export const useLocationStore = defineStore('location', () => {
  const enabled = ref(false);
  const lat = ref<number | null>(null);
  const lng = ref<number | null>(null);
  const error = ref<string | null>(null);
  let watchId: number | null = null;

  const hasLocation = computed(() => lat.value !== null && lng.value !== null);

  function enable() {
    if (!navigator.geolocation) {
      error.value = 'Geolocation is not supported by your browser';
      return;
    }
    enabled.value = true;
    error.value = null;
    watchId = navigator.geolocation.watchPosition(
      (pos) => {
        lat.value = pos.coords.latitude;
        lng.value = pos.coords.longitude;
        error.value = null;
      },
      (err) => {
        error.value = err.message;
        enabled.value = false;
      },
      { enableHighAccuracy: true }
    );
  }

  function disable() {
    enabled.value = false;
    if (watchId !== null) {
      navigator.geolocation.clearWatch(watchId);
      watchId = null;
    }
    lat.value = null;
    lng.value = null;
  }

  function toggle() {
    if (enabled.value) {
      disable();
    } else {
      enable();
    }
  }

  return { enabled, lat, lng, error, hasLocation, toggle, enable, disable };
});
