import { FlightRadarService } from './flightRadarService';
import { Observable, of, BehaviorSubject, ReplaySubject } from 'rxjs';
import { delay, finalize } from 'rxjs/operators';

import { Aircraft, Flight, TerrestialPosition } from '@/model/backendModel';

export class FlightRadarServiceMock implements FlightRadarService {
  private pos: TerrestialPosition[] = [
    { icao: '4b1617', callsign: 'SWR756C', lat: 47.02075, lon: 7.33998, track: 45, alt: 35000 },
    { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4429, track: 90, alt: 30000 },
    { icao: '440172', callsign: 'EJU4319', lat: 46.9149, lon: 7.4962, track: 180, alt: 25000 },
    { icao: 'a1b2c3', callsign: 'BAW123', lat: 47.12075, lon: 7.45998, track: 270, alt: 20000 },
    { icao: 'd4e5f6', callsign: 'KLM456', lat: 46.95185, lon: 7.3629, track: 359, alt: 15000 },
  ];

  private wsIntervalId: number | null = null;
  private cleanupIntervalId: number | null = null;
  private mockPositions: Map<string, TerrestialPosition & { lastUpdate: number }> = new Map();

  private flightWsIntervals: Map<string, number> = new Map();
  private flightPositionSubjects: Map<string, ReplaySubject<Array<TerrestialPosition>>> = new Map();
  private positionsSubject: BehaviorSubject<Map<string, TerrestialPosition>> | null = null;
  private flightPositionsCallbacks: Map<string, Set<(positions: Array<TerrestialPosition>) => void>> = new Map();

