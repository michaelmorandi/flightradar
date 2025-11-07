import { TerrestialPosition } from '@/model/backendModel';
import { FlightPath } from '@/components/map/flightPath';
import { FlightRadarService } from '@/services/flightRadarService';
import { useFlightStore, useMapStore, useConnectionStore } from '@/stores';
import { MarkerManager } from './MarkerManager';
import { AircraftIcon } from '@/components/map/aircraftElements';

export class FlightPathManager {
  private selectedFlightPath: FlightPath | null = null;
  private map: any;
  private radarService: FlightRadarService;
  private flightStore: ReturnType<typeof useFlightStore>;
  private mapStore: ReturnType<typeof useMapStore>;
  private connectionStore: ReturnType<typeof useConnectionStore>;
  private markerManager: MarkerManager;
  private onFlightSelectedCallback?: (flightId: string) => void;

  constructor(
    map: any, 
    radarService: FlightRadarService,
    flightStore: ReturnType<typeof useFlightStore>,
    mapStore: ReturnType<typeof useMapStore>,
    connectionStore: ReturnType<typeof useConnectionStore>,
    markerManager: MarkerManager
  ) {
    this.map = map;
    this.radarService = radarService;
    this.flightStore = flightStore;
    this.mapStore = mapStore;
    this.connectionStore = connectionStore;
    this.markerManager = markerManager;
  }

  setOnFlightSelectedCallback(callback: (flightId: string) => void) {
    this.onFlightSelectedCallback = callback;
  }

  getSelectedFlightPath(): FlightPath | null {
    return this.selectedFlightPath;
  }

  async addFlightPath(flightId: string): Promise<void> {
    try {
      const previousFlight = this.selectedFlightPath;
      if (previousFlight && previousFlight.flightId !== flightId) {
        if (this.connectionStore.isFlightConnected(previousFlight.flightId)) {
          this.connectionStore.disconnectFlightPositionsConnection(this.radarService, previousFlight.flightId);
        }
        
        if (this.markerManager.hasMarker(previousFlight.flightId)) {
          this.markerManager.setMarkerColor(previousFlight.flightId, AircraftIcon.INACTIVE_COLOR);
        }
      }

      this.selectedFlightPath = new FlightPath(flightId, this.map);

      // Clean up previous flight path AFTER creating the new one to avoid duplicate cleanup
      if (previousFlight) {
        previousFlight.removeFlightPath();
      }

      if (this.markerManager.hasMarker(flightId)) {
        this.markerManager.setMarkerColor(flightId, AircraftIcon.HIGHLIGHT_COLOR);
      }

      const positionsCallback = (positions: TerrestialPosition[]) => {
        // Only update if this is still the selected flight
        if (this.selectedFlightPath && this.selectedFlightPath.flightId === flightId && positions && positions.length > 0) {
          this.selectedFlightPath.updateFlightPath(positions);
        }
      };
      
      this.connectionStore.registerFlightPositionsConnection(this.radarService, flightId, positionsCallback);
      
      this.mapStore.highlightFlight(flightId);
      this.flightStore.selectFlight(flightId);
      
      if (this.onFlightSelectedCallback) {
        this.onFlightSelectedCallback(flightId);
      }
      
    } catch (error) {
      console.error('Error setting up flight path:', error);
      // If there's an error, clean up current selection
      if (this.selectedFlightPath && this.selectedFlightPath.flightId === flightId) {
        const flightToRemove = this.selectedFlightPath;
        this.selectedFlightPath = null;
        flightToRemove.removeFlightPath();

        if (this.markerManager.hasMarker(flightId)) {
          this.markerManager.setMarkerColor(flightId, AircraftIcon.INACTIVE_COLOR);
        }
        
        if (this.connectionStore.isFlightConnected(flightId)) {
          this.connectionStore.disconnectFlightPositionsConnection(this.radarService, flightId);
        }
        
        this.mapStore.clearHighlight();
        this.flightStore.clearSelectedFlight();
      }
    }
  }

  selectFlight(flightId: string): void {
    // Reset color of previously selected aircraft if it exists and it's not the same as the newly selected one
    if (this.selectedFlightPath && this.selectedFlightPath.flightId !== flightId && this.markerManager.hasMarker(this.selectedFlightPath.flightId)) {
      this.markerManager.setMarkerColor(this.selectedFlightPath.flightId, AircraftIcon.INACTIVE_COLOR);
    }

    if (this.markerManager.hasMarker(flightId)) {
      this.markerManager.setMarkerColor(flightId, AircraftIcon.HIGHLIGHT_COLOR);
    }

    this.mapStore.selectMarker(flightId);
    this.flightStore.selectFlight(flightId);
    this.mapStore.highlightFlight(flightId);

    this.addFlightPath(flightId);
  }

  unselectFlight(): void {
    this.resetIcon();

    if (this.selectedFlightPath) {
      const flightId = this.selectedFlightPath.flightId;

      if (this.connectionStore.isFlightConnected(flightId)) {
        this.connectionStore.disconnectFlightPositionsConnection(this.radarService, flightId);
      }

      const flightToRemove = this.selectedFlightPath;
      this.selectedFlightPath = null; // Clear the reference first to prevent recursive cleanup
      flightToRemove.removeFlightPath();
      

      this.mapStore.clearHighlight();
      this.flightStore.clearSelectedFlight();
    }
  }

  private resetIcon(): void {
    if (this.selectedFlightPath && this.markerManager.hasMarker(this.selectedFlightPath.flightId)) {
      this.markerManager.setMarkerColor(this.selectedFlightPath.flightId, AircraftIcon.INACTIVE_COLOR);
    }
  }


  handleFlightSelectionChange(newFlightId: string | null, oldFlightId: string | null): void {
    if (oldFlightId && oldFlightId !== newFlightId) {
      if (this.markerManager.hasMarker(oldFlightId)) {
        this.markerManager.setMarkerColor(oldFlightId, AircraftIcon.INACTIVE_COLOR);
      }
    }
    
    if (newFlightId && newFlightId !== oldFlightId) {
      if (this.markerManager.hasMarker(newFlightId)) {
        this.markerManager.setMarkerColor(newFlightId, AircraftIcon.HIGHLIGHT_COLOR);
      }
    }
  }

  handlePropsFlightIdChange(newFlightId: string | null): void {
    if (newFlightId) {
      this.mapStore.highlightFlight(newFlightId);
      this.flightStore.selectFlight(newFlightId);
      this.addFlightPath(newFlightId);
    } else {
      this.mapStore.clearHighlight();
      this.flightStore.clearSelectedFlight();
    }
  }

  cleanup(): void {
    if (this.selectedFlightPath) {
      const flightId = this.selectedFlightPath.flightId;

      if (this.connectionStore.isFlightConnected(flightId)) {
        this.connectionStore.disconnectFlightPositionsConnection(this.radarService, flightId);
      }

      this.selectedFlightPath.removeFlightPath();
      this.selectedFlightPath = null;
    }
  }
}