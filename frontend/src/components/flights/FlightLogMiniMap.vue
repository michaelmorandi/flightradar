<template>
  <div class="mini-map-container" v-if="visible">
    <div class="mini-map-header">
      <span class="mini-map-title">Flight Path</span>
      <button class="mini-map-close" @click="$emit('close')" aria-label="Close map">
        <i class="bi bi-x-lg"></i>
      </button>
    </div>
    <div ref="mapContainerRef" class="mini-map"></div>
    <div v-if="loading" class="mini-map-loading">
      <div class="spinner-border spinner-border-sm" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onBeforeUnmount, watch } from 'vue';
import { config } from '@/config';
import { getFlightApiService } from '@/services/flightApiService';
import { FlightPath } from '@/components/map/flightPath';
import type { TerrestialPosition } from '@/model/backendModel';
import { loadIconSvg, determineAircraftCategory, mapProtobufCategoryToIcon, type AircraftCategory } from '@/utils/aircraftIcons';

declare let H: any;

const MAP_STYLE_URL = '/radar-style.yml';

const props = defineProps({
  flightId: { type: String, required: true },
  visible: { type: Boolean, default: false },
});

const emit = defineEmits(['close']);

const mapContainerRef = ref<HTMLElement>();
const loading = ref(false);

let map: any = null;
let platform: any = null;
let ui: any = null;
let flightPath: FlightPath | null = null;
let aircraftMarker: any = null;

const apiService = getFlightApiService();

const toggleFullscreen = () => {
  const container = mapContainerRef.value?.parentElement;
  if (!container) return;

  if (!document.fullscreenElement) {
    container.requestFullscreen().catch(err => {
      console.error('Error attempting fullscreen:', err);
    });
  } else {
    document.exitFullscreen();
  }
};

const handleFullscreenChange = () => {
  // Resize map after fullscreen transition
  setTimeout(() => {
    if (map) {
      map.getViewPort().resize();
    }
  }, 100);
};

const initializeMap = async () => {
  if (!mapContainerRef.value || !config.hereApiKey) return;

  loading.value = true;

  try {
    platform = new H.service.Platform({
      apikey: config.hereApiKey,
    });

    const defaultLayers = platform.createDefaultLayers();

    map = new H.Map(mapContainerRef.value, defaultLayers.vector.normal.map, {
      center: { lat: 47.3769, lng: 8.5417 },
      zoom: 6,
      pixelRatio: window.devicePixelRatio || 1,
    });

    // Apply the same style as the main map
    const baseLayer = map.getBaseLayer();
    baseLayer.getProvider().setStyle(new H.map.Style(MAP_STYLE_URL));

    // Add map behaviors (zoom, pan)
    new H.mapevents.Behavior(new H.mapevents.MapEvents(map));

    // Create UI without default controls, then add only what we need
    ui = H.ui.UI.createDefault(map, defaultLayers, 'en-US');

    // Remove controls we don't want
    const mapSettingsControl = ui.getControl('mapsettings');
    if (mapSettingsControl) {
      ui.removeControl('mapsettings');
    }

    // Add fullscreen control - create custom button since HERE Maps doesn't have built-in fullscreen
    const fullscreenBtn = document.createElement('button');
    fullscreenBtn.className = 'mini-map-fullscreen-btn';
    fullscreenBtn.innerHTML = '<i class="bi bi-arrows-fullscreen"></i>';
    fullscreenBtn.title = 'Toggle fullscreen';
    fullscreenBtn.onclick = () => toggleFullscreen();
    mapContainerRef.value.appendChild(fullscreenBtn);

    // Handle container resize
    const resizeObserver = new ResizeObserver(() => {
      if (map) {
        map.getViewPort().resize();
      }
    });
    resizeObserver.observe(mapContainerRef.value);

    // Handle fullscreen change to resize map
    document.addEventListener('fullscreenchange', handleFullscreenChange);

    // Load flight path
    await loadFlightPath();
  } catch (error) {
    console.error('Error initializing mini map:', error);
  } finally {
    loading.value = false;
  }
};

const calculateHeading = (lat1: number, lon1: number, lat2: number, lon2: number): number => {
  const dLon = (lon2 - lon1) * Math.PI / 180;
  const y = Math.sin(dLon) * Math.cos(lat2 * Math.PI / 180);
  const x = Math.cos(lat1 * Math.PI / 180) * Math.sin(lat2 * Math.PI / 180) -
            Math.sin(lat1 * Math.PI / 180) * Math.cos(lat2 * Math.PI / 180) * Math.cos(dLon);
  const heading = Math.atan2(y, x) * 180 / Math.PI;
  return (heading + 360) % 360;
};

