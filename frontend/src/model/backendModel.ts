export interface Flight {
  id: string;
  icao24: string;
  cls: string;
  firstCntct: Date;
  lstCntct: Date;
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
