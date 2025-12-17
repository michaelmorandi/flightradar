/**
 * Aircraft category enum values from adsb.proto
 * These correspond to the AircraftCategory enum in the backend
 */
export enum AircraftCategory {
  UNKNOWN = 0,
  NO_INFO = 1,
  // Category Set A (TC=4)
  LIGHT = 2,              // < 15500 lbs
  MEDIUM_1 = 3,           // 15500 to 75000 lbs
  MEDIUM_2 = 4,           // 75000 to 300000 lbs
  HIGH_VORTEX_LARGE = 5,  // High wake vortex aircraft
  HEAVY = 6,              // > 300000 lbs
  HIGH_PERFORMANCE = 7,   // > 5g acceleration and > 400 kts
  ROTORCRAFT = 8,
  // Category Set B (TC=3)
  GLIDER = 9,
  LIGHTER_THAN_AIR = 10,
  PARACHUTIST = 11,
  ULTRALIGHT = 12,
  UAV = 13,
  SPACE = 14,
  // Category Set C (TC=2)
  SURFACE_EMERGENCY = 15,
  SURFACE_SERVICE = 16,
  POINT_OBSTACLE = 17,
  CLUSTER_OBSTACLE = 18,
  LINE_OBSTACLE = 19,
  // Reserved
  RESERVED = 20,
}

/**
 * Get human-readable name for an aircraft category
 */
export function getCategoryName(category: AircraftCategory): string {
  switch (category) {
    case AircraftCategory.UNKNOWN:
      return 'Unknown';
    case AircraftCategory.NO_INFO:
      return 'No Info';
    case AircraftCategory.LIGHT:
      return 'Light';
    case AircraftCategory.MEDIUM_1:
      return 'Medium 1';
    case AircraftCategory.MEDIUM_2:
      return 'Medium 2';
    case AircraftCategory.HIGH_VORTEX_LARGE:
      return 'High Vortex Large';
    case AircraftCategory.HEAVY:
      return 'Heavy';
    case AircraftCategory.HIGH_PERFORMANCE:
      return 'High Performance';
    case AircraftCategory.ROTORCRAFT:
      return 'Rotorcraft';
    case AircraftCategory.GLIDER:
      return 'Glider';
    case AircraftCategory.LIGHTER_THAN_AIR:
      return 'Lighter Than Air';
    case AircraftCategory.PARACHUTIST:
      return 'Parachutist';
    case AircraftCategory.ULTRALIGHT:
      return 'Ultralight';
    case AircraftCategory.UAV:
      return 'UAV';
    case AircraftCategory.SPACE:
      return 'Space';
    case AircraftCategory.SURFACE_EMERGENCY:
      return 'Surface Emergency';
    case AircraftCategory.SURFACE_SERVICE:
      return 'Surface Service';
    case AircraftCategory.POINT_OBSTACLE:
      return 'Point Obstacle';
    case AircraftCategory.CLUSTER_OBSTACLE:
      return 'Cluster Obstacle';
    case AircraftCategory.LINE_OBSTACLE:
      return 'Line Obstacle';
    case AircraftCategory.RESERVED:
      return 'Reserved';
    default:
      return 'Unknown';
  }
}
