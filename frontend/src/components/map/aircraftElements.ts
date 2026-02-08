// This file contains elements for aircraft visualization
import { HereCoordinates } from './flightPath';
import { AircraftInterpolator } from './AircraftInterpolator';
import {
  type AircraftCategory,
  loadIconSvg,
  determineAircraftCategory,
  mapProtobufCategoryToIcon
} from '@/utils/aircraftIcons';

// Here Maps API interfaces
interface HereMarker {
  setGeometry(coords: HereCoordinates): void;
  addEventListener(event: string, callback: (event: { target: { getData(): string } }) => void): void;
  getData(): string;
}

// Just a type marker for the DOM icon, actual implementation is in the H namespace
// eslint-disable-next-line @typescript-eslint/no-empty-interface
interface HereDomIcon {}

interface IconOptions {
  onAttach: (clonedElement: HTMLElement, domIcon: unknown, domMarker: { getData(): string }) => void;
  onDetach: (clonedElement: HTMLElement, domIcon: unknown, domMarker: { getData(): string }) => void;
}

declare let H: {
  map: {
    DomIcon: new (element: HTMLElement, options: IconOptions) => HereDomIcon;
    DomMarker: new (coords: HereCoordinates, options: { icon: HereDomIcon; data: string }) => HereMarker;
  };
};

// Cache structure for optimized SVG rendering
export interface CachedSvgData {
  svgElement: SVGElement;
  fillableElements: Element[];
  strokeableElements: Element[];
  currentColor: string;
}

// Track element references for visible SVGs
interface VisibleSvgElements {
  fillableElements: Element[];
  strokeableElements: Element[];
}

export class AircraftIcon {
  private aircraftIcon: HereDomIcon;
  private popoverMap: Map<string, HTMLElement> = new Map();
  private svgDataCache: Map<string, CachedSvgData> = new Map(); // Optimized cache with pre-computed element lists
  private visibleElementsCache: Map<string, VisibleSvgElements> = new Map(); // Track visible SVG elements for fast updates
  private lastRotationMap: Map<string, number> = new Map(); // Track last rotation to avoid unnecessary updates

  public static readonly INACTIVE_COLOR = '250, 255, 255';
  public static readonly HIGHLIGHT_COLOR = '250, 127, 0';
  public static readonly MILITARY_COLOR = '144, 190, 109';

