/**
 * Backend DTOs — mirror the Rust API wire shape exactly.
 *
 * All field names are snake_case (matching the JSON the Rust backend
 * emits). Timestamp fields are ISO-8601 strings; convert to Date at the
 * call site where needed.
 */

export interface Flight {
  id: string;
  icao24: string;
  callsign?: string;
  airline_icao?: string;
  is_military: boolean;
  /** ISO-8601 */
  first_contact: string;
  /** ISO-8601 */
  last_contact: string;
  duration_seconds: number;
}

export interface PaginatedFlightsResponse {
  items: Flight[];
  page: number;
  page_size: number;
  total: number;
  total_pages: number;
}

/** Historical position record from GET /flights/{id}/positions. */
export interface PositionRecord {
  icao24: string;
  lat: number;
  lon: number;
  alt_ft?: number;
  ground_speed_kt?: number;
  track_deg?: number;
  callsign?: string;
  category?: number;
  /** ISO-8601 */
  observed_at: string;
}

export interface Aircraft {
  icao24: string;
  registration?: string;
  type_code?: string;
  type_description?: string;
  operator?: string;
  designator?: string;
  source?: string;
}

export interface AirportInfo {
  country_code: string;
  region_name: string;
  iata: string;
  icao: string;
  airport: string;
  latitude: number;
  longitude: number;
}

export interface Airline {
  icao: string;
  name: string;
  country?: string;
  callsign?: string;
  iata?: string;
}

export interface FlightFilters {
  icao24?: string;
  airline?: string;
  q?: string;
}

export interface UserInfo {
  id: string;
  email: string;
  role: string;
  display_name?: string;
  is_admin: boolean;
}

export interface LoginResponse {
  user: UserInfo;
  /** ISO-8601 */
  expires_at: string;
}

export interface AdminStats {
  flight_count: number;
}

export interface ApiErrorBody {
  code: string;
  message: string;
}
