import { defineStore } from 'pinia';
import { ref, computed } from 'vue';

export interface MapCenter {
  lat: number;
  lng: number;
}

export interface MapViewport {
  center: MapCenter;
  zoom: number;
}

export interface MapConfiguration {
  apiKey: string;
  styleUrl?: string;
  defaultZoom: number;
  minZoom?: number;
  maxZoom?: number;
  aerialOverview: boolean;
  periodicRefresh: boolean;
  refreshInterval: number; // in milliseconds
}

export const useMapStore = defineStore('map', () => {
  // State
  const center = ref<MapCenter>({ lat: 47.3769, lng: 8.5417 }); // Default to Zurich
  const zoom = ref<number>(8);
  const isInitialized = ref<boolean>(false);
  const isLoading = ref<boolean>(false);
  const error = ref<Error | null>(null);
  
  // Map configuration
  const config = ref<MapConfiguration>({
    apiKey: '',
    styleUrl: '/radar-style.yml',
    defaultZoom: 8,
    minZoom: 2,
    maxZoom: 18,
    aerialOverview: true,
    periodicRefresh: true,
    refreshInterval: 1000
  });

  // Flight path and marker state
  const highlightedFlightId = ref<string | null>(null);
  const selectedMarkerId = ref<string | null>(null);
  const showFlightPaths = ref<boolean>(true);
  const showAircraftMarkers = ref<boolean>(true);
  
  // Map layers visibility
  const layerVisibility = ref({
    aircraft: true,
    flightPaths: true,
    airports: false,
    weather: false,
    airspace: false
  });

  // Getters
  const viewport = computed<MapViewport>(() => ({
    center: center.value,
    zoom: zoom.value
  }));

  const isFlightHighlighted = computed(() => highlightedFlightId.value !== null);
  
  const isMarkerSelected = computed(() => selectedMarkerId.value !== null);
  
  const mapConfiguration = computed(() => config.value);
  
  const isAerialViewEnabled = computed(() => config.value.aerialOverview);
  
  const isPeriodicRefreshEnabled = computed(() => config.value.periodicRefresh);

  // Actions
  const setCenter = (newCenter: MapCenter) => {
    center.value = { ...newCenter };
  };

  const setZoom = (newZoom: number) => {
    const minZoom = config.value.minZoom || 2;
    const maxZoom = config.value.maxZoom || 18;
    zoom.value = Math.max(minZoom, Math.min(maxZoom, newZoom));
  };

  const setViewport = (newViewport: MapViewport) => {
    setCenter(newViewport.center);
    setZoom(newViewport.zoom);
  };

  const panTo = (coordinates: MapCenter, zoomLevel?: number) => {
    setCenter(coordinates);
    if (zoomLevel !== undefined) {
      setZoom(zoomLevel);
    }
  };

  const highlightFlight = (flightId: string | null) => {
    highlightedFlightId.value = flightId;
  };

  const selectMarker = (markerId: string | null) => {
    selectedMarkerId.value = markerId;
  };

  const clearHighlight = () => {
    highlightedFlightId.value = null;
  };

  const clearSelection = () => {
    selectedMarkerId.value = null;
  };

  const updateConfig = (newConfig: Partial<MapConfiguration>) => {
    config.value = { ...config.value, ...newConfig };
  };

  const setApiKey = (apiKey: string) => {
    config.value.apiKey = apiKey;
  };

  const toggleAerialOverview = () => {
    config.value.aerialOverview = !config.value.aerialOverview;
  };

  const togglePeriodicRefresh = () => {
    config.value.periodicRefresh = !config.value.periodicRefresh;
  };

  const setRefreshInterval = (interval: number) => {
    config.value.refreshInterval = Math.max(500, interval); // Minimum 500ms
  };

  const toggleFlightPaths = () => {
    showFlightPaths.value = !showFlightPaths.value;
    layerVisibility.value.flightPaths = showFlightPaths.value;
  };

  const toggleAircraftMarkers = () => {
    showAircraftMarkers.value = !showAircraftMarkers.value;
    layerVisibility.value.aircraft = showAircraftMarkers.value;
  };

  const toggleLayer = (layer: keyof typeof layerVisibility.value) => {
    layerVisibility.value[layer] = !layerVisibility.value[layer];
  };

  const setLayerVisibility = (layer: keyof typeof layerVisibility.value, visible: boolean) => {
    layerVisibility.value[layer] = visible;
  };

  const setInitialized = (initialized: boolean) => {
    isInitialized.value = initialized;
  };

  const setLoading = (loading: boolean) => {
    isLoading.value = loading;
  };

  const setError = (newError: Error | null) => {
    error.value = newError;
  };

  const clearError = () => {
    error.value = null;
  };

  // Utility actions
  const fitToFlightPath = (coordinates: MapCenter[]) => {
    if (coordinates.length === 0) return;
    
    if (coordinates.length === 1) {
      panTo(coordinates[0], 12);
      return;
    }

    // Calculate bounds
    let minLat = coordinates[0].lat;
    let maxLat = coordinates[0].lat;
    let minLng = coordinates[0].lng;
    let maxLng = coordinates[0].lng;

    coordinates.forEach(coord => {
      minLat = Math.min(minLat, coord.lat);
      maxLat = Math.max(maxLat, coord.lat);
      minLng = Math.min(minLng, coord.lng);
      maxLng = Math.max(maxLng, coord.lng);
    });

    const centerLat = (minLat + maxLat) / 2;
    const centerLng = (minLng + maxLng) / 2;
    
    setCenter({ lat: centerLat, lng: centerLng });
    
    // Calculate appropriate zoom level based on bounds
    const latDiff = maxLat - minLat;
    const lngDiff = maxLng - minLng;
    const maxDiff = Math.max(latDiff, lngDiff);
    
    let zoomLevel = 10;
    if (maxDiff > 10) zoomLevel = 4;
    else if (maxDiff > 5) zoomLevel = 6;
    else if (maxDiff > 2) zoomLevel = 8;
    else if (maxDiff > 1) zoomLevel = 10;
    else if (maxDiff > 0.5) zoomLevel = 12;
    else zoomLevel = 14;
    
    setZoom(zoomLevel);
  };

  const resetToDefault = () => {
    setCenter({ lat: 47.3769, lng: 8.5417 });
    setZoom(config.value.defaultZoom);
    clearHighlight();
    clearSelection();
  };

  // Reset store state
  const $reset = () => {
    center.value = { lat: 47.3769, lng: 8.5417 };
    zoom.value = 8;
    isInitialized.value = false;
    isLoading.value = false;
    error.value = null;
    highlightedFlightId.value = null;
    selectedMarkerId.value = null;
    showFlightPaths.value = true;
    showAircraftMarkers.value = true;
    layerVisibility.value = {
      aircraft: true,
      flightPaths: true,
      airports: false,
      weather: false,
      airspace: false
    };
    config.value = {
      apiKey: '',
      styleUrl: '/radar-style.yml',
      defaultZoom: 8,
      minZoom: 2,
      maxZoom: 18,
      aerialOverview: true,
      periodicRefresh: true,
      refreshInterval: 1000
    };
  };

  return {
    // State
    center,
    zoom,
    isInitialized,
    isLoading,
    error,
    config,
    highlightedFlightId,
    selectedMarkerId,
    showFlightPaths,
    showAircraftMarkers,
    layerVisibility,
    
    // Getters
    viewport,
    isFlightHighlighted,
    isMarkerSelected,
    mapConfiguration,
    isAerialViewEnabled,
    isPeriodicRefreshEnabled,
    
    // Actions
    setCenter,
    setZoom,
    setViewport,
    panTo,
    highlightFlight,
    selectMarker,
    clearHighlight,
    clearSelection,
    updateConfig,
    setApiKey,
    toggleAerialOverview,
    togglePeriodicRefresh,
    setRefreshInterval,
    toggleFlightPaths,
    toggleAircraftMarkers,
    toggleLayer,
    setLayerVisibility,
    setInitialized,
    setLoading,
    setError,
    clearError,
    fitToFlightPath,
    resetToDefault,
    $reset
  };
});