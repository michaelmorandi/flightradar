// Simple configuration using Vite environment variables
// For Docker runtime configuration, these will be replaced by envsubst in the entrypoint script
export const config = {
  flightApiUrl: import.meta.env.VITE_FLIGHT_API_URL || '${VITE_FLIGHT_API_URL}' || undefined,
  flightApiUser: import.meta.env.VITE_FLIGHT_API_USER || '${VITE_FLIGHT_API_USER}' || undefined,
  flightApiPassword: import.meta.env.VITE_FLIGHT_API_PASSWORD || '${VITE_FLIGHT_API_PASSWORD}' || undefined,
  hereApiKey: import.meta.env.VITE_HERE_API_KEY || '${VITE_HERE_API_KEY}' || '',
  mockData: (import.meta.env.VITE_MOCK_DATA || '${VITE_MOCK_DATA}') === 'true',
  isDevelopment: import.meta.env.MODE === 'development',
  isProduction: import.meta.env.MODE === 'production'
} as const;

// Simple utility functions
export const isMockDataEnabled = () => config.mockData;
export const isDevelopment = () => config.isDevelopment;
export const isProduction = () => config.isProduction;