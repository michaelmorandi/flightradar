/**
 * Aircraft position interpolator for smooth animations between position updates
 *
 * This class estimates aircraft trajectory and velocity based on position updates,
 * then smoothly animates the aircraft between updates using requestAnimationFrame.
 * When new position data arrives, it corrects the trajectory and continues smooth motion.
 */

import { HereCoordinates } from './flightPath';

export interface PositionUpdate {
  coords: HereCoordinates;
  timestamp: number;
  groundSpeed?: number; // knots
}

export interface InterpolationState {
  current: HereCoordinates;
  target: HereCoordinates;
  velocity: { lat: number; lng: number }; // degrees per millisecond
  startTime: number;
  lastUpdate: number;
  estimatedArrivalTime: number;
  isMoving: boolean;
}

export class AircraftInterpolator {
  private state: InterpolationState | null = null;
  private history: PositionUpdate[] = [];
  private readonly MAX_HISTORY = 5;
  private readonly UPDATE_INTERVAL_MS = 2000; // Expected update interval
  private readonly MAX_INTERPOLATION_TIME = 3000; // Stop interpolating after 3s without update

  /**
   * Add a new position update and calculate velocity
   */
  public addPositionUpdate(coords: HereCoordinates, groundSpeed?: number): void {
    const now = Date.now();
    const update: PositionUpdate = { coords, timestamp: now, groundSpeed };

    // Add to history
    this.history.push(update);
    if (this.history.length > this.MAX_HISTORY) {
      this.history.shift();
    }

    // Calculate velocity based on previous position
    if (this.history.length >= 2) {
      const prev = this.history[this.history.length - 2];
      const curr = this.history[this.history.length - 1];

      const timeDelta = curr.timestamp - prev.timestamp;
      if (timeDelta > 0) {
        const latDelta = curr.coords.lat - prev.coords.lat;
        const lngDelta = curr.coords.lng - prev.coords.lng;

        // Check if aircraft is actually moving (threshold: ~0.5 knots)
        const distance = Math.sqrt(latDelta * latDelta + lngDelta * lngDelta);
        const isMoving = distance > 0.00001; // ~1 meter

        // IMPORTANT: Start from current interpolated position, not the new coords
        // This prevents jumps when new data arrives
        let startPosition: HereCoordinates;
        let velocity: { lat: number; lng: number };

        if (this.state) {
          // Calculate where we are NOW based on the previous state
          const timeSinceLastUpdate = now - this.state.startTime;
          const previousProgress = Math.min(timeSinceLastUpdate / this.UPDATE_INTERVAL_MS, 1.0);
          startPosition = {
            lat: this.state.current.lat + (this.state.target.lat - this.state.current.lat) * previousProgress,
            lng: this.state.current.lng + (this.state.target.lng - this.state.current.lng) * previousProgress,
            heading: coords.heading !== undefined ? coords.heading : this.state.current.heading,
          };

          // Recalculate velocity from current interpolated position to new target
          // This prevents overshoot when starting from an interpolated position
          const newLatDelta = coords.lat - startPosition.lat;
          const newLngDelta = coords.lng - startPosition.lng;
          velocity = {
            lat: newLatDelta / this.UPDATE_INTERVAL_MS,
            lng: newLngDelta / this.UPDATE_INTERVAL_MS,
          };
        } else {
          startPosition = coords;
          // Use velocity calculated from actual position history
          velocity = {
            lat: latDelta / timeDelta,
            lng: lngDelta / timeDelta,
          };
        }

        // Update state
        this.state = {
          current: startPosition,
          target: { ...coords }, // New actual position becomes the target
          velocity,
          startTime: now,
          lastUpdate: now,
          estimatedArrivalTime: now + this.UPDATE_INTERVAL_MS,
          isMoving,
        };
      }
    } else {
      // First position - no velocity yet
      this.state = {
        current: { ...coords },
        target: { ...coords },
        velocity: { lat: 0, lng: 0 },
        startTime: now,
        lastUpdate: now,
        estimatedArrivalTime: now + this.UPDATE_INTERVAL_MS,
        isMoving: false,
      };
    }
  }

  /**
   * Estimate the next position based on current velocity and expected update interval
   */
  private estimateNextPosition(current: HereCoordinates, velocity: { lat: number; lng: number }): HereCoordinates {
    // Estimate where the aircraft will be in UPDATE_INTERVAL_MS
    const estimatedLat = current.lat + velocity.lat * this.UPDATE_INTERVAL_MS;
    const estimatedLng = current.lng + velocity.lng * this.UPDATE_INTERVAL_MS;

    return {
      lat: estimatedLat,
      lng: estimatedLng,
      heading: current.heading,
    };
  }

  /**
   * Get the interpolated position at the current time
   * Returns null if no position data is available
   */
  public getInterpolatedPosition(): HereCoordinates | null {
    if (!this.state) {
      return null;
    }

    if (!this.state.isMoving) {
      return this.state.current;
    }

    const now = Date.now();
    const timeSinceUpdate = now - this.state.startTime;

    // Stop interpolating if too much time has passed without an update
    if (timeSinceUpdate > this.MAX_INTERPOLATION_TIME) {
      return this.state.target;
    }

    // Use constant velocity motion - aircraft fly at steady speeds
    // Simply move forward using the calculated velocity
    const interpolatedLat = this.state.current.lat + this.state.velocity.lat * timeSinceUpdate;
    const interpolatedLng = this.state.current.lng + this.state.velocity.lng * timeSinceUpdate;

    return {
      lat: interpolatedLat,
      lng: interpolatedLng,
      heading: this.state.current.heading,
    };
  }

  /**
   * Check if the interpolator has valid state
   */
  public hasState(): boolean {
    return this.state !== null;
  }

  /**
   * Get the current velocity in degrees per second
   */
  public getVelocity(): { lat: number; lng: number } | null {
    if (!this.state) return null;
    return {
      lat: this.state.velocity.lat * 1000,
      lng: this.state.velocity.lng * 1000,
    };
  }

  /**
   * Get time since last update in milliseconds
   */
  public getTimeSinceLastUpdate(): number {
    if (!this.state) return 0;
    return Date.now() - this.state.lastUpdate;
  }

  /**
   * Reset the interpolator state
   */
  public reset(): void {
    this.state = null;
    this.history = [];
  }

  /**
   * Check if aircraft is moving
   */
  public isMoving(): boolean {
    return this.state?.isMoving || false;
  }
}
