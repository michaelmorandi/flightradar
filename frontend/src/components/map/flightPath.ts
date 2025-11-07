import { TerrestialPosition } from '@/model/backendModel';

// Here Maps API interfaces
declare let H: {
  geo: {
    LineString: new () => HereLineString;
  };
  map: {
    Polyline: new (lineString: HereLineString, options: PolylineOptions) => HerePolyline;
    Object: {
      remove(): void;
    };
  };
};

interface HereLineString {
  pushPoint(point: { lat: number; lng: number }): void;
  getPointCount(): number;
}

interface PolylineOptions {
  style: {
    lineWidth: number;
    strokeColor: string;
    lineDash?: number[];
  };
}

interface HerePolyline {
  getGeometry(): HereLineString;
  setGeometry(lineString: HereLineString): void;
}

export interface HereCoordinates {
  lat: number;
  lng: number;
  heading: number;
}

export class FlightPath {
  private polyLine: HerePolyline | null = null;
  private pathSegments: HerePolyline[] = [];
  private lastPositionCount = 0;
  private readonly LARGE_GAP_THRESHOLD_NM = 5; 
  private readonly MEDIUM_GRAY_COLOR = '#505050'; 
  private readonly LIGHT_GRAY_COLOR = '#c0c0c0'; 

  constructor(
    private flightIdent: string,
    private map: { addObject: (obj: HerePolyline) => HerePolyline; removeObject: (obj: HerePolyline) => void },
  ) {}

  public get flightId(): string {
    return this.flightIdent;
  }

  /**
   * Fast approximation for distance between two points in nautical miles
   * Uses Euclidean distance with latitude correction - good for short distances
   * @param lat1 Latitude of first point in degrees
   * @param lon1 Longitude of first point in degrees
   * @param lat2 Latitude of second point in degrees
   * @param lon2 Longitude of second point in degrees
   * @returns Distance in nautical miles
   */
  private calculateDistanceApprox(lat1: number, lon1: number, lat2: number, lon2: number): number {
    const dLat = lat2 - lat1;
    const dLon = (lon2 - lon1) * Math.cos((lat1 + lat2) * Math.PI / 360);
    const distanceDegrees = Math.sqrt(dLat * dLat + dLon * dLon);
    return distanceDegrees * 60; // Convert degrees to nautical miles (1 degree ≈ 60 NM)
  }

  /**
   * Calculate the great-circle distance between two points in nautical miles
   * using the Haversine formula
   * @param lat1 Latitude of first point in degrees
   * @param lon1 Longitude of first point in degrees
   * @param lat2 Latitude of second point in degrees
   * @param lon2 Longitude of second point in degrees
   * @returns Distance in nautical miles
   */
  public static calculateDistanceNM(lat1: number, lon1: number, lat2: number, lon2: number): number {
    
    const lat1Rad = (lat1 * Math.PI) / 180;
    const lon1Rad = (lon1 * Math.PI) / 180;
    const lat2Rad = (lat2 * Math.PI) / 180;
    const lon2Rad = (lon2 * Math.PI) / 180;

    // Haversine formula
    const dLat = lat2Rad - lat1Rad;
    const dLon = lon2Rad - lon1Rad;
    const a = Math.sin(dLat / 2) * Math.sin(dLat / 2) + Math.cos(lat1Rad) * Math.cos(lat2Rad) * Math.sin(dLon / 2) * Math.sin(dLon / 2);
    const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

    // Distance in nautical miles
    return 3440.065 * c;
  }

  /**
   * Get color based on altitude using ground-to-sky gradient
   * Ground Level (0-5,000 ft): Crimson Red
   * Low Altitude (5,000-20,000 ft): Forest Green  
   * Medium Altitude (20,000-35,000 ft): Sky Blue
   * High Altitude (35,000-45,000 ft): Royal Blue
   * Very High Altitude (45,000+ ft): Blue Violet
   * @param altitude Altitude in feet
   * @returns Color in hex format
   */
  private getColorByAltitude(altitude = 0): string {
    
    const clampedAlt = Math.max(0, altitude);
    
    const colorStops = [
      { alt: 0,     color: { r: 220, g: 20,  b: 60  } }, // #DC143C Crimson Red
      { alt: 5000,  color: { r: 34,  g: 139, b: 34  } }, // #228B22 Forest Green
      { alt: 20000, color: { r: 135, g: 206, b: 235 } }, // #87CEEB Sky Blue
      { alt: 35000, color: { r: 65,  g: 105, b: 225 } }, // #4169E1 Royal Blue
      { alt: 45000, color: { r: 138, g: 43,  b: 226 } }  // #8A2BE2 Blue Violet
    ];

    for (let i = 0; i < colorStops.length - 1; i++) {
      const lower = colorStops[i];
      const upper = colorStops[i + 1];
      
      if (clampedAlt >= lower.alt && clampedAlt <= upper.alt) {

        const ratio = (clampedAlt - lower.alt) / (upper.alt - lower.alt);
        
        const r = Math.round(lower.color.r + (upper.color.r - lower.color.r) * ratio);
        const g = Math.round(lower.color.g + (upper.color.g - lower.color.g) * ratio);
        const b = Math.round(lower.color.b + (upper.color.b - lower.color.b) * ratio);
        
        return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
      }
    }
    
    // If altitude is above the highest threshold, use the highest color
    const highestColor = colorStops[colorStops.length - 1].color;
    return `#${highestColor.r.toString(16).padStart(2, '0')}${highestColor.g.toString(16).padStart(2, '0')}${highestColor.b.toString(16).padStart(2, '0')}`;
  }

