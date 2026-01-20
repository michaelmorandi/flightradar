import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import { getDataIngestionService } from './services/dataIngestionService';
import { getAuthService } from './services/authService';

/**
 * Initialize the application with authentication.
 *
 * Authentication must complete before the app can make API calls.
 */
async function initApp() {
  // Create the Vue app
  const app = createApp(App);
  const pinia = createPinia();

  // Initialize Pinia first (stores need this)
  app.use(pinia);

  try {
    // Initialize authentication first
    console.log('[App] Initializing authentication...');
    const authService = getAuthService();
    await authService.initialize();
    console.log('[App] Authentication initialized successfully');

    // Provide auth service for components that need it
    app.provide('authService', authService);

    // Initialize the data ingestion service and connect
    const dataService = getDataIngestionService();
    dataService.connect();

    // Provide the service for components that need direct access
    app.provide('dataIngestionService', dataService);

    // Mount the app
    app.use(router).mount('#app');
    console.log('[App] Application mounted successfully');
  } catch (error) {
    console.error('[App] Failed to initialize application:', error);

    // Show error to user
    const errorDiv = document.createElement('div');
    errorDiv.style.cssText = `
      position: fixed;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      padding: 20px;
      background: #f44336;
      color: white;
      border-radius: 4px;
      font-family: sans-serif;
      max-width: 500px;
      text-align: center;
    `;
    errorDiv.innerHTML = `
      <h2>Authentication Failed</h2>
      <p>Unable to connect to the API. Please check your configuration.</p>
      <p style="font-size: 12px; margin-top: 10px;">Error: ${error instanceof Error ? error.message : 'Unknown error'}</p>
    `;
    document.body.appendChild(errorDiv);
  }
}

// Start the application
initApp();
