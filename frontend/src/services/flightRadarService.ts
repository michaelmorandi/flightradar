import { Flight, Aircraft, TerrestialPosition } from '@/model/backendModel';
import { Observable } from 'rxjs';

export interface FlightRadarService {
  getFlights(numEntries: number, filter?: string): Observable<Array<Flight>>;

  getFlight(id: string): Observable<Flight>;

  getAircraft(icaoHexAddr: string): Observable<Aircraft>;

  // Methods for live position data
  observePositions(): Observable<Map<string, TerrestialPosition>>;
  disconnectPositions(): void;

  observeFlightPositions(flightId: string): Observable<Array<TerrestialPosition>>;
  disconnectFlightPositions(flightId: string): void;

  // Callback methods
  registerPositionsCallback(callback: (positions: Map<string, TerrestialPosition>) => void): void;
  registerFlightPositionsCallback(flightId: string, callback: (positions: Array<TerrestialPosition>) => void): void;
  removeFlightPositionCallback(flightId: string, callback?: (positions: Array<TerrestialPosition>) => void): void;

  getCurrentPositions(): Map<string, TerrestialPosition>;

  getCurrentPosition(flightId: string): TerrestialPosition | null;

  getPositions(flightId: string): Observable<Array<TerrestialPosition>>;

  getFlightRoute(callsign: string): Observable<string | null>;

  // Connection method
  connect(): EventSource;
}