  public createFlightPath(positions: TerrestialPosition[]) {
    if (positions.length > 1) {
      if (this.polyLine || this.pathSegments.length > 0) {
        this.removeFlightPath();
      }

      this.lastPositionCount = positions.length;

      // Create segments between each pair of positions
      for (let i = 0; i < positions.length - 1; i++) {
        const currentPos = positions[i];
        const nextPos = positions[i + 1];

        const distanceNM = this.calculateDistanceApprox(currentPos.lat, currentPos.lon, nextPos.lat, nextPos.lon);
        const isLargeGap = distanceNM >= this.LARGE_GAP_THRESHOLD_NM;
        const hasAltitude = currentPos.alt != null && nextPos.alt != null;      
        const avgAltitude = hasAltitude ? (currentPos.alt! + nextPos.alt!) / 2 : 0;

        const lineString = new H.geo.LineString();
        lineString.pushPoint({ lat: currentPos.lat, lng: currentPos.lon });
        lineString.pushPoint({ lat: nextPos.lat, lng: nextPos.lon });

        let strokeColor: string;
        if (isLargeGap) {
          strokeColor = this.MEDIUM_GRAY_COLOR;
        } else if (!hasAltitude) {
          strokeColor = this.LIGHT_GRAY_COLOR;
        } else {
          strokeColor = this.getColorByAltitude(avgAltitude);
        }

        const style: PolylineOptions['style'] = {
          lineWidth: 3,
          strokeColor,
          ...(isLargeGap ? { lineDash: [4, 4] } : {}), // 4px dash, 4px gap for large gaps
        };

        const segment = new H.map.Polyline(lineString, { style });

        this.map.addObject(segment);
        this.pathSegments.push(segment);
      }
    }
  }

  public updateFlightPath(positions: TerrestialPosition[]) {
    if (positions.length <= 1) {
      return;
    }

    // If we have fewer positions than before, recreate the entire path
    if (positions.length < this.lastPositionCount) {
      this.removeFlightPath();
      this.createFlightPath(positions);
      return;
    }

    // If this is the first time or we have no existing segments, create the full path
    if (this.pathSegments.length === 0 || this.lastPositionCount === 0) {
      this.createFlightPath(positions);
      return;
    }

    // Only add new segments for the additional positions
    const startIndex = this.lastPositionCount - 1; // Start from the last position we had
    for (let i = startIndex; i < positions.length - 1; i++) {
      const currentPos = positions[i];
      const nextPos = positions[i + 1];

      const distanceNM = this.calculateDistanceApprox(currentPos.lat, currentPos.lon, nextPos.lat, nextPos.lon);
      const isLargeGap = distanceNM >= this.LARGE_GAP_THRESHOLD_NM;
      const hasAltitude = currentPos.alt != null && nextPos.alt != null;      
      const avgAltitude = hasAltitude ? (currentPos.alt! + nextPos.alt!) / 2 : 0;

      const lineString = new H.geo.LineString();
      lineString.pushPoint({ lat: currentPos.lat, lng: currentPos.lon });
      lineString.pushPoint({ lat: nextPos.lat, lng: nextPos.lon });

      let strokeColor: string;
      if (isLargeGap) {
        strokeColor = this.MEDIUM_GRAY_COLOR;
      } else if (!hasAltitude) {
        strokeColor = this.LIGHT_GRAY_COLOR;
      } else {
        strokeColor = this.getColorByAltitude(avgAltitude);
      }

      const style: PolylineOptions['style'] = {
        lineWidth: 3,
        strokeColor,
        ...(isLargeGap ? { lineDash: [4, 4] } : {}),
      };

      const segment = new H.map.Polyline(lineString, { style });
      this.map.addObject(segment);
      this.pathSegments.push(segment);
    }

    this.lastPositionCount = positions.length;
  }

  public removeFlightPath(): void {
    this.pathSegments.forEach((segment) => {
      this.map.removeObject(segment);
    });
    this.pathSegments = [];

    if (this.polyLine) {
      this.map.removeObject(this.polyLine);
      this.polyLine = null;
    }
  }
}
