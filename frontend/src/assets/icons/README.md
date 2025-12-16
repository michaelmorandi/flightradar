# Aircraft Type Icons

This directory contains SVG icons for different aircraft types used in the FlightRadar application.

## Icon Files

### Category A: Standard Aircraft (by weight/size)

- **A1 (Light)**: `aircraft-a1-light.svg` - Aircraft < 15,500 lbs
  - Single-engine light aircraft (e.g., Cessna 172, Piper PA-28)

- **A2 (Small)**: `aircraft-a2-small.svg` - Aircraft 15,500 - 75,000 lbs
  - Regional aircraft, small twins (e.g., CRJ, ERJ, ATR)

- **A3 (Large)**: `aircraft-a3-large.svg` - Aircraft 75,000 - 300,000 lbs
  - Commercial airliners (e.g., Boeing 737, Airbus A320)

- **A4 (High Vortex)**: `aircraft-a4-highvortex.svg`
  - Aircraft creating strong wake turbulence (e.g., Boeing 757)

- **A5 (Heavy)**: `aircraft-a5-heavy.svg` - Aircraft > 300,000 lbs
  - Wide-body jets (e.g., Boeing 747, Airbus A380)

- **A6 (High Performance)**: `aircraft-a6-highperf.svg`
  - High-performance aircraft (> 5g, > 400 kts) - Fighter jets, military aircraft

- **A7 (Rotorcraft)**: `aircraft-a7-rotorcraft.svg`
  - Helicopters and other rotary-wing aircraft

### Category B: Special Aircraft Types

- **B1 (Glider)**: `aircraft-b1-glider.svg`
  - Sailplanes and gliders

- **B2 (Lighter-than-air)**: `aircraft-b2-balloon.svg`
  - Balloons, blimps, and airships

- **B3 (UAV)**: `aircraft-b3-uav.svg`
  - Drones and unmanned aerial vehicles (quadcopters)

- **B4 (Small UAV)**: `aircraft-b4-smalluav.svg`
  - Fixed-wing drones and small UAVs

- **B5 (Space)**: `aircraft-b5-space.svg`
  - Space vehicles and experimental aircraft

- **B6 (Ultralight)**: `aircraft-b6-ultralight.svg`
  - Ultralights and powered paragliders

### Default Icon

- **Default**: `aircraft-default.svg`
  - Used when aircraft type is unknown or cannot be determined

## Icon Specifications

All icons are designed to:
- Be 15px × 20px in size
- Use a viewBox of "0 0 15 20"
- Have a default fill color of `rgb(250, 255, 255)` (off-white)
- Use black strokes with 0.8px width
- Support dynamic color changes for selection/highlighting
- Rotate based on aircraft heading

## Usage in Code

The aircraft icon system is implemented in:

### Core Files
- **Icon Utilities**: `/src/utils/aircraftIcons.ts`
- **Aircraft Elements**: `/src/components/map/aircraftElements.ts`

### Loading Icons

```typescript
import { loadIconSvg, determineAircraftCategory } from '@/utils/aircraftIcons';

// Determine category from aircraft data
const category = determineAircraftCategory(aircraftType, icaoType);

// Load the SVG content
const svgContent = await loadIconSvg(category);
```

### Automatic Type Detection

The `determineAircraftCategory()` function automatically maps aircraft type codes to categories:

```typescript
// Examples
determineAircraftCategory('B737', undefined) // → 'A3' (Large)
determineAircraftCategory('B757', undefined) // → 'A4' (High Vortex)
determineAircraftCategory('HELI', undefined) // → 'A7' (Rotorcraft)
determineAircraftCategory('C172', undefined) // → 'A1' (Light)
```

### Using with Markers

Aircraft markers automatically load the appropriate icon based on type:

```typescript
const marker = new AircraftMarker(
  flightId,
  coords,
  aircraftIcon,
  map,
  iconSvgMap,
  callsign,
  aircraftType,  // Optional: e.g., 'B737'
  icaoType       // Optional: e.g., 'B738'
);

// Update aircraft type later if needed
marker.updateAircraftType('B777', 'B77W');
```

## Extending the Type Detection

To improve aircraft type detection, edit the `determineAircraftCategory()` function in `/src/utils/aircraftIcons.ts`. You can:

1. Add more specific aircraft type mappings
2. Integrate with aircraft database lookups
3. Use ICAO aircraft type codes for more accurate categorization
4. Parse manufacturer and model information

## Customizing Icons

To modify or add new icons:

1. Create SVG files following the same dimensions (15px × 20px)
2. Use the viewBox "0 0 15 20"
3. Set fill colors to `rgb(250, 255, 255)` for default state
4. Ensure fillable elements have explicit `fill` attributes (not CSS classes)
5. Use consistent stroke widths (0.6 - 1.2px)
6. Test rotation by viewing at different angles

## Future Enhancements

When aircraft type data becomes available in `TerrestialPosition`:

1. **Update Backend Model** (`/src/model/backendModel.ts`):
   ```typescript
   export interface TerrestialPosition {
     icao: string;
     callsign: string;
     lat: number;
     lon: number;
     alt?: number;
     track?: number;
     gs?: number;
     type?: string;      // Add this
     icaoType?: string;  // Add this
   }
   ```

2. **Update MarkerManager** (`/src/components/map/MarkerManager.ts`):
   ```typescript
   updateMarker(id: string, coords: HereCoordinates, groundSpeed?: number,
                callsign?: string, aircraftType?: string, icaoType?: string) {
     if (this.markers.has(id)) {
       const marker = this.markers.get(id);
       marker?.updatePosition(coords, groundSpeed);
       marker?.updateAircraftType(aircraftType, icaoType);
       // ...
     } else {
       const marker = new AircraftMarker(
         id, coords, this.aircraftIcon, this.map,
         this.iconSvgMap, callsign, aircraftType, icaoType
       );
       // ...
     }
   }
   ```

3. **Pass Type Data from Position Updates**:
   Update the code that calls `updateMarker()` to include aircraft type information from the backend.

## License

These icons are part of the FlightRadar application and follow the same license as the main project.