  constructor(
    private iconSvgMap: Map<string, SVGElement>,
    private callsignMap: Map<string, string> = new Map(),
    private aircraftTypeMap: Map<string, AircraftCategory> = new Map(),
  ) {
    const aircraftDomIconElement = document.createElement('div');
    aircraftDomIconElement.className = 'aircraft-icon-wrapper';

    aircraftDomIconElement.style.willChange = 'transform';
    aircraftDomIconElement.style.position = 'relative';

    aircraftDomIconElement.innerHTML = `
      <svg xmlns="http://www.w3.org/2000/svg" width="11.25px" height="15px" style="position: absolute; left: -5.625px; top: -7.5px; will-change: transform; backface-visibility: hidden;" viewBox="0 0 15 20">
        <polygon points="0,20 7.5,12 15,20 7.5,0 0,20" fill="rgb(${AircraftIcon.INACTIVE_COLOR})" stroke="black" stroke-width="1" />
      </svg>
      <div class="aircraft-popover" style="
        display: none;
        position: absolute;
        bottom: 20px;
        left: 0;
        transform: translateX(-50%);
        background: rgba(0, 0, 0, 0.8);
        color: white;
        padding: 4px 8px;
        border-radius: 4px;
        font-size: 12px;
        font-family: sans-serif;
        white-space: nowrap;
        pointer-events: none;
        z-index: 1000;
      "></div>
    `;

    this.aircraftIcon = new H.map.DomIcon(aircraftDomIconElement, {
      onAttach: (clonedElement: HTMLElement, _domIcon: unknown, domMarker: { getData(): string }) => {
        const flightId = domMarker.getData();
        const clonedContent = clonedElement.getElementsByTagName('svg')[0];

        // Check if we have a cached SVG for this flight and restore it immediately
        const cachedData = this.svgDataCache.get(flightId);
        if (cachedData && clonedContent.parentElement) {
          // Clone the cached SVG - this is much faster than re-parsing and re-querying
          const restoredSvg = cachedData.svgElement.cloneNode(true) as SVGElement;

          // Replace the default SVG with the cached one
          clonedContent.parentElement.replaceChild(restoredSvg, clonedContent);
          this.iconSvgMap.set(flightId, restoredSvg);

          // Cache the visible element references for fast color updates
          const fillableElements: Element[] = [];
          const strokeableElements: Element[] = [];

          restoredSvg.querySelectorAll('*').forEach(element => {
            const svgEl = element as SVGElement;

            // Check for fill in attribute or style
            const fillAttr = svgEl.getAttribute('fill');
            if (fillAttr && fillAttr !== 'none' && !fillAttr.includes('url(')) {
              fillableElements.push(element);
            } else if (svgEl.style.fill && svgEl.style.fill !== 'none' && !svgEl.style.fill.includes('url(')) {
              fillableElements.push(element);
            }

            // Check for stroke in attribute or style
            const strokeAttr = svgEl.getAttribute('stroke');
            if (strokeAttr && strokeAttr !== 'black' && strokeAttr !== '#000000' && !strokeAttr.includes('url(')) {
              strokeableElements.push(element);
            } else if (svgEl.style.stroke && svgEl.style.stroke !== 'black' && svgEl.style.stroke !== '#000000' && !svgEl.style.stroke.includes('url(')) {
              strokeableElements.push(element);
            }
          });

          this.visibleElementsCache.set(flightId, { fillableElements, strokeableElements });

          // Restore the last known rotation
          const lastRotation = this.lastRotationMap.get(flightId);
          if (lastRotation !== undefined) {
            restoredSvg.style.transform = `rotate(${lastRotation}deg)`;
          }
        } else {
          this.iconSvgMap.set(flightId, clonedContent);
        }

        const popover = clonedElement.querySelector('.aircraft-popover') as HTMLElement;
        if (popover) {
          this.popoverMap.set(flightId, popover);
          const callsign = this.callsignMap.get(flightId);
          popover.textContent = callsign || flightId;
        }

        clonedElement.addEventListener('mouseenter', () => {
          if (popover) {
            popover.style.display = 'block';
          }
        });

        clonedElement.addEventListener('mouseleave', () => {
          if (popover) {
            popover.style.display = 'none';
          }
        });
      },
      onDetach: (_clonedElement: HTMLElement, _domIcon: unknown, domMarker: { getData(): string }) => {
        const flightId = domMarker.getData();
        if (this.iconSvgMap.has(flightId)) {
          this.iconSvgMap.delete(flightId);
        }
        if (this.popoverMap.has(flightId)) {
          this.popoverMap.delete(flightId);
        }
        if (this.visibleElementsCache.has(flightId)) {
          this.visibleElementsCache.delete(flightId);
        }
      },
    });
  }

  public get hereIcon() {
    return this.aircraftIcon;
  }

  public setCallsign(flightId: string, callsign: string) {
    this.callsignMap.set(flightId, callsign);

    const popover = this.popoverMap.get(flightId);
    if (popover) {
      popover.textContent = callsign;
    }
  }

  public setAircraftType(flightId: string, category: AircraftCategory) {
    this.aircraftTypeMap.set(flightId, category);
  }

  public getAircraftType(flightId: string): AircraftCategory {
    return this.aircraftTypeMap.get(flightId) || 'default';
  }

  public getCachedSvgData(flightId: string): CachedSvgData | undefined {
    return this.svgDataCache.get(flightId);
  }

  public getVisibleElements(flightId: string): VisibleSvgElements | undefined {
    return this.visibleElementsCache.get(flightId);
  }

  public setVisibleElements(flightId: string, fillableElements: Element[], strokeableElements: Element[]) {
    this.visibleElementsCache.set(flightId, { fillableElements, strokeableElements });
  }

  public cacheSvgForFlight(flightId: string, svgElement: SVGElement, color: string = AircraftIcon.INACTIVE_COLOR) {
    // Clone the SVG first
    const clonedSvg = svgElement.cloneNode(true) as SVGElement;

    // Pre-compute lists of fillable and strokeable elements for fast color updates
    // Query from the CLONED element so references match
    const fillableElements: Element[] = [];
    const strokeableElements: Element[] = [];

    // Find all elements that have fill or stroke (either as attributes OR in style)
    clonedSvg.querySelectorAll('*').forEach(element => {
      const svgEl = element as SVGElement;

      // Check for fill in attribute
      const fillAttr = svgEl.getAttribute('fill');
      if (fillAttr && fillAttr !== 'none' && !fillAttr.includes('url(')) {
        fillableElements.push(element);
      }
      // Check for fill in style attribute
      else if (svgEl.style.fill && svgEl.style.fill !== 'none' && !svgEl.style.fill.includes('url(')) {
        fillableElements.push(element);
      }

      // Check for stroke in attribute
      const strokeAttr = svgEl.getAttribute('stroke');
      if (strokeAttr && strokeAttr !== 'black' && strokeAttr !== '#000000' && !strokeAttr.includes('url(')) {
        strokeableElements.push(element);
      }
      // Check for stroke in style attribute
      else if (svgEl.style.stroke && svgEl.style.stroke !== 'black' && svgEl.style.stroke !== '#000000' && !svgEl.style.stroke.includes('url(')) {
        strokeableElements.push(element);
      }
    });

    // Apply the initial color
    this.applyColorToElements(fillableElements, strokeableElements, color);

    // Store the optimized cache data
    this.svgDataCache.set(flightId, {
      svgElement: clonedSvg,
      fillableElements,
      strokeableElements,
      currentColor: color
    });
  }