const createAircraftMarker = async (positions: TerrestialPosition[]) => {
  if (!map || positions.length === 0) return;

  const lastPosition = positions[positions.length - 1];

  let heading = 0;
  if (positions.length >= 2) {
    const secondLastPosition = positions[positions.length - 2];
    heading = calculateHeading(
      secondLastPosition.lat,
      secondLastPosition.lon,
      lastPosition.lat,
      lastPosition.lon
    );
  } else if (lastPosition.track !== undefined) {
    heading = lastPosition.track;
  }

  try {
    const flight = await apiService.getFlight(props.flightId);
    let category: AircraftCategory = 'default';
    let protoCategory: number | undefined;

    if (flight) {
      const aircraft = await apiService.getAircraft(flight.icao24);
      if (aircraft) {
        category = determineAircraftCategory(aircraft.type, aircraft.icaoType);
      }
    }

    if (lastPosition.cat !== undefined) {
      protoCategory = lastPosition.cat;
      category = mapProtobufCategoryToIcon(protoCategory);
    }

    const svgContent = await loadIconSvg(category);
    const parser = new DOMParser();
    const doc = parser.parseFromString(svgContent, 'image/svg+xml');
    const svgElement = doc.querySelector('svg');

    if (!svgElement) return;

    const width = parseFloat(svgElement.getAttribute('width') || '22.5');
    const height = parseFloat(svgElement.getAttribute('height') || '30');

    svgElement.style.cssText = `
      position: absolute;
      left: ${-width / 2}px;
      top: ${-height / 2}px;
      width: ${width}px;
      height: ${height}px;
      transform: rotate(${heading}deg);
    `;

    const iconWrapper = document.createElement('div');
    iconWrapper.className = 'aircraft-icon-marker';
    iconWrapper.appendChild(svgElement);

    const domIcon = new H.map.DomIcon(iconWrapper);
    aircraftMarker = new H.map.DomMarker(
      { lat: lastPosition.lat, lng: lastPosition.lon },
      { icon: domIcon }
    );

    map.addObject(aircraftMarker);
  } catch (error) {
    console.error('Error creating aircraft marker:', error);
  }
};

const loadFlightPath = async () => {
  if (!map || !props.flightId) return;

  try {
    const positions = await apiService.getPositions(props.flightId);

    if (positions.length > 0) {
      flightPath = new FlightPath(props.flightId, map);
      flightPath.createFlightPath(positions);

      await createAircraftMarker(positions);

      fitBounds(positions);
    }
  } catch (error) {
    console.error('Error loading flight path:', error);
  }
};

const fitBounds = (positions: TerrestialPosition[]) => {
  if (!map || positions.length === 0) return;

  let minLat = positions[0].lat;
  let maxLat = positions[0].lat;
  let minLng = positions[0].lon;
  let maxLng = positions[0].lon;

  positions.forEach(pos => {
    minLat = Math.min(minLat, pos.lat);
    maxLat = Math.max(maxLat, pos.lat);
    minLng = Math.min(minLng, pos.lon);
    maxLng = Math.max(maxLng, pos.lon);
  });

  // Add 10% margin around the flight path
  const latPadding = (maxLat - minLat) * 0.1;
  const lngPadding = (maxLng - minLng) * 0.1;

  // Ensure minimum padding for very short flights
  const minPadding = 0.01; // ~1km
  const effectiveLatPadding = Math.max(latPadding, minPadding);
  const effectiveLngPadding = Math.max(lngPadding, minPadding);

  const bounds = new H.geo.Rect(
    maxLat + effectiveLatPadding,
    minLng - effectiveLngPadding,
    minLat - effectiveLatPadding,
    maxLng + effectiveLngPadding
  );

  map.getViewModel().setLookAtData({
    bounds: bounds,
  }, true);
};

const cleanup = () => {
  document.removeEventListener('fullscreenchange', handleFullscreenChange);

  if (aircraftMarker && map) {
    map.removeObject(aircraftMarker);
    aircraftMarker = null;
  }
  if (flightPath) {
    flightPath.removeFlightPath();
    flightPath = null;
  }
  if (ui) {
    ui.dispose();
    ui = null;
  }
  if (map) {
    map.dispose();
    map = null;
  }
  platform = null;
};

watch(() => props.visible, (newVisible) => {
  if (newVisible) {
    // Use nextTick to ensure DOM is ready
    setTimeout(() => {
      initializeMap();
    }, 50);
  } else {
    cleanup();
  }
});

onBeforeUnmount(() => {
  cleanup();
});
</script>

<style scoped>
.mini-map-container {
  margin-top: 8px;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  overflow: hidden;
  background: #fff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.mini-map-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #f8f9fa;
  border-bottom: 1px solid #e0e0e0;
}

.mini-map-title {
  font-size: 0.85rem;
  font-weight: 500;
  color: #495057;
}

.mini-map-close {
  background: none;
  border: none;
  padding: 4px 8px;
  cursor: pointer;
  color: #6c757d;
  border-radius: 4px;
  transition: all 0.15s ease;
}

.mini-map-close:hover {
  background: #e9ecef;
  color: #212529;
}

.mini-map {
  height: 250px;
  width: 100%;
  position: relative;
}

.mini-map-loading {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: rgba(255, 255, 255, 0.9);
  padding: 12px;
  border-radius: 8px;
}

/* Style HERE Maps UI controls to fit the mini map */
.mini-map :deep(.H_ui) {
  font-size: 12px;
}

.mini-map :deep(.H_ctl) {
  margin: 8px;
}

/* Fullscreen button styling */
.mini-map-container :deep(.mini-map-fullscreen-btn) {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 100;
  width: 32px;
  height: 32px;
  background: white;
  border: 1px solid #ccc;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
  transition: all 0.15s ease;
}

.mini-map-container :deep(.mini-map-fullscreen-btn:hover) {
  background: #f0f0f0;
  border-color: #999;
}

.mini-map-container :deep(.mini-map-fullscreen-btn i) {
  font-size: 14px;
  color: #333;
}

/* Fullscreen mode styling */
.mini-map-container:fullscreen {
  background: white;
}

.mini-map-container:fullscreen .mini-map {
  height: calc(100vh - 50px);
}

.mini-map-container:fullscreen .mini-map-header {
  padding: 12px 16px;
}
</style>
