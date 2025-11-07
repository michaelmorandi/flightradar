import { TerrestialPosition } from '@/model/backendModel';
import { AircraftIcon, AircraftMarker } from '@/components/map/aircraftElements';
import { HereCoordinates } from '@/components/map/flightPath';
import { usePositionStore } from '@/stores';
import { differenceInSeconds } from 'date-fns';

export class MarkerManager {
  private markers: Map<string, AircraftMarker> = new Map();
  private iconSvgMap: Map<string, any> = new Map();
  private aircraftIcon: AircraftIcon;
  private map: any;
  private positionStore: ReturnType<typeof usePositionStore>;
  private onMarkerClickCallback?: (flightId: string) => void;

  constructor(map: any, positionStore: ReturnType<typeof usePositionStore>) {
    this.map = map;
    this.positionStore = positionStore;
    this.aircraftIcon = new AircraftIcon(this.iconSvgMap);
  }

  setOnMarkerClickCallback(callback: (flightId: string) => void) {
    this.onMarkerClickCallback = callback;
  }

  updateMarker(id: string, coords: HereCoordinates, groundSpeed?: number, callsign?: string) {
    if (this.markers.has(id)) {
      const marker = this.markers.get(id);
      marker?.updatePosition(coords, groundSpeed);
      if (callsign) {
        marker?.updateCallsign(callsign);
      }
    } else {
      const marker = new AircraftMarker(id, coords, this.aircraftIcon, this.map, this.iconSvgMap, callsign);
      marker.onPointerDown((event: any) => {
        if (this.onMarkerClickCallback) {
          this.onMarkerClickCallback(event.target.getData());
        }
      });
      this.markers.set(id, marker);
    }
  }

  removeMarker(id: string): boolean {
    const marker = this.markers.get(id);
    if (marker) {
      marker.remove();
      this.markers.delete(id);
      this.iconSvgMap.delete(id);
      return true;
    }
    return false;
  }

  hasMarker(id: string): boolean {
    return this.markers.has(id);
  }

  getMarker(id: string): AircraftMarker | undefined {
    return this.markers.get(id);
  }

  setMarkerColor(id: string, color: string) {
    const marker = this.markers.get(id);
    if (marker) {
      marker.setColor(color);
    }
  }

  calculateHeading(lat1: number, lon1: number, lat2: number, lon2: number): number {
    const lat1Rad = (lat1 * Math.PI) / 180;
    const lon1Rad = (lon1 * Math.PI) / 180;
    const lat2Rad = (lat2 * Math.PI) / 180;
    const lon2Rad = (lon2 * Math.PI) / 180;

    const y = Math.sin(lon2Rad - lon1Rad) * Math.cos(lat2Rad);
    const x = Math.cos(lat1Rad) * Math.sin(lat2Rad) - Math.sin(lat1Rad) * Math.cos(lat2Rad) * Math.cos(lon2Rad - lon1Rad);
    let heading = (Math.atan2(y, x) * 180) / Math.PI;

    heading = (heading + 360) % 360;
    return heading;
  }

  convertToHereCoords(flPos: TerrestialPosition, positions?: Map<string, TerrestialPosition>): HereCoordinates {
    if (flPos.track !== undefined) {
      return { lat: Number(flPos.lat), lng: Number(flPos.lon), heading: flPos.track } as HereCoordinates;
    }

    if (positions && flPos.icao) {
      const positionEntries = Array.from(positions.entries());

      for (let i = positionEntries.length - 1; i >= 0; i--) {
        const [_, prevPos] = positionEntries[i];
        if (prevPos.icao === flPos.icao && (prevPos.lat !== flPos.lat || prevPos.lon !== flPos.lon)) {
          const heading = this.calculateHeading(prevPos.lat, prevPos.lon, flPos.lat, flPos.lon);
          return { lat: Number(flPos.lat), lng: Number(flPos.lon), heading } as HereCoordinates;
        }
      }
    }

    return { lat: Number(flPos.lat), lng: Number(flPos.lon), heading: 0 } as HereCoordinates;
  }

  updateAircraftPositions(positions: Map<string, TerrestialPosition>) {
    this.positionStore.updatePositions(positions);

    positions.forEach((pos: TerrestialPosition, flightId: string) => {
      const coords = this.convertToHereCoords(pos, positions);
      this.updateMarker(flightId, coords, pos.gs, pos.callsign);
    });

    const now = new Date();

    // Purge stale markers
    for (const [key, value] of this.markers) {
      if (differenceInSeconds(now, value.lastUpdated) > this.positionStore.staleThreshold) {
        this.removeMarker(key);
      }
    }

    this.positionStore.purgeStalePositions();
  }

  getAllMarkers(): Map<string, AircraftMarker> {
    return new Map(this.markers);
  }

  clearAllMarkers() {
    this.markers.forEach((marker) => {
      marker.remove();
    });
    this.markers.clear();
    this.iconSvgMap.clear();
  }

  getMarkerCount(): number {
    return this.markers.size;
  }
}