  public setCurrentColor(flightId: string, color: string) {
    const cachedData = this.svgDataCache.get(flightId);
    if (cachedData) {
      // Only update if color actually changed
      if (cachedData.currentColor !== color) {
        // Apply color directly to the cached SVG element
        const rgbColor = `rgb(${color})`;

        // Only update fill colors - stroke colors (outlines) should remain black
        for (let i = 0; i < cachedData.fillableElements.length; i++) {
          const svgEl = cachedData.fillableElements[i] as SVGElement;
          // Update attribute if it exists
          if (svgEl.hasAttribute('fill')) {
            svgEl.setAttribute('fill', rgbColor);
          }
          // Update style if it exists
          if (svgEl.style.fill) {
            svgEl.style.fill = rgbColor;
          }
        }

        // Update the cached color
        cachedData.currentColor = color;
      }
    }
  }

  private applyColorToElements(fillableElements: Element[], _strokeableElements: Element[], color: string) {
    // Apply colors - only update fill, NOT stroke (outlines must stay black)
    const rgbColor = `rgb(${color})`;

    for (let i = 0; i < fillableElements.length; i++) {
      const svgEl = fillableElements[i] as SVGElement;
      // Update attribute if it exists
      if (svgEl.hasAttribute('fill')) {
        svgEl.setAttribute('fill', rgbColor);
      }
      // Update style if it exists
      if (svgEl.style.fill) {
        svgEl.style.fill = rgbColor;
      }
    }

    // DO NOT update stroke colors - black outlines must remain black
  }

  public getLastRotation(flightId: string): number | undefined {
    return this.lastRotationMap.get(flightId);
  }

  public setLastRotation(flightId: string, rotation: number): void {
    this.lastRotationMap.set(flightId, rotation);
  }

  public clearFlightData(flightId: string) {
    this.svgDataCache.delete(flightId);
    this.lastRotationMap.delete(flightId);
    this.callsignMap.delete(flightId);
    this.aircraftTypeMap.delete(flightId);
  }
}

export class AircraftMarker {
  private marker: HereMarker;
  private lastUpdate: Date;
  private interpolator: AircraftInterpolator;
  private animationFrameId: number | null = null;
  private isAnimating: boolean = false;
  private lastRenderedPosition: HereCoordinates | null = null;
  private callsign: string | undefined;
  private aircraftType: string | undefined;
  private icaoType: string | undefined;
  private protoCategory: number | undefined;
  private lastFrameTime: number = 0;
  private readonly TARGET_FPS = 25;

  constructor(
    private flightId: string,
    private coords: HereCoordinates,
    private aircraftIcon: AircraftIcon,
    private map: { addObject: (marker: HereMarker) => void; removeObject: (marker: HereMarker) => void },
    private iconSvgMap: Map<string, SVGElement>,
    callsign?: string,
    protoCategory?: number,
    aircraftType?: string,
    icaoType?: string,
  ) {
    this.callsign = callsign;
    this.protoCategory = protoCategory;
    this.aircraftType = aircraftType;
    this.icaoType = icaoType;

    if (callsign) {
      this.aircraftIcon.setCallsign(flightId, callsign);
    }

    // Determine the aircraft category:
    // 1. First try to use the ADS-B category from protobuf if available
    // 2. Fall back to heuristic-based detection from aircraft type
    let category: AircraftCategory;
    if (protoCategory !== undefined) {
      category = mapProtobufCategoryToIcon(protoCategory);
    } else {
      category = determineAircraftCategory(aircraftType, icaoType);
    }

    this.aircraftIcon.setAircraftType(flightId, category);

    this.marker = new H.map.DomMarker(coords, { icon: this.aircraftIcon.hereIcon, data: flightId });
    this.map.addObject(this.marker);
    this.lastUpdate = new Date();
    this.interpolator = new AircraftInterpolator();

    this.interpolator.addPositionUpdate(coords);

    // Load the appropriate icon after marker is created
    this.loadAircraftIcon(category);
  }

