// Aircraft type to icon mapping utilities

export type AircraftCategory = 'A1' | 'A2' | 'A3' | 'A4' | 'A5' | 'A6' | 'A7' | 'B1' | 'B2' | 'B3' | 'B4' | 'B5' | 'B6' | 'default';

// Map of aircraft categories to their descriptions
export const AIRCRAFT_CATEGORIES = {
  A1: 'Light (< 15,500 lbs)',
  A2: 'Small (15,500 - 75,000 lbs)',
  A3: 'Large (75,000 - 300,000 lbs)',
  A4: 'High Vortex (e.g., B757)',
  A5: 'Heavy (> 300,000 lbs)',
  A6: 'High Performance (> 5g, > 400 kts)',
  A7: 'Rotorcraft',
  B1: 'Glider',
  B2: 'Lighter-than-air',
  B3: 'UAV/Drone',
  B4: 'Small UAV',
  B5: 'Space/Experimental',
  B6: 'Ultralight',
  default: 'Unknown'
} as const;

/**
 * Map ADS-B protobuf category enum values to icon categories
 * Based on the AircraftCategory enum in adsb.proto
 */
export function mapProtobufCategoryToIcon(protoCategory?: number): AircraftCategory {
  if (!protoCategory) return 'default';

  switch (protoCategory) {
    case 2:  // AIRCRAFT_CATEGORY_LIGHT
      return 'A1';
    case 3:  // AIRCRAFT_CATEGORY_MEDIUM_1
      return 'A2';
    case 4:  // AIRCRAFT_CATEGORY_MEDIUM_2
      return 'A3';
    case 5:  // AIRCRAFT_CATEGORY_HIGH_VORTEX_LARGE
      return 'A4';
    case 6:  // AIRCRAFT_CATEGORY_HEAVY
      return 'A5';
    case 7:  // AIRCRAFT_CATEGORY_HIGH_PERFORMANCE
      return 'A6';
    case 8:  // AIRCRAFT_CATEGORY_ROTORCRAFT
      return 'A7';
    case 9:  // AIRCRAFT_CATEGORY_GLIDER
      return 'B1';
    case 10: // AIRCRAFT_CATEGORY_LIGHTER_THAN_AIR
      return 'B2';
    case 13: // AIRCRAFT_CATEGORY_UAV
      return 'B3';
    case 12: // AIRCRAFT_CATEGORY_ULTRALIGHT
      return 'B6';
    case 14: // AIRCRAFT_CATEGORY_SPACE
      return 'B5';
    case 0:  // AIRCRAFT_CATEGORY_UNKNOWN
    case 1:  // AIRCRAFT_CATEGORY_NO_INFO
    default:
      return 'default';
  }
}

// Map of aircraft categories to their SVG file paths
// Icons are served from the public directory, so they're accessible at /icons/...
const ICON_PATH_MAP: Record<AircraftCategory, string> = {
  A1: '/icons/aircraft-a1-light.svg',
  A2: '/icons/aircraft-a2-small.svg',
  A3: '/icons/aircraft-a3-large.svg',
  A4: '/icons/aircraft-a4-highvortex.svg',
  A5: '/icons/aircraft-a5-heavy.svg',
  A6: '/icons/aircraft-a6-highperf.svg',
  A7: '/icons/aircraft-a7-rotorcraft.svg',
  B1: '/icons/aircraft-b1-glider.svg',
  B2: '/icons/aircraft-b2-balloon.svg',
  B3: '/icons/aircraft-b3-uav.svg',
  B4: '/icons/aircraft-b4-smalluav.svg',
  B5: '/icons/aircraft-b5-space.svg',
  B6: '/icons/aircraft-b6-ultralight.svg',
  default: '/icons/aircraft-default.svg'
};

// Cache for loaded SVG content
const svgCache = new Map<AircraftCategory, string>();

/**
 * Get the icon path for a given aircraft category
 */
export function getIconPath(category: AircraftCategory): string {
  return ICON_PATH_MAP[category] || ICON_PATH_MAP.default;
}

/**
 * Load SVG content for a given aircraft category
 */
export async function loadIconSvg(category: AircraftCategory): Promise<string> {
  // Check cache first
  if (svgCache.has(category)) {
    return svgCache.get(category)!;
  }

  const path = getIconPath(category);

  try {
    const response = await fetch(path);
    if (!response.ok) {
      throw new Error(`Failed to load SVG: ${response.statusText}`);
    }

    const svgContent = await response.text();
    svgCache.set(category, svgContent);
    return svgContent;
  } catch (error) {
    console.error(`Error loading icon for category ${category}:`, error);
    // Fall back to default icon if specific icon fails to load
    if (category !== 'default') {
      return loadIconSvg('default');
    }
    // If even default fails, return a simple fallback SVG
    return `<svg xmlns="http://www.w3.org/2000/svg" width="15px" height="20px">
      <polygon points="0,20 7.5,12 15,20 7.5,0 0,20" fill="rgb(250, 255, 255)" stroke="black" stroke-width="1" />
    </svg>`;
  }
}

