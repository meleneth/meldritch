export interface Point {
  x: number;
  y: number;
}

export interface DomainBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ViewportOptions {
  minZoomX?: number;
  maxZoomX?: number;
  minZoomY?: number;
  maxZoomY?: number;
  zoomX?: number;
  zoomY?: number;
  originX?: number;
  originY?: number;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

/** Invertible domain -> viewport -> screen transform (CSS pixels). */
export class SurfaceViewport {
  width = 1;
  height = 1;
  originX: number;
  originY: number;
  zoomX: number;
  zoomY: number;
  readonly minZoomX: number;
  readonly maxZoomX: number;
  readonly minZoomY: number;
  readonly maxZoomY: number;

  constructor(options: ViewportOptions = {}) {
    this.minZoomX = options.minZoomX ?? 0.25;
    this.maxZoomX = options.maxZoomX ?? 64;
    this.minZoomY = options.minZoomY ?? 0.25;
    this.maxZoomY = options.maxZoomY ?? 64;
    this.zoomX = clamp(options.zoomX ?? 1, this.minZoomX, this.maxZoomX);
    this.zoomY = clamp(options.zoomY ?? 1, this.minZoomY, this.maxZoomY);
    this.originX = options.originX ?? 0;
    this.originY = options.originY ?? 0;
  }

  resize(width: number, height: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
  }

  domainToScreen(point: Point): Point {
    return {
      x: (point.x - this.originX) * this.zoomX,
      y: (point.y - this.originY) * this.zoomY,
    };
  }

  screenToDomain(point: Point): Point {
    return {
      x: point.x / this.zoomX + this.originX,
      y: point.y / this.zoomY + this.originY,
    };
  }

  panScreen(deltaX: number, deltaY: number) {
    this.originX -= deltaX / this.zoomX;
    this.originY -= deltaY / this.zoomY;
  }

  zoomAt(anchor: Point, factorX: number, factorY = factorX) {
    const domainAnchor = this.screenToDomain(anchor);
    this.zoomX = clamp(this.zoomX * factorX, this.minZoomX, this.maxZoomX);
    this.zoomY = clamp(this.zoomY * factorY, this.minZoomY, this.maxZoomY);
    this.originX = domainAnchor.x - anchor.x / this.zoomX;
    this.originY = domainAnchor.y - anchor.y / this.zoomY;
  }

  fit(bounds: DomainBounds, padding = 0) {
    const availableWidth = Math.max(1, this.width - padding * 2);
    const availableHeight = Math.max(1, this.height - padding * 2);
    this.zoomX = clamp(
      availableWidth / Math.max(Number.EPSILON, bounds.width),
      this.minZoomX,
      this.maxZoomX,
    );
    this.zoomY = clamp(
      availableHeight / Math.max(Number.EPSILON, bounds.height),
      this.minZoomY,
      this.maxZoomY,
    );
    this.originX = bounds.x - padding / this.zoomX;
    this.originY = bounds.y - padding / this.zoomY;
  }

  reset() {
    this.originX = 0;
    this.originY = 0;
    this.zoomX = clamp(1, this.minZoomX, this.maxZoomX);
    this.zoomY = clamp(1, this.minZoomY, this.maxZoomY);
  }
}
