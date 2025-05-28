/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_FLIGHT_API_URL: string;
  readonly VITE_FLIGHT_API_USER?: string;
  readonly VITE_FLIGHT_API_PASSWORD?: string;
  readonly VITE_HERE_API_KEY: string;
  readonly VITE_MOCK_DATA?: string;
  readonly MODE: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}