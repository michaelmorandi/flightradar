/**
 * Flight API Service — REST client for the Rust backend.
 *
 * Live streaming (positions / per-flight history feed) lives in
 * DataIngestionService; this file is request/response only.
 *
 * All endpoints live under `/api/v1` and use snake_case JSON; field
 * mapping happens once here at the boundary so the rest of the app
 * doesn't see wire names.
 */

import Axios from 'axios';
import { setupCache, type AxiosCacheInstance, type CacheRequestConfig } from 'axios-cache-interceptor';
import { config } from '@/config';
import type {
  Aircraft,
  Airline,
  AirportInfo,
  Flight,
  FlightFilters,
  PaginatedFlightsResponse,
  PositionRecord,
} from '@/model/backendModel';

/** Cache TTL for flight list (1 second) */
const FLIGHTS_CACHE_TTL = 1000;

/** Cache TTL for aircraft details (1 hour) */
const AIRCRAFT_CACHE_TTL = 1000 * 60 * 60;

/** Cache TTL for airline data (5 minutes) */
const AIRLINES_CACHE_TTL = 1000 * 60 * 5;

/** HexDB API base path for route information */
const HEXDB_API_BASEPATH = 'https://hexdb.io/api/v1/';

export class FlightApiService {
  private axios: AxiosCacheInstance;
  private apiUrl: string;

  constructor() {
    const instance = Axios.create({
      withCredentials: true, // session cookie
    });
    this.axios = setupCache(instance);
    this.apiUrl = config.flightApiUrl || '';
  }

  /**
   * List flights with pagination + filtering.
   */
  async getFlights(
    limit: number = 50,
    mil?: boolean,
    page: number = 1,
    _excludeLive: boolean = false,
    filters?: FlightFilters,
  ): Promise<PaginatedFlightsResponse> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return { items: [], page: 1, page_size: limit, total: 0, total_pages: 0 };
    }

    const params = new URLSearchParams({
      page: page.toString(),
      page_size: limit.toString(),
    });
    if (mil) {
      params.append('military_only', 'true');
    }
    if (filters?.icao24) {
      params.append('icao24', filters.icao24);
    }
    if (filters?.airline) {
      params.append('airline', filters.airline);
    }
    if (filters?.q) {
      params.append('q', filters.q);
    }

    try {
      const response = await this.axios.get<PaginatedFlightsResponse>(
        `${this.apiUrl}/flights?${params}`,
        { cache: { ttl: FLIGHTS_CACHE_TTL } },
      );
      return response.data;
    } catch (error) {
      console.error('Error fetching flights:', error);
      throw error;
    }
  }

  async getFlight(flightId: string): Promise<Flight | null> {
    if (!this.apiUrl) return null;
    try {
      const response = await this.axios.get<Flight>(
        `${this.apiUrl}/flights/${encodeURIComponent(flightId)}`,
        { cache: { ttl: FLIGHTS_CACHE_TTL } },
      );
      return response.data;
    } catch (error: unknown) {
      if (Axios.isAxiosError(error) && error.response?.status === 404) return null;
      console.error(`Error fetching flight ${flightId}:`, error);
      throw error;
    }
  }

  async getAircraft(icao24: string): Promise<Aircraft | null> {
    if (!this.apiUrl) return null;
    try {
      const response = await this.axios.get<Aircraft>(
        `${this.apiUrl}/aircraft/${encodeURIComponent(icao24)}`,
        { cache: { ttl: AIRCRAFT_CACHE_TTL } },
      );
      return response.data;
    } catch (error: unknown) {
      if (Axios.isAxiosError(error) && error.response?.status === 404) {
        console.debug(`Aircraft not found: ${icao24}`);
        return null;
      }
      console.error(`Error fetching aircraft ${icao24}:`, error);
      throw error;
    }
  }

  /**
   * Historical positions for a flight (Vec<PositionRecord> from the
   * backend; not a tuple array).
   */
  async getPositions(flightId: string): Promise<PositionRecord[]> {
    if (!this.apiUrl) return [];
    const cacheConfig: CacheRequestConfig = { cache: false };
    try {
      const response = await this.axios.get<PositionRecord[]>(
        `${this.apiUrl}/flights/${encodeURIComponent(flightId)}/positions`,
        cacheConfig,
      );
      return Array.isArray(response.data) ? response.data : [];
    } catch (error) {
      console.error(`Error fetching positions for flight ${flightId}:`, error);
      throw error;
    }
  }

  /**
   * Airline search. The Rust backend has no "list all airlines"
   * endpoint; calling without a query returns an empty array.
   */
  async getAirlines(query?: string): Promise<Airline[]> {
    if (!this.apiUrl || !query?.trim()) return [];
    return this.searchAirlines(query, 50);
  }

  async getAirlineDetail(icao: string): Promise<Airline | null> {
    if (!this.apiUrl) return null;
    try {
      const response = await this.axios.get<Airline>(
        `${this.apiUrl}/airlines/${encodeURIComponent(icao)}`,
        { cache: { ttl: AIRLINES_CACHE_TTL } },
      );
      return response.data;
    } catch (error: unknown) {
      if (Axios.isAxiosError(error) && error.response?.status === 404) return null;
      console.error(`Error fetching airline ${icao}:`, error);
      throw error;
    }
  }

  async searchAirlines(query: string, limit: number = 20): Promise<Airline[]> {
    if (!this.apiUrl || !query.trim()) return [];
    try {
      const params = new URLSearchParams({ q: query, limit: limit.toString() });
      const response = await this.axios.get<Airline[]>(
        `${this.apiUrl}/airlines/search?${params}`,
        { cache: { ttl: AIRLINES_CACHE_TTL } },
      );
      return Array.isArray(response.data) ? response.data : [];
    } catch (error) {
      console.error('Error searching airlines:', error);
      return [];
    }
  }

  // ---- External (HexDB) — unchanged ---------------------------------

  async getAirportInfo(iata: string): Promise<AirportInfo | null> {
    try {
      const response = await Axios.get(`${HEXDB_API_BASEPATH}airport/iata/${iata}`);
      if (response.status >= 200 && response.status < 300 && response.data?.airport) {
        return response.data as AirportInfo;
      }
      return null;
    } catch {
      console.debug(`Airport not found for IATA code ${iata}`);
      return null;
    }
  }

  async getFlightRoute(callsign: string): Promise<string | null> {
    try {
      const response = await Axios.get(`${HEXDB_API_BASEPATH}route/iata/${callsign}`);
      if (response.status >= 200 && response.status < 300 && response.data?.route) {
        return response.data.route;
      }
      return null;
    } catch {
      console.debug(`Route not found for callsign ${callsign}`);
      return null;
    }
  }
}

let instance: FlightApiService | null = null;

export function getFlightApiService(): FlightApiService {
  if (!instance) {
    instance = new FlightApiService();
  }
  return instance;
}

export function createFlightApiService(): FlightApiService {
  return new FlightApiService();
}
