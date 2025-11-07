// This file contains elements for aircraft visualization
import { HereCoordinates } from './flightPath';
import { AircraftInterpolator } from './AircraftInterpolator';

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

export class AircraftIcon {
  private aircraftIcon: HereDomIcon;

  public static readonly INACTIVE_COLOR = '250, 255, 255';
  public static readonly HIGHLIGHT_COLOR = '250, 127, 0';

  constructor(
    private iconSvgMap: Map<string, SVGElement>,
    private callsignMap: Map<string, string> = new Map(),
  ) {
    const aircraftDomIconElement = document.createElement('div');
    aircraftDomIconElement.className = 'aircraft-icon-wrapper';

    aircraftDomIconElement.style.margin = '-12px 0 0 -5px';
    aircraftDomIconElement.style.willChange = 'transform';
    aircraftDomIconElement.style.transform = 'translateZ(0)';
    aircraftDomIconElement.style.position = 'relative';

    aircraftDomIconElement.innerHTML = `
      <svg xmlns="http://www.w3.org/2000/svg" width="15px" height="20px">
        <polygon points="0,20 7.5,12 15,20 7.5,0 0,20" fill="rgb(${AircraftIcon.INACTIVE_COLOR})" stroke="black" stroke-width="1" />
      </svg>
      <div class="aircraft-popover" style="
        display: none;
        position: absolute;
        bottom: 25px;
        left: 50%;
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
      onAttach: (clonedElement: HTMLElement, domIcon: unknown, domMarker: { getData(): string }) => {
        const clonedContent = clonedElement.getElementsByTagName('svg')[0];
        const flightId = domMarker.getData();
        this.iconSvgMap.set(flightId, clonedContent);

        const popover = clonedElement.querySelector('.aircraft-popover') as HTMLElement;
        if (popover) {
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
      onDetach: (clonedElement: HTMLElement, domIcon: unknown, domMarker: { getData(): string }) => {
        if (this.iconSvgMap.has(domMarker.getData())) {
          this.iconSvgMap.delete(domMarker.getData());
        }
      },
    });
  }

  public get hereIcon() {
    return this.aircraftIcon;
  }

  public setCallsign(flightId: string, callsign: string) {
    this.callsignMap.set(flightId, callsign);
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
  private lastFrameTime: number = 0;
  private readonly TARGET_FPS = 25;

  constructor(
    private flightId: string,
    private coords: HereCoordinates,
    private aircraftIcon: AircraftIcon,
    private map: { addObject: (marker: HereMarker) => void; removeObject: (marker: HereMarker) => void },
    private iconSvgMap: Map<string, SVGElement>,
    callsign?: string,
  ) {
    this.callsign = callsign;

    if (callsign) {
      this.aircraftIcon.setCallsign(flightId, callsign);
    }

    this.marker = new H.map.DomMarker(coords, { icon: this.aircraftIcon.hereIcon, data: flightId });
    this.map.addObject(this.marker);
    this.lastUpdate = new Date();
    this.interpolator = new AircraftInterpolator();

    this.interpolator.addPositionUpdate(coords);
  }

  public get hereMarker() {
    return this.marker;
  }

  public onPointerDown(callback: (event: { target: { getData(): string } }) => void) {
    this.marker.addEventListener('pointerdown', callback);
  }

  public updatePosition(coords: HereCoordinates, groundSpeed?: number) {
    this.interpolator.addPositionUpdate(coords, groundSpeed);

    if (this.iconSvgMap.has(this.flightId) && coords.heading != null) {
      const svgElement = this.iconSvgMap.get(this.flightId);
      if (svgElement) {
        svgElement.style.transform = 'rotate(' + coords.heading + 'deg)';
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

          if (this.iconSvgMap.has(this.flightId) && interpolatedPos.heading != null) {
            const svgElement = this.iconSvgMap.get(this.flightId);
            if (svgElement) {
              svgElement.style.transform = 'rotate(' + interpolatedPos.heading + 'deg)';
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
  }

  public setColor(rgbString: string) {
    const iconSvg = this.iconSvgMap.get(this.flightId);
    if (iconSvg) {
      const polygon = iconSvg.getElementsByTagName('polygon')[0];
      if (polygon) {
        polygon.setAttribute('fill', `rgb(${rgbString})`);
      }
    }
  }

  public updateCallsign(callsign: string) {
    if (this.callsign !== callsign) {
      this.callsign = callsign;
      this.aircraftIcon.setCallsign(this.flightId, callsign);

      const markerElement = (this.marker as any).wb;
      if (markerElement) {
        const popover = markerElement.querySelector('.aircraft-popover') as HTMLElement;
        if (popover) {
          popover.textContent = callsign;
        }
      }
    }
  }

  public get lastUpdated(): Date {
    return this.lastUpdate;
  }
}
