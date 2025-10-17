import Axios, { AxiosResponse, AxiosRequestConfig } from 'axios';
import { CacheRequestConfig, setupCache, AxiosCacheInstance } from 'axios-cache-interceptor';
import { Flight, Aircraft, TerrestialPosition } from '@/model/backendModel';
import { config } from '@/config';
import { FlightRadarService } from './flightRadarService';
import { Observable, from, BehaviorSubject, ReplaySubject, of } from 'rxjs';
import { map, catchError, finalize } from 'rxjs/operators';


export class FlightRadarServiceImpl implements FlightRadarService {
  private static readonly HEXDB_API_BASEPATH = 'https://hexdb.io/api/v1/';
  private authConfig: AxiosRequestConfig;
  private apiBasepath: string;
  private axios: AxiosCacheInstance;
  private noCacheConfig: CacheRequestConfig;
  private oneSecondCacheConfig: CacheRequestConfig;
  private oneHourCacheConfig: CacheRequestConfig;
  private positionsEventSource: EventSource | null = null;
  private positionsData: Map<string, TerrestialPosition & { lastUpdate: number }> = new Map();
  private positionsCleanupInterval: number | null = null;

  private flightPositionsEventSources: Map<string, EventSource> = new Map();
  private flightPositionSubjects: Map<string, ReplaySubject<Array<TerrestialPosition>>> = new Map();
  private positionsSubject: BehaviorSubject<Map<string, TerrestialPosition>> | null = null;
  private flightPositionsCallbacks: Map<string, Set<(positions: Array<TerrestialPosition>) => void>> = new Map();

  constructor() {
    const instance = Axios.create();
    this.axios = setupCache(instance);

    this.authConfig = config.flightApiUser
      ? ({
          auth: {
            username: config.flightApiUser,
            password: config.flightApiPassword,
          },
        } as AxiosRequestConfig)
      : {};

    this.noCacheConfig = { cache: false };

    this.oneSecondCacheConfig = {
      cache: {
        ttl: 1000,
      },
    };

    this.oneHourCacheConfig = {
      cache: {
        ttl: 1000 * 60 * 60,
      },
    };

    this.apiBasepath = config.flightApiUrl;
  }

  public getFlights(numEntries: number, filter?: string): Observable<Array<Flight>> {
    const urlWithParams: string =
      filter == null ? `${this.apiBasepath}/flights?limit=${numEntries}` : `${this.apiBasepath}/flights?limit=${numEntries}&filter=${filter}`;

    return from(this.axios.get(urlWithParams, { ...this.oneSecondCacheConfig, ...this.authConfig })).pipe(
      map((res) => {
        if (this.is2xx(res)) return res.data;
        else throw new Error(res.statusText || 'Error retrieving flights');
      }),
      catchError((err) => {
        console.error('Error retrieving flights:', err);
        throw err;
      }),
    );
  }

  public getFlight(id: string): Observable<Flight> {
    return from(this.axios.get(`${this.apiBasepath}/flights/${id}`, { ...this.oneSecondCacheConfig, ...this.authConfig })).pipe(
      map((res) => {
        if (this.is2xx(res)) return res.data;
        else throw new Error(res.statusText || 'Error retrieving flight details');
      }),
      catchError((err) => {
        console.error('Error retrieving flight details:', err);
        throw err;
      }),
    );
  }

  public getAircraft(icaoHexAddr: string): Observable<Aircraft> {
    return from(this.axios.get(`${this.apiBasepath}/aircraft/${icaoHexAddr}`, { ...this.oneHourCacheConfig, ...this.authConfig })).pipe(
      map((res) => {
        if (this.is2xx(res)) return res.data;
        else throw new Error(res.statusText || 'Error retrieving aircraft details');
      }),
      catchError((err) => {
        if (err.response && err.response.status === 404) {
          console.warn('Aircraft not found:', icaoHexAddr);
          return of({} as Aircraft);
        } else {
          console.error('Error retrieving aircraft details:', err);
          throw err;
        }
      }),
    );
  }