/**
 * Preload all aircraft icons
 */
export async function preloadAllIcons(): Promise<void> {
  const categories: AircraftCategory[] = ['A1', 'A2', 'A3', 'A4', 'A5', 'A6', 'A7', 'B1', 'B2', 'B3', 'B4', 'B5', 'B6', 'default'];

  await Promise.all(categories.map(category => loadIconSvg(category)));
}

/**
 * Determine aircraft category from type string or ICAO type code
 * This is a placeholder implementation - you may want to implement
 * more sophisticated logic based on your data
 */
export function determineAircraftCategory(aircraftType?: string, icaoType?: string): AircraftCategory {
  if (!aircraftType && !icaoType) {
    return 'default';
  }

  const type = (aircraftType || icaoType || '').toUpperCase();

  // Rotorcraft detection
  if (type.includes('HELI') || type.startsWith('H') || type.includes('ROTOR')) {
    return 'A7';
  }

  // Glider detection
  if (type.includes('GLID') || type.startsWith('G')) {
    return 'B1';
  }

  // Balloon/Airship detection
  if (type.includes('BALL') || type.includes('BLIMP') || type.includes('AIRSHIP')) {
    return 'B2';
  }

  // UAV/Drone detection
  if (type.includes('DRONE') || type.includes('UAV') || type.includes('UAS') || type.includes('QUAD')) {
    return 'B3';
  }

  // High performance (fighter jets, etc.)
  if (type.startsWith('F-') || type.startsWith('F/') || type.includes('FIGHTER')) {
    return 'A6';
  }

  // Heavy aircraft (A380, B747, etc.)
  if (type.includes('A380') || type.includes('B747') || type.includes('B777') || type.includes('A350')) {
    return 'A5';
  }

  // High vortex (B757, B762)
  if (type.includes('B757') || type.includes('B752') || type.includes('B753')) {
    return 'A4';
  }

  // Large aircraft (most commercial jets)
  if (type.includes('B737') || type.includes('A320') || type.includes('A321') ||
      type.includes('B787') || type.includes('A330') || type.startsWith('B7') || type.startsWith('A3')) {
    return 'A3';
  }

  // Small aircraft (regional jets, turboprops)
  if (type.includes('CRJ') || type.includes('ERJ') || type.includes('AT') ||
      type.includes('DHC') || type.includes('E170') || type.includes('E190')) {
    return 'A2';
  }

  // Light aircraft (single engine, small twins)
  if (type.includes('C172') || type.includes('PA') || type.includes('C182') ||
      type.includes('SR2') || type.length <= 4) {
    return 'A1';
  }

  // Default to large if it looks like a commercial aircraft type code
  if (type.length === 4 && /^[A-Z0-9]+$/.test(type)) {
    return 'A3';
  }

  return 'default';
}

/**
 * Create an SVG element from SVG string content and apply colors
 */
export function createSvgElement(svgContent: string, fillColor: string = '250, 255, 255'): SVGElement {
  const parser = new DOMParser();
  const doc = parser.parseFromString(svgContent, 'image/svg+xml');
  const svgElement = doc.querySelector('svg');

  if (!svgElement) {
    throw new Error('Invalid SVG content');
  }

  // Apply fill color to all fillable elements
  const fillableElements = svgElement.querySelectorAll('[fill]');
  fillableElements.forEach(element => {
    const currentFill = element.getAttribute('fill');
    // Only update if it's not 'none' or a special value
    if (currentFill && currentFill !== 'none' && !currentFill.includes('url(')) {
      element.setAttribute('fill', `rgb(${fillColor})`);
    }
  });

  return svgElement as unknown as SVGElement;
}

/**
 * Update the fill and stroke color of an SVG element
 */
export function updateSvgColor(svgElement: SVGElement, fillColor: string): void {
  // Update fill attributes
  const fillableElements = svgElement.querySelectorAll('[fill]');
  fillableElements.forEach(element => {
    const currentFill = element.getAttribute('fill');
    if (currentFill && currentFill !== 'none' && !currentFill.includes('url(')) {
      element.setAttribute('fill', `rgb(${fillColor})`);
    }
  });

  // Update stroke attributes (but preserve black strokes for outlines)
  const strokeElements = svgElement.querySelectorAll('[stroke]');
  strokeElements.forEach(element => {
    const currentStroke = element.getAttribute('stroke');
    // Update stroke colors that aren't black (black is used for outlines)
    if (currentStroke && currentStroke !== 'black' && !currentStroke.includes('url(')) {
      element.setAttribute('stroke', `rgb(${fillColor})`);
    }
  });
}