  private async loadAircraftIcon(category: AircraftCategory) {
    try {
      const svgContent = await loadIconSvg(category);

      // Parse the new SVG content once
      const parser = new DOMParser();
      const doc = parser.parseFromString(svgContent, 'image/svg+xml');
      const newSvg = doc.querySelector('svg');

      if (!newSvg) {
        return;
      }

      const newSvgElement = newSvg as unknown as SVGElement;

      // Position the SVG absolutely, centered on the position point (0, 0)
      const width = parseFloat(newSvg.getAttribute('width') || '22.5');
      const height = parseFloat(newSvg.getAttribute('height') || '30');

      // Scale down default icon to 50%
      const scale = category === 'default' ? 0.5 : 1;
      const scaledWidth = width * scale;
      const scaledHeight = height * scale;

      // Apply positioning and optimization styles once
      newSvgElement.style.cssText = `position: absolute; left: ${-scaledWidth / 2}px; top: ${-scaledHeight / 2}px; width: ${scaledWidth}px; height: ${scaledHeight}px; will-change: transform; backface-visibility: hidden;`;

      // Cache with optimized structure - this handles all color operations
      this.aircraftIcon.cacheSvgForFlight(this.flightId, newSvgElement, AircraftIcon.INACTIVE_COLOR);

      // Only update visible element if currently attached
      const svgElement = this.iconSvgMap.get(this.flightId);
      if (svgElement && svgElement.parentElement) {
        // Get cached data to use the pre-styled SVG
        const cachedData = this.aircraftIcon.getCachedSvgData(this.flightId);
        if (cachedData) {
          const restoredSvg = cachedData.svgElement.cloneNode(true) as SVGElement;

          // Preserve rotation
          const currentTransform = svgElement.style.transform;
          if (currentTransform) {
            restoredSvg.style.transform = currentTransform;
          } else if (this.coords.heading !== undefined) {
            // If no rotation was set yet, apply initial heading from coords
            // This handles the case where the icon loads before the first update
            restoredSvg.style.transform = `rotate(${this.coords.heading}deg)`;
            this.aircraftIcon.setLastRotation(this.flightId, this.coords.heading);
          }

          // Replace element
          svgElement.parentElement.replaceChild(restoredSvg, svgElement);
          this.iconSvgMap.set(this.flightId, restoredSvg);

          // Cache the visible element references for fast color updates
          const fillableElements: Element[] = [];
          const strokeableElements: Element[] = [];

          restoredSvg.querySelectorAll('*').forEach(element => {
            const svgEl = element as SVGElement;

            // Check for fill in attribute or style
            const fillAttr = svgEl.getAttribute('fill');
            if (fillAttr && fillAttr !== 'none' && !fillAttr.includes('url(')) {
              fillableElements.push(element);
            } else if (svgEl.style.fill && svgEl.style.fill !== 'none' && !svgEl.style.fill.includes('url(')) {
              fillableElements.push(element);
            }

            // Check for stroke in attribute or style
            const strokeAttr = svgEl.getAttribute('stroke');
            if (strokeAttr && strokeAttr !== 'black' && strokeAttr !== '#000000' && !strokeAttr.includes('url(')) {
              strokeableElements.push(element);
            } else if (svgEl.style.stroke && svgEl.style.stroke !== 'black' && svgEl.style.stroke !== '#000000' && !svgEl.style.stroke.includes('url(')) {
              strokeableElements.push(element);
            }
          });

          this.aircraftIcon.setVisibleElements(this.flightId, fillableElements, strokeableElements);
        }
      }
    } catch (error) {
      console.error(`Failed to load icon for aircraft ${this.flightId}:`, error);
    }
  }

  public get hereMarker() {
    return this.marker;
  }

  public onPointerDown(callback: (event: { target: { getData(): string } }) => void) {
    this.marker.addEventListener('pointerdown', callback);
  }

  public updatePosition(coords: HereCoordinates, groundSpeed?: number) {
    this.interpolator.addPositionUpdate(coords, groundSpeed);

    // Only update rotation if we have a valid heading (not undefined)
    if (this.iconSvgMap.has(this.flightId) && coords.heading !== undefined) {
      const svgElement = this.iconSvgMap.get(this.flightId);
      if (svgElement) {
        const lastRotation = this.aircraftIcon.getLastRotation(this.flightId);
        // Only update DOM if rotation actually changed
        if (lastRotation !== coords.heading) {
          svgElement.style.transform = `rotate(${coords.heading}deg)`;
          this.aircraftIcon.setLastRotation(this.flightId, coords.heading);
        }
      }
    }

    this.lastUpdate = new Date();

    if (!this.isAnimating) {
      this.startAnimation();
    }
  }

