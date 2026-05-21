/**
 * Data Ingestion Service — SSE consumer for the Rust backend.
 *
 * Two streams:
 *   - GET /api/v1/live/stream                   (all aircraft)
 *   - GET /api/v1/live/stream/{icao24}          (single aircraft)
 *
 * Both emit two named events:
 *   - `snapshot` { positions: { [icao24]: LivePosition }, emitted_at }
 *   - `delta`    { changed:   { [icao24]: LivePosition },
 *                  removed:   string[],
 *                  emitted_at }
 *
 * `LivePosition` carries `callsign` and `category` inline — there are
 * no separate callsigns/categories streams any more.
 */

import type {
  PositionUpdate,
  HistoryPosition,
  RawPositionData,
  SSEDeltaMessage,
  SSESnapshotMessage,
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

  /** Subscribe to the global live-positions stream. */
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

      this.positionsEventSource.addEventListener('snapshot', (event) => {
        try {
          const data: SSESnapshotMessage = JSON.parse(event.data);
          this.applyPositionMap(data.positions, true);
        } catch (error) {
          console.error('Error processing snapshot event:', error);
        }
      });

      this.positionsEventSource.addEventListener('delta', (event) => {
        try {
          const data: SSEDeltaMessage = JSON.parse(event.data);
          this.applyPositionMap(data.changed, false);
          if (Array.isArray(data.removed) && data.removed.length > 0) {
            const aircraftStore = useAircraftStore();
            aircraftStore.removeAircraft(data.removed);
            for (const id of data.removed) this.lastUpdateTimes.delete(id);
          }
        } catch (error) {
          console.error('Error processing delta event:', error);
        }
      });

      // Keep-alive comments don't fire named events; nothing to do.

      this.startCleanupInterval();
    } catch (error) {
      console.error('Failed to connect to positions stream:', error);
      aircraftStore.setConnectionStatus('error');
    }
  }

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
   * Subscribe to per-aircraft live updates. NOTE: the Rust backend keys
   * the single-aircraft stream by ICAO24, not by internal flight id —
   * callers must pass the ICAO24 of the aircraft they want to track.
   */
  subscribeToFlight(icao24: string): void {
    if (this.flightEventSources.has(icao24)) {
      console.warn(`Already subscribed to ${icao24}`);
      return;
    }

    const url = this.getStreamUrl(`live/stream/${encodeURIComponent(icao24)}`);
    const historyStore = useFlightHistoryStore();

    historyStore.subscribe(icao24);
    historyStore.setLoading(icao24, true);

    try {
      const eventSource = new EventSource(url, { withCredentials: true });
      this.flightEventSources.set(icao24, eventSource);

      eventSource.onopen = () => {
        console.debug(`Live stream connected for ${icao24}`);
      };

      eventSource.onerror = (error) => {
        console.error(`Live stream error for ${icao24}:`, error);
        historyStore.setLoading(icao24, false);
      };

      eventSource.addEventListener('snapshot', (event) => {
        try {
          const data: SSESnapshotMessage = JSON.parse(event.data);
          const positions = Object.entries(data.positions || {});
          const histories: HistoryPosition[] = positions
            .filter(([id]) => id === icao24)
            .map(([, pos]) => toHistoryPosition(pos));
          historyStore.setHistory(icao24, histories);
          historyStore.setLoading(icao24, false);
        } catch (error) {
          console.error(`Error processing snapshot for ${icao24}:`, error);
        }
      });

      eventSource.addEventListener('delta', (event) => {
        try {
          const data: SSEDeltaMessage = JSON.parse(event.data);
          const update = data.changed?.[icao24];
          if (update) {
            historyStore.addPosition(icao24, toHistoryPosition(update));
          }
        } catch (error) {
          console.error(`Error processing delta for ${icao24}:`, error);
        }
      });
    } catch (error) {
      console.error(`Failed to subscribe to ${icao24}:`, error);
      historyStore.setLoading(icao24, false);
      historyStore.unsubscribe(icao24);
    }
  }

  unsubscribeFromFlight(icao24: string): void {
    const eventSource = this.flightEventSources.get(icao24);
    if (eventSource) {
      eventSource.close();
      this.flightEventSources.delete(icao24);
    }
    const historyStore = useFlightHistoryStore();
    historyStore.cleanupFlight(icao24, false);
  }

  disconnectAllFlights(): void {
    for (const [icao24, eventSource] of this.flightEventSources) {
      eventSource.close();
      const historyStore = useFlightHistoryStore();
      historyStore.unsubscribe(icao24);
    }
    this.flightEventSources.clear();
  }

  disconnectAll(): void {
    this.disconnect();
    this.disconnectAllFlights();
  }

  isConnected(): boolean {
    return (
      this.positionsEventSource !== null &&
      this.positionsEventSource.readyState === EventSource.OPEN
    );
  }

  isSubscribedToFlight(icao24: string): boolean {
    return this.flightEventSources.has(icao24);
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE — Data processing
  // ═══════════════════════════════════════════════════════════

  private applyPositionMap(
    positions: Record<string, RawPositionData> | undefined,
    isInitial: boolean,
  ): void {
    if (!positions) return;

    const aircraftStore = useAircraftStore();
    const now = Date.now();
    if (isInitial) this.lastUpdateTimes.clear();

    const updates = new Map<string, PositionUpdate>();
    for (const [id, rawPos] of Object.entries(positions)) {
      updates.set(id, parsePositionData(id, rawPos));
      this.lastUpdateTimes.set(id, now);
    }
    aircraftStore.updatePositions(updates, isInitial);
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE — Cleanup
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
    for (const [id, lastUpdate] of this.lastUpdateTimes) {
      if (now - lastUpdate > STALE_TIMEOUT_MS) staleIds.push(id);
    }
    for (const id of staleIds) this.lastUpdateTimes.delete(id);
    const aircraftStore = useAircraftStore();
    aircraftStore.purgeStaleAircraft();
  }

  // ═══════════════════════════════════════════════════════════
  // PRIVATE — Utilities
  // ═══════════════════════════════════════════════════════════

  private getStreamUrl(path: string): string {
    if (!this.apiUrl) {
      throw new Error('Flight API URL not configured');
    }
    const baseUrl = this.apiUrl.endsWith('/') ? this.apiUrl : `${this.apiUrl}/`;
    return `${baseUrl}${path}`;
  }
}

function toHistoryPosition(raw: RawPositionData): HistoryPosition {
  return {
    lat: raw.lat,
    lon: raw.lon,
    altitude: raw.alt_ft,
    groundSpeed: raw.ground_speed_kt,
    track: raw.track_deg,
    timestamp: raw.updated_at ? Date.parse(raw.updated_at) : Date.now(),
  };
}

let instance: DataIngestionService | null = null;

export function getDataIngestionService(): DataIngestionService {
  if (!instance) {
    instance = new DataIngestionService();
  }
  return instance;
}

export function createDataIngestionService(): DataIngestionService {
  return new DataIngestionService();
}
