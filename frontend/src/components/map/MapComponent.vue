<template>
  <div 
    id="mapContainer" 
    ref="mapContainerRef"
    @click="handleMapClick"
    @mousedown="handleMapMouseDown"
    @mouseup="handleMapMouseUp"
    @mousemove="handleMapMouseMove"
  ></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import { useMapStore } from '@/stores';

const mapStore = useMapStore();

const mapContainerRef = ref<HTMLElement>();

// Map instance reference
let map: any = null;
let platform: any = null;

const props = defineProps({
  apikey: {
    type: String,
    required: true
  },
  lat: {
    type: String,
    default: '52.5'
  },
  lng: {
    type: String,
    default: '13.4'
  }
});

const emit = defineEmits(['mapInitialized', 'mapClick', 'mapMouseDown', 'mapMouseUp', 'mapMouseMove']);

declare let H: any;

// Watch for map center changes from store
watch(
  () => mapStore.center,
  (newCenter) => {
    if (map && mapStore.isInitialized) {
      map.setCenter(newCenter);
    }
  },
  { deep: true }
);

// Watch for map zoom changes from store
watch(
  () => mapStore.zoom,
  (newZoom) => {
    if (map && mapStore.isInitialized) {
      map.setZoom(newZoom);
    }
  }
);

onMounted(() => {
  initializeMap();
});

onBeforeUnmount(() => {
  cleanup();
});

const initializeMap = () => {
  mapStore.setLoading(true);
  
  try {
    // Initialize HERE platform
    platform = new H.service.Platform({
      apikey: props.apikey,
    });

    let defaultLayers = platform.createDefaultLayers();

    // Set initial map center from props or store default
    const initialCenter = {
      lat: Number(props.lat) || mapStore.center.lat,
      lng: Number(props.lng) || mapStore.center.lng
    };

    // Instantiate (and display) a map object
    map = new H.Map(mapContainerRef.value, defaultLayers.vector.normal.map, {
      center: initialCenter,
      zoom: mapStore.zoom,
      pixelRatio: window.devicePixelRatio || 1,
    });

    // Set map style
    let baseLayer = map.getBaseLayer();
    baseLayer.getProvider().setStyle(new H.map.Style(mapStore.config.styleUrl || '/radar-style.yml'));

    // Handle window resize
    const handleResize = () => {
      if (map) {
        map.getViewPort().resize();
      }
    };
    window.addEventListener('resize', handleResize);

    // Map events and behaviors
    new H.mapevents.Behavior(new H.mapevents.MapEvents(map));
    H.ui.UI.createDefault(map, defaultLayers);

    // Update store with initial state
    mapStore.setCenter(initialCenter);
    mapStore.setInitialized(true);
    mapStore.setLoading(false);
    mapStore.clearError();

    // Emit map initialization event with map instance
    emit('mapInitialized', { map, platform });

  } catch (error) {
    console.error('Error initializing map:', error);
    mapStore.setError(error as Error);
    mapStore.setLoading(false);
  }
};

const cleanup = () => {
  if (map) {
    map.dispose();
    map = null;
  }
  platform = null;
  mapStore.setInitialized(false);
};

// Event handlers
const handleMapClick = (event: Event) => {
  emit('mapClick', event);
};

const handleMapMouseDown = (event: Event) => {
  emit('mapMouseDown', event);
};

const handleMapMouseUp = (event: Event) => {
  emit('mapMouseUp', event);
};

const handleMapMouseMove = (event: Event) => {
  emit('mapMouseMove', event);
};

// Expose map instance and methods for parent component
defineExpose({
  map: () => map,
  platform: () => platform,
  getMapContainer: () => mapContainerRef.value
});
</script>

<style scoped>
#mapContainer {
  height: calc(100vh - 44px);
  width: 100%;
  position: relative;
  overflow: hidden;
}
</style>