  public getFlightRoute(callsign: string): Observable<string | null> {
    // TODO: Use plain axios cache interceptor (currently there's an issue with CORS and caching headers)
    const plainAxios = Axios.create();
    
    return from(plainAxios.get(`${FlightRadarServiceImpl.HEXDB_API_BASEPATH}route/iata/${callsign}`)).pipe(
      map((res) => {
        if (this.is2xx(res) && res.data && res.data.route) {
          return res.data.route;
        }
        return null;
      }),
      catchError((err) => {
        console.error('Error retrieving flight route:', err);
        return of(null);
      }),
    );
  }

  public getCurrentPositions(): Map<string, TerrestialPosition> {
    const result = new Map<string, TerrestialPosition>();
    this.positionsData.forEach((value, key) => {
      const { lastUpdate, ...position } = value;
      result.set(key, position);
    });
    return result;
  }

  public getCurrentPosition(flightId: string): TerrestialPosition | null {
    if (this.positionsData.has(flightId)) {
      const positionData = this.positionsData.get(flightId);
      if (positionData) {
        const { lastUpdate, ...position } = positionData;
        return position;
      }
    }
    return null;
  }

  public observePositions(): Observable<Map<string, TerrestialPosition>> {
    // Ensure connection is established
    if (!this.positionsEventSource) {
      this.connect();
    }

    // If subject doesn't exist, create one
    if (!this.positionsSubject) {
      this.positionsSubject = new BehaviorSubject<Map<string, TerrestialPosition>>(this.getCurrentPositions());

      // Register callback that will update the subject
      this.registerPositionsCallback((positions) => {
        this.positionsSubject?.next(positions);
      });
    }

    return this.positionsSubject.asObservable();
  }

  public observeFlightPositions(flightId: string): Observable<Array<TerrestialPosition>> {
    if (!this.flightPositionSubjects.has(flightId)) {
      const subject = new ReplaySubject<Array<TerrestialPosition>>(1);
      this.flightPositionSubjects.set(flightId, subject);

      this.registerFlightPositionsCallback(flightId, (positions) => {
        const subj = this.flightPositionSubjects.get(flightId);
        if (subj) {
          subj.next(positions);
        }
      });

      // When all subscribers unsubscribe, disconnect the EventSource
      return subject.asObservable().pipe(
        finalize(() => {
          const subj = this.flightPositionSubjects.get(flightId);
          if (subj) {
            this.flightPositionSubjects.delete(flightId);
            this.disconnectFlightPositions(flightId);
          }
        }),
      );
    }

    // eslint-disable-next-line
    return this.flightPositionSubjects.get(flightId)!.asObservable();
  }

  public connect(): EventSource {
    const url = this.getStreamUrl('positions/live/stream');

    this.positionsEventSource = new EventSource(url);

    this.positionsEventSource.onopen = () => {
      console.debug('Position stream connection established');
    };

    this.positionsEventSource.onerror = (error) => {
      this.handleConnectionFailure(error);
    };

    this.positionsEventSource.addEventListener('positions', (event) => {
      try {
        const data = JSON.parse(event.data);
        this.processPositionsData(data);
      } catch (error) {
        console.error('Error processing positions message:', error);
      }
    });

    this.positionsEventSource.addEventListener('heartbeat', (event) => {
      console.debug('Received heartbeat:', event.data);
    });

    return this.positionsEventSource;
  }

  public registerPositionsCallback(callback: (positions: Map<string, TerrestialPosition>) => void): void {
    if (!this.positionsEventSource) {
      console.warn('Connection not established, attempting to connect...');
      this.connect();
      
      // Give the connection a moment to establish
      setTimeout(() => {
        if (!this.positionsEventSource) {
          throw new Error('Connection failed');
        }
        this.setupPositionsCallback(callback);
      }, 1000);
      return;
    }

    this.setupPositionsCallback(callback);
  }

  private setupPositionsCallback(callback: (positions: Map<string, TerrestialPosition>) => void): void {
    if (!this.positionsEventSource) {
      throw new Error('Connection not established');
    }

    // Store callback for processing data
    this.positionsCallback = callback;

    // Start cleanup interval to remove stale data
    if (this.positionsCleanupInterval === null) {
      this.positionsCleanupInterval = window.setInterval(() => {
        const now = Date.now();
        let hasRemovedData = false;

        // Remove positions not updated in the last 30 seconds
        this.positionsData.forEach((value, key) => {
          if (now - value.lastUpdate > 30000) {
            this.positionsData.delete(key);
            hasRemovedData = true;
          }
        });

        if (hasRemovedData && this.positionsCallback) {
          const result = new Map<string, TerrestialPosition>();
          this.positionsData.forEach((value, key) => {
            const { lastUpdate: _, ...position } = value;
            result.set(key, position);
          });
          this.positionsCallback(result);
        }
      }, 5000); // Check every 5 seconds
    }
  }

