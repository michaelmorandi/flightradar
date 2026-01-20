/**
 * Flight API Service
 *
 * Provides REST API access for fetching flight data, aircraft details,
 * and route information. This service handles non-streaming API calls.
 *
 * For live streaming data (positions, callsigns, categories), use
 * DataIngestionService instead.
 */

import Axios from 'axios';
import { setupCache, type AxiosCacheInstance, type CacheRequestConfig } from 'axios-cache-interceptor';
import { config } from '@/config';
import type { Flight, Aircraft, TerrestialPosition, PaginatedFlightsResponse } from '@/model/backendModel';

/** Cache TTL for flight list (1 second) */
const FLIGHTS_CACHE_TTL = 1000;

/** Cache TTL for aircraft details (1 hour) */
const AIRCRAFT_CACHE_TTL = 1000 * 60 * 60;

/** HexDB API base path for route information */
const HEXDB_API_BASEPATH = 'https://hexdb.io/api/v1/';

export class FlightApiService {
  private axios: AxiosCacheInstance;
  private apiUrl: string;
  private authConfig: { auth?: { username: string; password: string } };

  constructor() {
    const instance = Axios.create();
    this.axios = setupCache(instance);

    this.apiUrl = config.flightApiUrl || '';

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
   * Get list of past flights from the database with pagination.
   *
   * @param limit Maximum number of flights to return per page
   * @param mil Filter by military status: true for military only, false for civilian only, undefined for all
   * @param page Page number (1-indexed)
   */
  async getFlights(limit: number = 50, mil?: boolean, page: number = 1): Promise<PaginatedFlightsResponse> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return {
        flights: [],
        total: 0,
        page: 1,
        pageSize: limit,
        totalPages: 0
      };
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: FLIGHTS_CACHE_TTL },
    };

    const params = new URLSearchParams({
      limit: limit.toString(),
      page: page.toString()
    });
    if (mil !== undefined) {
      params.append('mil', mil.toString());
    }

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/flights?${params}`,
        { ...cacheConfig, ...this.authConfig }
      );

      if (response.status >= 200 && response.status < 300) {
        const data = response.data;
        // Convert date strings to Date objects
        data.flights = data.flights.map((flight: any) => ({
          ...flight,
          firstCntct: new Date(flight.firstCntct),
          lstCntct: new Date(flight.lstCntct)
        }));
        return data;
      }

      throw new Error(response.statusText || 'Error retrieving flights');
    } catch (error) {
      console.error('Error fetching flights:', error);
      throw error;
    }
  }

  /**
   * Get flight details by ID.
   */
  async getFlight(flightId: string): Promise<Flight | null> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return null;
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: FLIGHTS_CACHE_TTL },
    };

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/flights/${flightId}`,
        { ...cacheConfig, ...this.authConfig }
      );

      if (response.status >= 200 && response.status < 300) {
        return response.data;
      }

      return null;
    } catch (error) {
      console.error(`Error fetching flight ${flightId}:`, error);
      throw error;
    }
  }

  /**
   * Get aircraft details by ICAO24 address.
   */
  async getAircraft(icao24: string): Promise<Aircraft | null> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return null;
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: AIRCRAFT_CACHE_TTL },
    };

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/aircraft/${icao24}`,
        { ...cacheConfig, ...this.authConfig }
      );

      if (response.status >= 200 && response.status < 300) {
        return response.data;
      }

      return null;
    } catch (error: unknown) {
      // 404 is expected for unknown aircraft
      if (Axios.isAxiosError(error) && error.response?.status === 404) {
        console.debug(`Aircraft not found: ${icao24}`);
        return null;
      }

      console.error(`Error fetching aircraft ${icao24}:`, error);
      throw error;
    }
  }

  /**
   * Get historical positions for a flight.
   */
  async getPositions(flightId: string): Promise<TerrestialPosition[]> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return [];
    }

    const cacheConfig: CacheRequestConfig = {
      cache: false, // Don't cache positions
    };

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/flights/${flightId}/positions`,
        { ...cacheConfig, ...this.authConfig }
      );

      if (response.status >= 200 && response.status < 300) {
        // Backend returns positions as arrays [lat, lon, alt]
        return response.data.map((arr: number[]) => ({
          lat: arr[0],
          lon: arr[1],
          alt: arr[2],
          icao: '',
          callsign: '',
        } as TerrestialPosition));
      }

      return [];
    } catch (error) {
      console.error(`Error fetching positions for flight ${flightId}:`, error);
      throw error;
    }
  }

  /**
   * Get flight route information from HexDB.
   */
  async getFlightRoute(callsign: string): Promise<string | null> {
    try {
      // Use a separate axios instance without caching for CORS reasons
      const response = await Axios.get(`${HEXDB_API_BASEPATH}route/iata/${callsign}`);

      if (response.status >= 200 && response.status < 300 && response.data?.route) {
        return response.data.route;
      }

      return null;
    } catch (error) {
      console.debug(`Route not found for callsign ${callsign}`);
      return null;
    }
  }
}

// Singleton instance
let instance: FlightApiService | null = null;

/**
 * Get the FlightApiService singleton instance.
 */
export function getFlightApiService(): FlightApiService {
  if (!instance) {
    instance = new FlightApiService();
  }
  return instance;
}

/**
 * Create a new FlightApiService instance.
 */
export function createFlightApiService(): FlightApiService {
  return new FlightApiService();
}
