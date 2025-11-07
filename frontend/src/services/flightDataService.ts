import { FlightRadarService } from '@/services/flightRadarService';
import { TerrestialPosition } from '@/model/backendModel';
import { firstValueFrom } from 'rxjs';

type TerrPosArray = Array<TerrestialPosition>;

export class FlightDataServiceImp {
  private positionCache: Map<string, TerrPosArray> = new Map();

  private latestPosition: Map<string, TerrestialPosition> = new Map();

  constructor(private frService: FlightRadarService) {}

  public getFlightPositions(flightId: string): Promise<TerrPosArray> {
    // Convert Observable to Promise
    return firstValueFrom(this.frService.getPositions(flightId));
  }
}