  private positionsCallback: ((positions: Map<string, TerrestialPosition>) => void) | null = null;

  private processPositionsData(data: any): void {
    if (!this.positionsCallback) return;
    
    const now = Date.now();

    if (data.positions) {
      // For 'initial' type, replace all position data
      if (data.type === 'initial') {
        this.positionsData.clear();

        // Process and store each position with timestamp
        Object.entries<TerrestialPosition>(data.positions).forEach(([id, pos]) => {
          const position = pos;
          if (position.track !== undefined) {
            position.track = Math.round(position.track);
          }
          this.positionsData.set(id, { ...position, lastUpdate: now });
        });
      }
      // For 'update' type, only update the specified positions
      else if (data.type === 'update') {
        Object.entries<Partial<TerrestialPosition>>(data.positions).forEach(([id, pos]) => {
          const deltaPosition = pos;

          // Handle both full and delta updates
          if (this.positionsData.has(id)) {
            // Get existing data and merge with new data (delta update)
            const existingData = this.positionsData.get(id);
            if (!existingData) {
              return;
            }
            const { lastUpdate, ...existingPosition } = existingData;

            // Merge the existing position with the delta update
            const updatedPosition = {
              ...existingPosition,
              ...deltaPosition,
            } as TerrestialPosition;

            // Round track if it exists
            if (updatedPosition.track !== undefined) {
              updatedPosition.track = Math.round(updatedPosition.track);
            }

            this.positionsData.set(id, { ...updatedPosition, lastUpdate: now });
          } else {
            // New position that wasn't in our data before
            const position = pos as TerrestialPosition;
            if (position.track !== undefined) {
              position.track = Math.round(position.track);
            }
            this.positionsData.set(id, { ...position, lastUpdate: now });
          }
        });
      }

      const result = new Map<string, TerrestialPosition>();
      this.positionsData.forEach((value, key) => {
        const { lastUpdate: _, ...position } = value;
        result.set(key, position);
      });

      this.positionsCallback(result);
    }
  }

  public disconnectPositions(): void {
    if (this.positionsEventSource) {
      this.positionsEventSource.close();
      this.positionsEventSource = null;
    }

    if (this.positionsCleanupInterval !== null) {
      window.clearInterval(this.positionsCleanupInterval);
      this.positionsCleanupInterval = null;
    }

    this.positionsData.clear();
    this.positionsCallback = null;

    if (this.positionsSubject) {
      this.positionsSubject.complete();
      this.positionsSubject = null;
    }
  }

  public getPositions(flightid: string): Observable<Array<TerrestialPosition>> {
    return from(this.axios.get(`${this.apiBasepath}/flights/${flightid}/positions`, { ...this.noCacheConfig, ...this.authConfig })).pipe(
      map((res) => {
        if (this.is2xx(res)) {
          return res.data.map((arr: Array<number>) => {
            return {
              lat: arr[0],
              lon: arr[1],
              alt: arr[2],
            } as TerrestialPosition;
          });
        } else {
          throw new Error(res.statusText || 'Error retrieving flight positions');
        }
      }),
      catchError((err) => {
        console.error('Error retrieving flight positions:', err);
        throw err;
      }),
    );
  }