  private startAnimation() {
    if (this.isAnimating) return;

    this.isAnimating = true;
    this.lastFrameTime = performance.now();

    const animate = (currentTime: number) => {
      if (!this.isAnimating) return;

      const elapsed = currentTime - this.lastFrameTime;

      const frameInterval = 1000 / this.TARGET_FPS;

      if (elapsed >= frameInterval) {
        this.lastFrameTime = currentTime - (elapsed % frameInterval);

        const interpolatedPos = this.interpolator.getInterpolatedPosition();
        if (interpolatedPos) {
          this.marker.setGeometry(interpolatedPos);
          this.lastRenderedPosition = { ...interpolatedPos };

          // Only update rotation if we have a valid heading (not undefined)
          if (this.iconSvgMap.has(this.flightId) && interpolatedPos.heading !== undefined) {
            const svgElement = this.iconSvgMap.get(this.flightId);
            if (svgElement) {
              const lastRotation = this.aircraftIcon.getLastRotation(this.flightId);
              // Only update DOM if rotation actually changed
              if (lastRotation !== interpolatedPos.heading) {
                svgElement.style.transform = `rotate(${interpolatedPos.heading}deg)`;
                this.aircraftIcon.setLastRotation(this.flightId, interpolatedPos.heading);
              }
            }
          }
        }
      }

      this.animationFrameId = requestAnimationFrame(animate);
    };

    this.animationFrameId = requestAnimationFrame(animate);
  }

  private stopAnimation() {
    this.isAnimating = false;
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }

  public remove() {
    this.stopAnimation();
    this.map.removeObject(this.hereMarker);
    // Clear cached data for this flight
    this.aircraftIcon.clearFlightData(this.flightId);
  }

  public setColor(rgbString: string) {
    // Update the cached color first (this will update the cached SVG)
    this.aircraftIcon.setCurrentColor(this.flightId, rgbString);

    // If the icon is currently visible, update it immediately using cached element references
    const visibleElements = this.aircraftIcon.getVisibleElements(this.flightId);
    if (visibleElements) {
      // FAST PATH: Use pre-cached element references - no querySelectorAll needed!
      const rgbColor = `rgb(${rgbString})`;

      // Only update fill colors - stroke colors (outlines) must remain black
      for (let i = 0; i < visibleElements.fillableElements.length; i++) {
        const svgEl = visibleElements.fillableElements[i] as SVGElement;
        // Update attribute if it exists
        if (svgEl.hasAttribute('fill')) {
          svgEl.setAttribute('fill', rgbColor);
        }
        // Update style if it exists
        if (svgEl.style.fill) {
          svgEl.style.fill = rgbColor;
        }
      }

      // DO NOT update stroke colors - black outlines must remain black
    }
  }

  public updateCallsign(callsign: string) {
    if (this.callsign !== callsign) {
      this.callsign = callsign;
      this.aircraftIcon.setCallsign(this.flightId, callsign);
    }
  }

  public updateAircraftType(aircraftType?: string, icaoType?: string) {
    // Only update if the type has changed
    if (this.aircraftType === aircraftType && this.icaoType === icaoType) {
      return;
    }

    this.aircraftType = aircraftType;
    this.icaoType = icaoType;

    const category = determineAircraftCategory(aircraftType, icaoType);
    const currentCategory = this.aircraftIcon.getAircraftType(this.flightId);

    // Only reload icon if category changed
    if (category !== currentCategory) {
      this.aircraftIcon.setAircraftType(this.flightId, category);
      this.loadAircraftIcon(category);
    }
  }

  public updateCategory(protoCategory: number) {
    // Only update if the category has changed
    if (this.protoCategory === protoCategory) {
      return;
    }

    this.protoCategory = protoCategory;

    const category = mapProtobufCategoryToIcon(protoCategory);
    const currentCategory = this.aircraftIcon.getAircraftType(this.flightId);

    // Only reload icon if category changed
    if (category !== currentCategory) {
      this.aircraftIcon.setAircraftType(this.flightId, category);
      this.loadAircraftIcon(category);
    }
  }

  public get lastUpdated(): Date {
    return this.lastUpdate;
  }
}
