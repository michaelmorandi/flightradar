export interface Flight {
  id: string;
  icao24: string;
  cls: string;
  firstCntct: Date;
  lstCntct: Date;
  positionCount?: number;
}

export interface PaginatedFlightsResponse {
  flights: Flight[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

export interface TerrestialPosition {
  icao: string;
  callsign: string;
  lat: number;
  lon: number;
  alt?: number;
  track?: number;
  gs?: number;
  cat?: number;  // Aircraft category (compact numeric encoding from AircraftCategory enum)
}

export interface Aircraft {
  icao24: string;
  type?: string;
  icaoType?: string;
  reg?: string;
  op?: string;
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
