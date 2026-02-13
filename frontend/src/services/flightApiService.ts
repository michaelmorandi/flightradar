/**
 * Flight API Service
 *
 * Provides REST API access for fetching flight data, aircraft details,
 * airline information, and route information.
 *
 * For live streaming data (positions, callsigns, categories), use
 * DataIngestionService instead.
 */

import Axios from 'axios';
import { setupCache, type AxiosCacheInstance, type CacheRequestConfig } from 'axios-cache-interceptor';
import { config } from '@/config';
import type {
  Flight, Aircraft, TerrestialPosition, PaginatedFlightsResponse,
  AirportInfo, AirlinesResponse, AirlineDetail, Airline, FlightFilters
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
      withCredentials: true, // Send cookies for JWT authentication
    });
    this.axios = setupCache(instance);

    this.apiUrl = config.flightApiUrl || '';
  }

  /**
   * Get list of past flights from the database with pagination.
   *
   * @param limit Maximum number of flights to return per page
   * @param mil Filter by military status
   * @param page Page number (1-indexed)
   * @param excludeLive If true, exclude flights with last contact within 5 minutes
   * @param filters Optional filters for icao24 or airline
   */
  async getFlights(
    limit: number = 50,
    mil?: boolean,
    page: number = 1,
    excludeLive: boolean = false,
    filters?: FlightFilters
  ): Promise<PaginatedFlightsResponse> {
    if (!this.apiUrl) {
      console.warn('Flight API URL not configured');
      return { flights: [], total: 0, page: 1, pageSize: limit, totalPages: 0 };
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
    if (excludeLive) {
      params.append('exclude_live', 'true');
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
      const response = await this.axios.get(
        `${this.apiUrl}/flights?${params}`,
        cacheConfig
      );

      if (response.status >= 200 && response.status < 300) {
        const data = response.data;
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
        cacheConfig
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
        cacheConfig
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
        cacheConfig
      );

      if (response.status >= 200 && response.status < 300) {
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
   * Get all airlines with flight statistics.
   */
  async getAirlines(query?: string): Promise<AirlinesResponse> {
    if (!this.apiUrl) {
      return { airlines: [], total: 0 };
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: AIRLINES_CACHE_TTL },
    };

    const params = new URLSearchParams();
    if (query) {
      params.append('q', query);
    }

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/airlines?${params}`,
        cacheConfig
      );

      if (response.status >= 200 && response.status < 300) {
        return response.data;
      }

      return { airlines: [], total: 0 };
    } catch (error) {
      console.error('Error fetching airlines:', error);
      throw error;
    }
  }

  /**
   * Get airline details by ICAO code.
   */
  async getAirlineDetail(icaoCode: string): Promise<AirlineDetail | null> {
    if (!this.apiUrl) {
      return null;
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: AIRLINES_CACHE_TTL },
    };

    try {
      const response = await this.axios.get(
        `${this.apiUrl}/airlines/${icaoCode}`,
        cacheConfig
      );

      if (response.status >= 200 && response.status < 300) {
        return response.data;
      }

      return null;
    } catch (error: unknown) {
      if (Axios.isAxiosError(error) && error.response?.status === 404) {
        return null;
      }
      console.error(`Error fetching airline ${icaoCode}:`, error);
      throw error;
    }
  }

  /**
   * Search airline database by name or ICAO code.
   */
  async searchAirlines(query: string, limit: number = 20): Promise<Airline[]> {
    if (!this.apiUrl || !query) {
      return [];
    }

    const cacheConfig: CacheRequestConfig = {
      cache: { ttl: AIRLINES_CACHE_TTL },
    };

    try {
      const params = new URLSearchParams({ q: query, limit: limit.toString() });
      const response = await this.axios.get(
        `${this.apiUrl}/airlines/search?${params}`,
        cacheConfig
      );

      if (response.status >= 200 && response.status < 300) {
        return response.data;
      }

      return [];
    } catch (error) {
      console.error('Error searching airlines:', error);
      return [];
    }
  }

  /**
   * Get airport information by IATA code from HexDB.
   */
  async getAirportInfo(iata: string): Promise<AirportInfo | null> {
    try {
      const response = await Axios.get(`${HEXDB_API_BASEPATH}airport/iata/${iata}`);

      if (response.status >= 200 && response.status < 300 && response.data?.airport) {
        return response.data as AirportInfo;
      }

      return null;
    } catch (error) {
      console.debug(`Airport not found for IATA code ${iata}`);
      return null;
    }
  }

  /**
   * Get flight route information from HexDB.
   */
  async getFlightRoute(callsign: string): Promise<string | null> {
    try {
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
