import { describe, expect, it } from "vitest";
import { SurfaceViewport } from "./viewport";

describe("surface viewport", () => {
  it("round-trips coordinates and pans in screen units", () => {
    const viewport = new SurfaceViewport({
      zoomX: 4,
      zoomY: 8,
      originX: 2,
      originY: -3,
    });
    const domain = { x: 7.25, y: 1.5 };
    expect(viewport.screenToDomain(viewport.domainToScreen(domain))).toEqual(
      domain,
    );
    viewport.panScreen(8, 16);
    expect(viewport.originX).toBe(0);
    expect(viewport.originY).toBe(-5);
  });

  it("preserves a pointer-centered zoom anchor and enforces limits", () => {
    const viewport = new SurfaceViewport({
      minZoomX: 1,
      maxZoomX: 8,
      minZoomY: 2,
      maxZoomY: 16,
      zoomX: 2,
      zoomY: 4,
    });
    const anchor = { x: 120, y: 80 };
    const before = viewport.screenToDomain(anchor);
    viewport.zoomAt(anchor, 100);
    expect(viewport.screenToDomain(anchor)).toEqual(before);
    expect(viewport.zoomX).toBe(8);
    expect(viewport.zoomY).toBe(16);
    viewport.resize(300, 200);
    expect(viewport.screenToDomain(anchor)).toEqual(before);
  });
});
