import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import { FlightRadarService } from './services/flightRadarService';
import { FlightRadarServiceImpl } from './services/flightRadarServiceImpl';
import { FlightRadarServiceMock } from './services/flightRadarServiceMock';
import { isMockDataEnabled } from './config';

// Choose the appropriate service implementation based on configuration
let baseService: FlightRadarService;

// Choose the appropriate service implementation
if (isMockDataEnabled()) {
  // Using mock data mode
  baseService = new FlightRadarServiceMock();
} else {
  // Using real API mode
  const frService = new FlightRadarServiceImpl();
  frService.connect();
  baseService = frService;
}

// Wrap the base service with the adapter to ensure compatibility
const frService = baseService;

// Create the Vue app
const app = createApp(App);
const pinia = createPinia();

// Register the flight radar service as a global dependency
app.provide('frService', frService);

// Mount the app
app.use(pinia).use(router).mount('#app');