  getFlights(numEntries: number, filter?: string): Observable<Flight[]> {
    // Generate mock flights
    const flights: Flight[] = [
      {
        id: '123456',
        icao24: '4b1617',
        cls: 'SWR756',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 3600000), // 1 hour ago
      },
      {
        id: '234567',
        icao24: '76ceef',
        cls: 'SIA346',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 7200000), // 2 hours ago
      },
      {
        id: '345678',
        icao24: '440172',
        cls: 'EJU4319',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 1800000), // 30 minutes ago
      },
      {
        id: '456789',
        icao24: 'a1b2c3',
        cls: 'BAW123',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 5400000), // 1.5 hours ago
      },
      {
        id: '567890',
        icao24: 'd4e5f6',
        cls: 'KLM456',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 900000), // 15 minutes ago
      },
    ];

    // Apply filter if specified
    if (filter === 'mil') {
      // Just return an empty list for mock data with military filter
      return of([]).pipe(delay(100));
    }

    // Limit the number of entries as requested
    return of(flights.slice(0, numEntries)).pipe(delay(100));
  }
  getFlight(id: string): Observable<Flight> {
    // Mock flights by ID
    const flightMap: Record<string, Flight> = {
      '123456': {
        id: '123456',
        icao24: '4b1617',
        cls: 'SWR756',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 3600000), // 1 hour ago
      },
      '234567': {
        id: '234567',
        icao24: '76ceef',
        cls: 'SIA346',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 7200000), // 2 hours ago
      },
      '345678': {
        id: '345678',
        icao24: '440172',
        cls: 'EJU4319',
        lstCntct: new Date(),
        firstCntct: new Date(Date.now() - 1800000), // 30 minutes ago
      },
    };

    // Return the requested flight or a default
    return of(
      flightMap[id] ||
        ({
          id: id,
          icao24: 'ABCDEF',
          cls: 'MOCK' + id,
          lstCntct: new Date(),
          firstCntct: new Date(Date.now() - 3600000),
        } as Flight),
    ).pipe(delay(100));
  }

  getAircraft(icaoHexAddr: string): Observable<Aircraft> {
    // Mock aircraft data by ICAO address
    const aircraftMap: Record<string, Aircraft> = {
      '4b1617': {
        icao24: '4b1617',
        type: 'Airbus A320-214',
        icaoType: 'A320',
        reg: 'HB-JLT',
        op: 'Swiss International Air Lines',
      },
      '76ceef': {
        icao24: '76ceef',
        type: 'Boeing 777-312/ER',
        icaoType: 'B77W',
        reg: '9V-SWL',
        op: 'Singapore Airlines',
      },
      '440172': {
        icao24: '440172',
        type: 'Airbus A319-111',
        icaoType: 'A319',
        reg: 'G-EZBD',
        op: 'easyJet UK',
      },
    };

    // Return the requested aircraft or a default
    return of(
      aircraftMap[icaoHexAddr] ||
        ({
          icao24: icaoHexAddr,
          type: 'Mock Aircraft',
          icaoType: 'MOCK',
          reg: 'REG-' + icaoHexAddr.toUpperCase(),
          op: 'Mock Airlines',
        } as Aircraft),
    ).pipe(delay(100));
  }

  getFlightRoute(callsign: string): Observable<string | null> {
    // Mock route data based on callsign
    const mockRoutes: { [key: string]: string } = {
      'SWR756C': 'ZUR-LHR',
      'SIA346': 'SIN-CDG',
      'EJU4319': 'GVA-BER',
      'BAW123': 'LHR-JFK',
      'KLM456': 'AMS-LAX',
    };
    
    return of(mockRoutes[callsign] || null).pipe(delay(100));
  }

  getPositions(flightId: string): Observable<TerrestialPosition[]> {
    // Return specific flight path or default
    return of(this.flightPaths[flightId] || this.pos).pipe(delay(100));
  }

  registerPositionsCallback(callback: (positions: Map<string, TerrestialPosition>) => void): void {
    // Simulate streaming connection with a timer that sends position updates
    if (this.wsIntervalId !== null || this.cleanupIntervalId !== null) {
      this.disconnectPositions();
    }

    // Clear existing positions
    this.mockPositions.clear();

    // Simulate initial data load
    const now = Date.now();
    this.initializeMockPositions(now);

    // Send initial full data
    callback(this.getPositionsForCallback());

    // Then send updates every second with small changes
    this.wsIntervalId = window.setInterval(() => {
      // Randomly update 1-2 positions and add a new position every ~5 seconds
      this.updateRandomPositions();

      // Every ~5 seconds, add a new flight
      if (Math.random() < 0.2) {
        this.addRandomFlight();
      }

      // Send the updated positions to the callback
      callback(this.getPositionsForCallback());
    }, 1000);

    // Create cleanup interval to remove stale positions
    this.cleanupIntervalId = window.setInterval(() => {
      const now = Date.now();
      let hasRemovedData = false;

      // Remove positions not updated in the last 30 seconds
      this.mockPositions.forEach((value, key) => {
        if (now - value.lastUpdate > 30000) {
          // 30 seconds
          this.mockPositions.delete(key);
          hasRemovedData = true;
        }
      });

      // If we removed any data, send an update
      if (hasRemovedData) {
        callback(this.getPositionsForCallback());
      }
    }, 5000); // Check every 5 seconds
  }

  connect(): EventSource {
    // Mock implementation - return a fake EventSource
    // This is just for compatibility with the interface
    return {} as EventSource;
  }

  disconnectPositions(): void {
    if (this.wsIntervalId !== null) {
      window.clearInterval(this.wsIntervalId);
      this.wsIntervalId = null;
    }

    if (this.cleanupIntervalId !== null) {
      window.clearInterval(this.cleanupIntervalId);
      this.cleanupIntervalId = null;
    }

    // Clear stored positions
    this.mockPositions.clear();

    // Complete any observables
    if (this.positionsSubject) {
      this.positionsSubject.complete();
      this.positionsSubject = null;
    }
  }

  private initializeMockPositions(timestamp: number): void {
    // Initialize with several mock flights
    this.mockPositions.set('123456', {
      icao: '4b1617',
      callsign: 'SWR756C',
      lat: 47.02075,
      lon: 7.33998,
      track: 45,
      alt: 35000,
      lastUpdate: timestamp,
    });

    this.mockPositions.set('234567', {
      icao: '76ceef',
      callsign: 'SIA346',
      lat: 46.96185,
      lon: 7.4429,
      track: 90,
      alt: 30000,
      lastUpdate: timestamp,
    });

    this.mockPositions.set('345678', {
      icao: '440172',
      callsign: 'EJU4319',
      lat: 46.9149,
      lon: 7.4962,
      track: 180,
      alt: 25000,
      lastUpdate: timestamp,
    });

    this.mockPositions.set('456789', {
      icao: 'a1b2c3',
      callsign: 'BAW123',
      lat: 47.12075,
      lon: 7.45998,
      track: 270,
      alt: 20000,
      lastUpdate: timestamp,
    });
  }

  private updateRandomPositions(): void {
    const now = Date.now();
    const entries = Array.from(this.mockPositions.entries());

    // Randomly update 1-2 positions
    const numToUpdate = Math.ceil(Math.random() * 2);

    for (let i = 0; i < numToUpdate && i < entries.length; i++) {
      const randomIndex = Math.floor(Math.random() * entries.length);
      const [id, position] = entries[randomIndex];

      // Randomly decide whether to send full or delta update (30% chance for delta update)
      const sendDeltaUpdate = Math.random() < 0.3;

      if (sendDeltaUpdate) {
        // Simulate a delta update with only lat, lon, and alt
        const delta = {
          lat: position.lat + (Math.random() * 0.01 - 0.005),
          lon: position.lon + (Math.random() * 0.01 - 0.005),
          alt: position.alt !== undefined ? position.alt + Math.floor(Math.random() * 200 - 100) : undefined,
          lastUpdate: now,
        };

        // Update with delta (preserving other fields)
        this.mockPositions.set(id, { ...position, ...delta });
      } else {
        // Update position with small changes (full update)
        this.mockPositions.set(id, {
          ...position,
          lat: position.lat + (Math.random() * 0.01 - 0.005),
          lon: position.lon + (Math.random() * 0.01 - 0.005),
          track: position.track !== undefined ? (position.track + (Math.random() * 5 - 2.5)) % 360 : Math.floor(Math.random() * 360),
          alt: position.alt !== undefined ? position.alt + Math.floor(Math.random() * 200 - 100) : undefined,
          lastUpdate: now,
        });
      }
    }
  }

  private addRandomFlight(): void {
    const now = Date.now();
    const flightId = `mock${Math.floor(Math.random() * 100000)}`;

    // Create a new random flight
    this.mockPositions.set(flightId, {
      icao: `${Math.random().toString(16).substring(2, 8)}`,
      callsign: `TEST${Math.floor(Math.random() * 1000)}`,
      lat: 46.5 + Math.random(),
      lon: 7.0 + Math.random(),
      track: Math.floor(Math.random() * 360),
      alt: Math.floor(Math.random() * 40000),
      lastUpdate: now,
    });
  }

  private getPositionsForCallback(): Map<string, TerrestialPosition> {
    // Convert the internal map to the expected format (without lastUpdate property)
    const result = new Map<string, TerrestialPosition>();

    this.mockPositions.forEach((value, key) => {
      const { lastUpdate, ...position } = value;
      result.set(key, position);
    });

    return result;
  }
  getCurrentPositions(): Map<string, TerrestialPosition> {
    // Reuse existing method
    return this.getPositionsForCallback();
  }

  getCurrentPosition(flightId: string): TerrestialPosition | null {
    if (this.mockPositions.has(flightId)) {
      const positionData = this.mockPositions.get(flightId);
      if (positionData) {
        const { lastUpdate, ...position } = positionData;
        return position;
      }
    }
    return null;
  }

  registerFlightPositionsCallback(flightId: string, callback: (positions: Array<TerrestialPosition>) => void): void {
    if (!this.flightPositionsCallbacks.has(flightId)) {
      this.flightPositionsCallbacks.set(flightId, new Set());
    }

    // eslint-disable-next-line
    const callbacks = this.flightPositionsCallbacks.get(flightId)!;

    callbacks.add(callback);

    if (this.flightWsIntervals.has(flightId)) {
      return;
    }

    this.getPositions(flightId).subscribe((positions) => {
      const callbacksSet = this.flightPositionsCallbacks.get(flightId);
      if (!callbacksSet || callbacksSet.size === 0) {
        return;
      }

      callbacksSet.forEach((cb) => cb(positions));

      const intervalId = window.setInterval(() => {
        const existingPositions = this.flightPaths[flightId] || this.pos;

        const currentCallbacks = this.flightPositionsCallbacks.get(flightId);
        if (!currentCallbacks || currentCallbacks.size === 0) {
          this.disconnectFlightPositions(flightId);
          return;
        }

        // Modify positions slightly to simulate movement
        const updatedPositions = existingPositions.map((pos) => ({
          ...pos,
          lat: pos.lat + (Math.random() * 0.002 - 0.001),
          lon: pos.lon + (Math.random() * 0.002 - 0.001),
          alt: pos.alt !== undefined ? pos.alt + Math.floor(Math.random() * 200 - 100) : undefined,
        }));

        if (Math.random() < 0.3) {
          const lastPos = updatedPositions[updatedPositions.length - 1];
          if (lastPos) {
            updatedPositions.push({
              ...lastPos,
              lat: lastPos.lat + (Math.random() * 0.01 - 0.005),
              lon: lastPos.lon + (Math.random() * 0.01 - 0.005),
              alt: lastPos.alt !== undefined ? lastPos.alt + Math.floor(Math.random() * 300 - 100) : undefined,
            });
          }
        }

        this.flightPaths[flightId] = updatedPositions;

        // Notify all callbacks
        currentCallbacks.forEach((cb) => cb(updatedPositions));
      }, 2000);

      this.flightWsIntervals.set(flightId, intervalId);
    });
  }

  /**
   * Removes a specific callback for a flight's position updates.
   * Only disconnects the connection when no callbacks remain.
   * @param flightId The flight ID
   * @param callback The callback to remove (optional). If not provided, all callbacks will be removed.
   */
  removeFlightPositionCallback(flightId: string, callback?: (positions: Array<TerrestialPosition>) => void): void {
    const callbacks = this.flightPositionsCallbacks.get(flightId);

    if (!callbacks) {
      return;
    }

    if (callback) {
      callbacks.delete(callback);
    } else {
      callbacks.clear();
    }

    if (callbacks.size === 0) {
      this.disconnectFlightPositions(flightId);
    }
  }

  disconnectFlightPositions(flightId: string): void {
    this.flightPositionsCallbacks.delete(flightId);

    const intervalId = this.flightWsIntervals.get(flightId);
    if (intervalId !== undefined) {
      window.clearInterval(intervalId);
      this.flightWsIntervals.delete(flightId);
    }

    const subject = this.flightPositionSubjects.get(flightId);
    if (subject) {
      subject.complete();
      this.flightPositionSubjects.delete(flightId);
    }
  }

  observePositions(): Observable<Map<string, TerrestialPosition>> {
    if (!this.positionsSubject) {
      this.positionsSubject = new BehaviorSubject<Map<string, TerrestialPosition>>(this.getCurrentPositions());

      this.registerPositionsCallback((positions) => {
        this.positionsSubject?.next(positions);
      });
    }

    return this.positionsSubject.asObservable();
  }

  observeFlightPositions(flightId: string): Observable<Array<TerrestialPosition>> {
    if (!this.flightPositionSubjects.has(flightId)) {
      const subject = new ReplaySubject<Array<TerrestialPosition>>(1);
      this.flightPositionSubjects.set(flightId, subject);

      const callback = (positions: Array<TerrestialPosition>) => {
        const subj = this.flightPositionSubjects.get(flightId);
        if (subj) {
          subj.next(positions);
        }
      };

      this.registerFlightPositionsCallback(flightId, callback);

      return subject.asObservable().pipe(
        finalize(() => {
          const subj = this.flightPositionSubjects.get(flightId);
          if (subj) {
            this.flightPositionSubjects.delete(flightId);
            this.removeFlightPositionCallback(flightId, callback);
          }
        }),
      );
    }

    // eslint-disable-next-line
    return this.flightPositionSubjects.get(flightId)!.asObservable();
  }

  private flightPaths: Record<string, TerrestialPosition[]> = {
    '123456': [
      { icao: '4b1617', callsign: 'SWR756C', lat: 47.02075, lon: 7.33998, track: 45, alt: 35000 },
      { icao: '4b1617', callsign: 'SWR756C', lat: 47.03075, lon: 7.34998, track: 45, alt: 35000 },
      { icao: '4b1617', callsign: 'SWR756C', lat: 47.04075, lon: 7.35998, track: 45, alt: 35000 },
      { icao: '4b1617', callsign: 'SWR756C', lat: 47.05075, lon: 7.36998, track: 45, alt: 35000 },
      { icao: '4b1617', callsign: 'SWR756C', lat: 47.06075, lon: 7.37998, track: 45, alt: 35000 },
    ],
    '234567': [
      { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4429, track: 90, alt: 30000 },
      { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4529, track: 90, alt: 30000 },
      { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4629, track: 90, alt: 30000 },
      { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4729, track: 90, alt: 30000 },
      { icao: '76ceef', callsign: 'SIA346', lat: 46.96185, lon: 7.4829, track: 90, alt: 30000 },
    ],
  };
}
