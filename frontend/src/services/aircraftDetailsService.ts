/**
 * Aircraft Details Service
 *
 * Lazy-loads static aircraft information (type, registration, operator)
 * on demand when an aircraft is selected.
 */

import Axios from 'axios';
import { setupCache, type AxiosCacheInstance, type CacheRequestConfig } from 'axios-cache-interceptor';
import type { AircraftDetails } from '@/stores/aircraft';
import { useAircraftStore } from '@/stores/aircraft';
import { config } from '@/config';

/** Cache TTL for aircraft details (1 hour) */
const CACHE_TTL_MS = 1000 * 60 * 60;

export class AircraftDetailsService {
  private axios: AxiosCacheInstance;
  private apiUrl: string;
  private cacheConfig: CacheRequestConfig;
  private authConfig: { auth?: { username: string; password: string } };

  // Track in-flight requests to prevent duplicates
  private pendingRequests: Map<string, Promise<AircraftDetails | null>> = new Map();

  constructor() {
    const instance = Axios.create();
    this.axios = setupCache(instance);

    this.apiUrl = config.flightApiUrl || '';

    this.cacheConfig = {
      cache: {
        ttl: CACHE_TTL_MS,
      },
    };

    this.authConfig = config.flightApiUser
      ? {
          auth: {
            username: config.flightApiUser,
            password: config.flightApiPassword || '',
          },
        }
      : {};
  }

  /**
   * Fetch aircraft details by ICAO24 address.
   * Results are cached both in axios and in the aircraft store.
   *
   * @param icao24 The 24-bit ICAO transponder address
   * @returns Aircraft details or null if not found
   */
  async fetchAircraftDetails(icao24: string): Promise<AircraftDetails | null> {
    // Check if we already have this data cached in the store
    const aircraftStore = useAircraftStore();
    const cached = aircraftStore.getAircraftDetails(icao24);
    if (cached) {
      return cached;
    }

    // Check for in-flight request
    const pending = this.pendingRequests.get(icao24);
    if (pending) {
      return pending;
    }

    // Start new request
    const request = this.doFetch(icao24);
    this.pendingRequests.set(icao24, request);

    try {
      const result = await request;

      // Cache in store if we got data
      if (result) {
        aircraftStore.cacheAircraftDetails(result);
      }

      return result;
    } finally {
      this.pendingRequests.delete(icao24);
    }
  }

  /**
   * Fetch aircraft details for the currently selected aircraft.
   * Convenience method that gets the ICAO24 from the selection.
   */
  async fetchSelectedAircraftDetails(): Promise<AircraftDetails | null> {
    const aircraftStore = useAircraftStore();
    const selected = aircraftStore.selectedAircraft;

    if (!selected) {
      return null;
    }

    return this.fetchAircraftDetails(selected.icao24);
  }

  /**
   * Pre-fetch aircraft details for a list of ICAO24 addresses.
   * Useful for batch loading when many aircraft are visible.
   *
   * @param icao24s List of ICAO24 addresses to fetch
   * @param maxConcurrent Maximum concurrent requests (default 5)
   */
  async prefetchMultiple(icao24s: string[], maxConcurrent = 5): Promise<void> {
    // Filter out already cached
    const aircraftStore = useAircraftStore();
    const toFetch = icao24s.filter(icao24 => !aircraftStore.getAircraftDetails(icao24));

    if (toFetch.length === 0) {
      return;
    }

    // Fetch in batches
    for (let i = 0; i < toFetch.length; i += maxConcurrent) {
      const batch = toFetch.slice(i, i + maxConcurrent);
      await Promise.all(batch.map(icao24 => this.fetchAircraftDetails(icao24)));
    }
  }

  /**
   * Check if details are cached for an ICAO24.
   */
  isCached(icao24: string): boolean {
    const aircraftStore = useAircraftStore();
    return aircraftStore.getAircraftDetails(icao24) !== undefined;
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE
  // ═══════════════════════════════════════════════════════════

  private async doFetch(icao24: string): Promise<AircraftDetails | null> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return null;
    }

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/aircraft/${icao24}`,
        { ...this.cacheConfig, ...this.authConfig }
      );

      if (response.status >= 200 && response.status < 300 && response.data) {
        const data = response.data;
        return {
          icao24: data.icao24 || icao24,
          type: data.type,
          icaoType: data.icaoType,
          registration: data.reg,
          operator: data.op,
        };
      }

      return null;
    } catch (error: unknown) {
      // 404 is expected for unknown aircraft
      if (Axios.isAxiosError(error) && error.response?.status === 404) {
        console.debug(`Aircraft not found: ${icao24}`);
        // Cache empty result to prevent repeated lookups
        return {
          icao24,
        };
      }

      console.error(`Error fetching aircraft details for ${icao24}:`, error);
      return null;
    }
  }
}

// Singleton instance
let instance: AircraftDetailsService | null = null;

/**
 * Get the AircraftDetailsService singleton instance.
 */
export function getAircraftDetailsService(): AircraftDetailsService {
  if (!instance) {
    instance = new AircraftDetailsService();
  }
  return instance;
}

/**
 * Create a new AircraftDetailsService instance.
 */
export function createAircraftDetailsService(): AircraftDetailsService {
  return new AircraftDetailsService();
}
