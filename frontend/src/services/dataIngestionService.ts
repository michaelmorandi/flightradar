/**
 * Data Ingestion Service
 *
 * Manages SSE connections for live aircraft data.
 * Parses incoming data and updates the aircraft stores.
 */

import type {
  PositionUpdate,
  HistoryPosition,
  SSEPositionsMessage,
  SSECallsignsMessage,
  SSECategoriesMessage,
  SSEFlightPositionMessage,
  RawPositionData,
} from '@/stores/aircraft';
import { useAircraftStore, useFlightHistoryStore, parsePositionData } from '@/stores/aircraft';
import { config } from '@/config';

/** Cleanup interval for stale aircraft (5 seconds) */
const CLEANUP_INTERVAL_MS = 5000;

/** Stale data timeout (30 seconds) - after this, positions are removed */
const STALE_TIMEOUT_MS = 30000;

export class DataIngestionService {
  private positionsEventSource: EventSource | null = null;
  private flightEventSources: Map<string, EventSource> = new Map();
  private cleanupInterval: number | null = null;
  private apiUrl: string;

  // Track last update times for stale detection
  private lastUpdateTimes: Map<string, number> = new Map();

  constructor() {
    this.apiUrl = config.flightApiUrl || '';
  }

  /**
   * Connect to the main positions SSE stream.
   * This is the primary data source for live aircraft positions.
   */
  connect(): void {
    if (this.positionsEventSource) {
      console.warn('Already connected to positions stream');
      return;
    }

    if (!this.apiUrl) {
      console.error('Flight API URL not configured');
      return;
    }

    const url = this.getStreamUrl('live/stream');
    const aircraftStore = useAircraftStore();

    aircraftStore.setConnectionStatus('connecting');

    try {
      this.positionsEventSource = new EventSource(url, { withCredentials: true });

      this.positionsEventSource.onopen = () => {
        console.debug('Position stream connection established');
        aircraftStore.setConnectionStatus('connected');
      };

      this.positionsEventSource.onerror = (error) => {
        console.error('Position stream connection error:', error);
        aircraftStore.setConnectionStatus('error');
      };

      // Handle position updates
      this.positionsEventSource.addEventListener('positions', (event) => {
        try {
          const data: SSEPositionsMessage = JSON.parse(event.data);
          this.processPositionsData(data);
        } catch (error) {
          console.error('Error processing positions message:', error);
        }
      });

      // Handle callsign updates
      this.positionsEventSource.addEventListener('callsigns', (event) => {
        try {
          const data: SSECallsignsMessage = JSON.parse(event.data);
          this.processCallsignsData(data);
        } catch (error) {
          console.error('Error processing callsigns message:', error);
        }
      });

      // Handle category updates
      this.positionsEventSource.addEventListener('categories', (event) => {
        try {
          const data: SSECategoriesMessage = JSON.parse(event.data);
          this.processCategoriesData(data);
        } catch (error) {
          console.error('Error processing categories message:', error);
        }
      });

      // Handle heartbeat (keep-alive)
      this.positionsEventSource.addEventListener('heartbeat', (event) => {
        console.debug('Received heartbeat:', event.data);
      });

      // Start cleanup interval
      this.startCleanupInterval();

    } catch (error) {
      console.error('Failed to connect to positions stream:', error);
      aircraftStore.setConnectionStatus('error');
    }
  }

  /**
   * Disconnect from the main positions SSE stream.
   */
  disconnect(): void {
    if (this.positionsEventSource) {
      this.positionsEventSource.close();
      this.positionsEventSource = null;
    }

    this.stopCleanupInterval();
    this.lastUpdateTimes.clear();

    const aircraftStore = useAircraftStore();
    aircraftStore.setConnectionStatus('disconnected');
  }

  /**
   * Subscribe to flight position updates (for path rendering).
   */
  subscribeToFlight(flightId: string): void {
    if (this.flightEventSources.has(flightId)) {
      console.warn(`Already subscribed to flight ${flightId}`);
      return;
    }

    const url = this.getStreamUrl(`flights/${flightId}/positions/stream`);
    const historyStore = useFlightHistoryStore();

    historyStore.subscribe(flightId);
    historyStore.setLoading(flightId, true);

    try {
      const eventSource = new EventSource(url, { withCredentials: true });
      this.flightEventSources.set(flightId, eventSource);

      eventSource.onopen = () => {
        console.debug(`Flight position stream connected for ${flightId}`);
      };

      eventSource.onerror = (error) => {
        console.error(`Flight position stream error for ${flightId}:`, error);
        historyStore.setLoading(flightId, false);
      };

      // Handle flight position updates
      eventSource.addEventListener('flight_position', (event) => {
        try {
          const data: SSEFlightPositionMessage = JSON.parse(event.data);
          this.processFlightPositionData(flightId, data);
        } catch (error) {
          console.error(`Error processing flight position for ${flightId}:`, error);
        }
      });

      // Handle heartbeat
      eventSource.addEventListener('heartbeat', (event) => {
        console.debug(`Received heartbeat for flight ${flightId}:`, event.data);
      });

    } catch (error) {
      console.error(`Failed to subscribe to flight ${flightId}:`, error);
      historyStore.setLoading(flightId, false);
      historyStore.unsubscribe(flightId);
    }
  }

