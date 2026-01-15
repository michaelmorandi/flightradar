import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import { getDataIngestionService } from './services/dataIngestionService';

// Create the Vue app
const app = createApp(App);
const pinia = createPinia();

// Initialize Pinia first (stores need this)
app.use(pinia);

// Initialize the data ingestion service and connect
const dataService = getDataIngestionService();
dataService.connect();

// Provide the service for components that need direct access
app.provide('dataIngestionService', dataService);

// Mount the app
app.use(router).mount('#app');