  public registerFlightPositionsCallback(flightId: string, callback: (positions: Array<TerrestialPosition>) => void): void {
    if (!this.flightPositionsCallbacks.has(flightId)) {
      this.flightPositionsCallbacks.set(flightId, new Set());
    }

    // eslint-disable-next-line
    const callbacks = this.flightPositionsCallbacks.get(flightId)!;

    callbacks.add(callback);

    if (this.flightPositionsEventSources.has(flightId)) {
      return;
    }

    const url = this.getStreamUrl(`flights/${flightId}/positions/stream`);

    try {
      const eventSource = new EventSource(url);
      this.flightPositionsEventSources.set(flightId, eventSource);

      const flightPositions: TerrestialPosition[] = [];

      eventSource.onopen = () => {
        console.debug(`Position stream connection established for flight ${flightId}`);
      };

      eventSource.addEventListener('flight_position', (event) => {
        try {
          const data = JSON.parse(event.data);
          const callbacksSet = this.flightPositionsCallbacks.get(flightId);

          if (!callbacksSet || callbacksSet.size === 0) {
            this.disconnectFlightPositions(flightId);
            return;
          }

          if (data.type === 'initial' && data.positions && data.positions[flightId]) {
            const positions = data.positions[flightId];
            if (Array.isArray(positions)) {
              flightPositions.length = 0;
              positions.forEach((pos: TerrestialPosition) => {
                flightPositions.push(pos);
              });

              callbacksSet.forEach((cb) => cb([...flightPositions]));
            }
          } else if (data.type === 'update') {
            // Backend sends position data directly in the update message, not nested in positions
            const newPosition: Partial<TerrestialPosition> = {
              lat: data.lat,
              lon: data.lon,
              alt: data.alt
            };
            if (data.gs !== undefined) {
              newPosition.gs = data.gs;
            }
            if (data.track !== undefined) {
              newPosition.track = data.track;
            }

            if (newPosition.lat !== undefined && newPosition.lon !== undefined) {
              flightPositions.push(newPosition as TerrestialPosition);
              callbacksSet.forEach((cb) => cb([...flightPositions]));
            }
          }

        } catch (error) {
          console.error(`Error processing message for flight ${flightId}:`, error);
        }
      });

      eventSource.addEventListener('heartbeat', (event) => {
        console.debug(`Received heartbeat for flight ${flightId}:`, event.data);
      });

      eventSource.onerror = (error) => {
        console.error(`Connection error for flight ${flightId}:`, error);
        this.handleConnectionFailure(error);
      };

    } catch (error) {
      console.error(`Error setting up connection for flight ${flightId}:`, error);
      this.handleConnectionFailure(error);
    }
  }

  /**
   * Removes a specific callback for a flight's position updates.
   * Only disconnects the EventSource when no callbacks remain.
   * @param flightId The flight ID
   * @param callback The callback to remove (optional). If not provided, all callbacks will be removed.
   */
  public removeFlightPositionCallback(flightId: string, callback?: (positions: Array<TerrestialPosition>) => void): void {
    const callbacks = this.flightPositionsCallbacks.get(flightId);

    if (!callbacks) {
      return; // No callbacks registered for this flight
    }

    if (callback) {
      // Remove specific callback
      callbacks.delete(callback);
    } else {
      // Remove all callbacks
      callbacks.clear();
    }

    // If no callbacks remain, disconnect the EventSource
    if (callbacks.size === 0) {
      this.disconnectFlightPositions(flightId);
    }
  }

  /**
   * Disconnect the EventSource for a specific flight.
   * This is an internal method that should only be called when there are no more callbacks
   * or when we need to force the connection to close.
   */
  public disconnectFlightPositions(flightId: string): void {
    // Remove all callbacks
    this.flightPositionsCallbacks.delete(flightId);

    const eventSource = this.flightPositionsEventSources.get(flightId);
    if (eventSource) {
      try {
        eventSource.close();
      } catch (error) {
        console.error(`Error closing EventSource for flight ${flightId}:`, error);
      }
      this.flightPositionsEventSources.delete(flightId);
    }

    // Complete and remove any subject for this flight
    const subject = this.flightPositionSubjects.get(flightId);
    if (subject) {
      subject.complete();
      this.flightPositionSubjects.delete(flightId);
    }
  }

  private handleConnectionFailure(error: unknown): void {
    console.error('Connection error:', error);
    console.warn('Connection failed. Please ensure your backend supports streaming connections.');
  }

  private is2xx(result: AxiosResponse): boolean {
    return result.status >= 200 && result.status < 300;
  }

  private getStreamUrl(path: string): string {
    // Get the API URL
    const apiBaseUrl = this.apiBasepath;

    // If apiBaseUrl is not defined properly, log an error
    if (!apiBaseUrl) {
      throw new Error('Flight API URL not defined, connection not established');
    }

    // Ensure apiBaseUrl ends with a slash for proper URL construction
    const baseUrl = apiBaseUrl.endsWith('/') ? apiBaseUrl : `${apiBaseUrl}/`;

    // Construct the final URL
    const finalUrl = `${baseUrl}${path}`;

    return finalUrl;
  }
}