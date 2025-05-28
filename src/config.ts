// Simple configuration using Vite environment variables
export const config = {
  flightApiUrl: import.meta.env.VITE_FLIGHT_API_URL || 'https://flights-api.example.com/api/v1',
  flightApiUser: import.meta.env.VITE_FLIGHT_API_USER,
  flightApiPassword: import.meta.env.VITE_FLIGHT_API_PASSWORD,
  hereApiKey: import.meta.env.VITE_HERE_API_KEY || '',
  mockData: import.meta.env.VITE_MOCK_DATA === 'true',
  isDevelopment: import.meta.env.MODE === 'development',
  isProduction: import.meta.env.MODE === 'production'
} as const;

// Simple utility functions
export const isMockDataEnabled = () => config.mockData;
export const isDevelopment = () => config.isDevelopment;
export const isProduction = () => config.isProduction;