  /**
   * Unsubscribe from flight position updates.
   */
  unsubscribeFromFlight(flightId: string): void {
    const eventSource = this.flightEventSources.get(flightId);
    if (eventSource) {
      eventSource.close();
      this.flightEventSources.delete(flightId);
    }

    const historyStore = useFlightHistoryStore();
    historyStore.cleanupFlight(flightId, false); // Keep history for display
  }

  /**
   * Disconnect all flight subscriptions.
   */
  disconnectAllFlights(): void {
    for (const [flightId, eventSource] of this.flightEventSources) {
      eventSource.close();
      const historyStore = useFlightHistoryStore();
      historyStore.unsubscribe(flightId);
    }
    this.flightEventSources.clear();
  }

  /**
   * Disconnect everything.
   */
  disconnectAll(): void {
    this.disconnect();
    this.disconnectAllFlights();
  }

  /**
   * Check if connected to main positions stream.
   */
  isConnected(): boolean {
    return this.positionsEventSource !== null &&
           this.positionsEventSource.readyState === EventSource.OPEN;
  }

  /**
   * Check if subscribed to a specific flight.
   */
  isSubscribedToFlight(flightId: string): boolean {
    return this.flightEventSources.has(flightId);
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE - Data Processing
  // ═══════════════════════════════════════════════════════════

  private processPositionsData(data: SSEPositionsMessage): void {
    if (!data.positions) return;

    const aircraftStore = useAircraftStore();
    const now = Date.now();
    const isInitial = data.type === 'initial';

    // Clear last update times for initial load
    if (isInitial) {
      this.lastUpdateTimes.clear();
    }

    // Parse and collect position updates
    const updates = new Map<string, PositionUpdate>();

    for (const [id, rawPos] of Object.entries(data.positions)) {
      const update = parsePositionData(id, rawPos as RawPositionData);
      updates.set(id, update);
      this.lastUpdateTimes.set(id, now);
    }

    // Update the store
    aircraftStore.updatePositions(updates, isInitial);
  }

  private processCallsignsData(data: SSECallsignsMessage): void {
    if (!data.callsigns) return;

    const aircraftStore = useAircraftStore();
    const callsigns = new Map<string, string>(Object.entries(data.callsigns));
    aircraftStore.updateCallsigns(callsigns);
  }

  private processCategoriesData(data: SSECategoriesMessage): void {
    if (!data.categories) return;

    const aircraftStore = useAircraftStore();
    const categories = new Map<string, number>(Object.entries(data.categories));
    aircraftStore.updateCategories(categories);
  }

  private processFlightPositionData(flightId: string, data: SSEFlightPositionMessage): void {
    const historyStore = useFlightHistoryStore();
    const now = Date.now();

    if (data.type === 'initial') {
      // Initial load - set complete history
      const flightPositions = data.positions?.[flightId];
      if (Array.isArray(flightPositions)) {
        const positions: HistoryPosition[] = flightPositions.map((pos, index) => ({
          lat: pos.lat,
          lon: pos.lon,
          altitude: pos.alt,
          groundSpeed: pos.gs,
          track: pos.track,
          timestamp: now - (flightPositions.length - index) * 1000, // Estimate timestamps
        }));
        historyStore.setHistory(flightId, positions);
      }
      historyStore.setLoading(flightId, false);
    } else if (data.type === 'update') {
      // Incremental update - add single position
      if (data.lat !== undefined && data.lon !== undefined) {
        const position: HistoryPosition = {
          lat: data.lat,
          lon: data.lon,
          altitude: data.alt,
          groundSpeed: data.gs,
          track: data.track,
          timestamp: now,
        };
        historyStore.addPosition(flightId, position);
      }
    }
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE - Cleanup
  // ═══════════════════════════════════════════════════════════

  private startCleanupInterval(): void {
    if (this.cleanupInterval !== null) return;

    this.cleanupInterval = window.setInterval(() => {
      this.cleanupStaleData();
    }, CLEANUP_INTERVAL_MS);
  }

  private stopCleanupInterval(): void {
    if (this.cleanupInterval !== null) {
      window.clearInterval(this.cleanupInterval);
      this.cleanupInterval = null;
    }
  }

  private cleanupStaleData(): void {
    const now = Date.now();
    const staleIds: string[] = [];

    // Find stale entries
    for (const [id, lastUpdate] of this.lastUpdateTimes) {
      if (now - lastUpdate > STALE_TIMEOUT_MS) {
        staleIds.push(id);
      }
    }

    // Remove from tracking
    for (const id of staleIds) {
      this.lastUpdateTimes.delete(id);
    }

    // Let the store handle actual cleanup based on its own stale threshold
    const aircraftStore = useAircraftStore();
    aircraftStore.purgeStaleAircraft();
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE - Utilities
  // ═══════════════════════════════════════════════════════════

  private getStreamUrl(path: string): string {
    if (!this.apiUrl) {
      throw new Error('Flight API URL not configured');
    }

    const baseUrl = this.apiUrl.endsWith('/') ? this.apiUrl : `${this.apiUrl}/`;
    return `${baseUrl}${path}`;
  }
}

// Singleton instance
let instance: DataIngestionService | null = null;

/**
 * Get the DataIngestionService singleton instance.
 */
export function getDataIngestionService(): DataIngestionService {
  if (!instance) {
    instance = new DataIngestionService();
  }
  return instance;
}

/**
 * Create a new DataIngestionService instance.
 * Use this for testing or when you need a fresh instance.
 */
export function createDataIngestionService(): DataIngestionService {
  return new DataIngestionService();
}
